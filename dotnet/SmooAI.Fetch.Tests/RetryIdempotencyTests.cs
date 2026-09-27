using System.Collections.Concurrent;
using System.Diagnostics;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Text.Json;
using SmooAI.Fetch;

namespace SmooAI.Fetch.Tests;

/// <summary>
/// SMOODEV-3375 — retries were method-blind, so a POST that timed out or got a 429/5xx was
/// re-sent up to twice more and its side effect (a charge, a message, a generated image)
/// could execute three times server-side.
///
/// Every case comes from spec/retry-idempotency-corpus.json, shared with the other four
/// ports (copied next to the test assembly by the csproj). Each one runs against a REAL
/// local server and asserts how many requests the SERVER received: the bug is a side
/// effect repeating server-side, and only the server can count that.
/// </summary>
public class RetryIdempotencyTests
{
    private static readonly Lazy<JsonElement> CorpusRoot = new(() =>
    {
        using var doc = JsonDocument.Parse(File.ReadAllText("retry-idempotency-corpus.json"));
        return doc.RootElement.Clone();
    });

    private static JsonElement Corpus => CorpusRoot.Value;

    public static IEnumerable<object[]> Cases() =>
        Corpus.GetProperty("cases").EnumerateArray().Select(c => new object[] { c.GetProperty("name").GetString()! });

    // Positive control: a corpus that failed to load, or loaded empty, would make every
    // Theory below vacuous — xunit reports zero cases as a pass.
    [Fact]
    public void Loaded_the_shared_corpus()
    {
        Assert.True(Corpus.GetProperty("cases").GetArrayLength() > 0, "retry corpus has no cases");
        Assert.True(Corpus.GetProperty("methods").GetProperty("idempotent").GetArrayLength() > 0);
        Assert.True(Corpus.GetProperty("methods").GetProperty("nonIdempotent").GetArrayLength() > 0);
        Assert.Equal(RetryPolicy.IdempotencyKeyHeader, Corpus.GetProperty("idempotencyKeyHeader").GetString());
    }

    [Fact]
    public void Classifies_every_corpus_method()
    {
        foreach (var m in Corpus.GetProperty("methods").GetProperty("idempotent").EnumerateArray())
        {
            Assert.True(RetryPolicy.IsIdempotentMethod(m.GetString()!), $"{m} should be idempotent");
            Assert.True(RetryPolicy.IsIdempotentMethod(new HttpMethod(m.GetString()!)), $"{m} should be idempotent");
        }

        foreach (var m in Corpus.GetProperty("methods").GetProperty("nonIdempotent").EnumerateArray())
        {
            Assert.False(RetryPolicy.IsIdempotentMethod(m.GetString()!), $"{m} should NOT be idempotent");
            Assert.False(RetryPolicy.IsIdempotentMethod(new HttpMethod(m.GetString()!)), $"{m} should NOT be idempotent");
        }
    }

    [Theory]
    [MemberData(nameof(Cases))]
    public async Task Server_sees_the_expected_number_of_attempts(string name)
    {
        var c = Corpus.GetProperty("cases").EnumerateArray().Single(x => x.GetProperty("name").GetString() == name);
        var respond = c.GetProperty("respond");
        var hang = respond.TryGetProperty("hang", out var h) && h.GetBoolean();
        var status = hang ? 0 : respond.GetProperty("status").GetInt32();
        var responseHeaders = new Dictionary<string, string>();
        if (!hang && respond.TryGetProperty("headers", out var rh))
        {
            foreach (var p in rh.EnumerateObject())
            {
                responseHeaders[p.Name] = p.Value.GetString()!;
            }
        }

        await using var server = new CountingServer(hang, status, responseHeaders);

        var retry = Corpus.GetProperty("retry");
        var fetch = SmooFetchBuilder.Create()
            .WithRetry(new RetryPolicy
            {
                MaxRetries = retry.GetProperty("attempts").GetInt32(),
                BaseDelay = TimeSpan.FromMilliseconds(retry.GetProperty("initialIntervalMs").GetInt32()),
                BackoffFactor = retry.GetProperty("factor").GetDouble(),
                UseJitter = retry.GetProperty("jitterAdjustment").GetDouble() > 0,
                JitterFraction = retry.GetProperty("jitterAdjustment").GetDouble(),
                AllowNonIdempotent = c.TryGetProperty("allowNonIdempotent", out var a) && a.GetBoolean(),
            })
            .WithTimeout(TimeSpan.FromMilliseconds(Corpus.GetProperty("timeoutMs").GetInt32()))
            .Build();

        using var request = new HttpRequestMessage(new HttpMethod(c.GetProperty("method").GetString()!), server.Url);
        if (c.TryGetProperty("body", out var body))
        {
            request.Content = new StringContent(body.GetString()!, Encoding.UTF8, "application/json");
        }

        if (c.TryGetProperty("headers", out var headers))
        {
            foreach (var p in headers.EnumerateObject())
            {
                request.Headers.TryAddWithoutValidation(p.Name, p.Value.GetString());
            }
        }

        try
        {
            using var response = await fetch.SendAsync(request);
            Assert.Equal(status, (int)response.StatusCode);
        }
        catch (Exception ex) when (hang && ex is OperationCanceledException)
        {
            // Expected: every attempt timed out.
        }

        Assert.Equal(c.GetProperty("expectedAttempts").GetInt32(), server.Requests);
    }

    [Fact]
    public async Task Timed_out_retried_GET_closes_first_connection_before_the_retry_arrives()
    {
        var (timeoutMs, slackMs) = TimeoutAbortKnobs();
        await using var server = new HangingSocketServer();

        var fetch = SmooFetchBuilder.Create()
            .WithRetry(new RetryPolicy { MaxRetries = 1, BaseDelay = TimeSpan.FromMilliseconds(1), UseJitter = false })
            .WithTimeout(TimeSpan.FromMilliseconds(timeoutMs))
            .Build();

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => fetch.GetAsync<object>(server.Url));

        var first = await server.WaitForConnection(0, TimeSpan.FromSeconds(10));
        var second = await server.WaitForConnection(1, TimeSpan.FromSeconds(10));
        var firstClosed = await first.WaitClosed(TimeSpan.FromMilliseconds(timeoutMs + slackMs));
        Assert.NotNull(firstClosed);
        Assert.True(
            firstClosed!.Value - first.ArrivedAt <= TimeSpan.FromMilliseconds(timeoutMs + slackMs),
            $"first attempt's connection closed {(firstClosed.Value - first.ArrivedAt).TotalMilliseconds}ms after it arrived; expected <= {timeoutMs + slackMs}ms");

        // The FIN goes out before the retry's SYN; the two server-side observations race
        // on the thread pool, so allow a small scheduling tolerance. An attempt that was
        // abandoned rather than cancelled stays open for the whole test instead.
        Assert.True(
            firstClosed.Value <= second.ArrivedAt + TimeSpan.FromMilliseconds(100),
            $"first connection closed at {firstClosed.Value.TotalMilliseconds}ms, AFTER the retry arrived at {second.ArrivedAt.TotalMilliseconds}ms — the timed-out attempt was not cancelled");
    }

    [Fact]
    public async Task Timed_out_POST_closes_its_connection_and_is_not_retried()
    {
        var (timeoutMs, slackMs) = TimeoutAbortKnobs();
        await using var server = new HangingSocketServer();

        var fetch = SmooFetchBuilder.Create()
            .WithRetry(new RetryPolicy { MaxRetries = 2, BaseDelay = TimeSpan.FromMilliseconds(1), UseJitter = false })
            .WithTimeout(TimeSpan.FromMilliseconds(timeoutMs))
            .Build();

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => fetch.PostAsync<object, object>(server.Url, new { a = 1 }));

        var first = await server.WaitForConnection(0, TimeSpan.FromSeconds(10));
        var closed = await first.WaitClosed(TimeSpan.FromMilliseconds(timeoutMs + slackMs));
        Assert.NotNull(closed);
        Assert.True(closed!.Value - first.ArrivedAt <= TimeSpan.FromMilliseconds(timeoutMs + slackMs));

        // Give a (wrongly) retried attempt time to show up before asserting it didn't.
        await Task.Delay(200);
        Assert.Equal(1, server.ConnectionCount);
    }

    private static (int timeoutMs, int slackMs) TimeoutAbortKnobs()
    {
        var t = Corpus.GetProperty("timeoutAbort");
        return (t.GetProperty("timeoutMs").GetInt32(), t.GetProperty("closeSlackMs").GetInt32());
    }

    private static async Task<bool> ReadRequestHeadAsync(NetworkStream stream, CancellationToken ct)
    {
        // Read the head byte-by-byte up to CRLFCRLF, then drain Content-Length bytes.
        var head = new StringBuilder();
        var buf = new byte[1];
        while (!head.ToString().EndsWith("\r\n\r\n", StringComparison.Ordinal))
        {
            if (await stream.ReadAsync(buf, ct) == 0)
            {
                return false;
            }

            head.Append((char)buf[0]);
        }

        var length = 0;
        foreach (var line in head.ToString().Split("\r\n"))
        {
            if (line.StartsWith("Content-Length:", StringComparison.OrdinalIgnoreCase))
            {
                length = int.Parse(line["Content-Length:".Length..].Trim());
            }
        }

        var body = new byte[length];
        var read = 0;
        while (read < length)
        {
            var n = await stream.ReadAsync(body.AsMemory(read), ct);
            if (n == 0)
            {
                return false;
            }

            read += n;
        }

        return true;
    }

    /// <summary>
    /// Minimal HTTP/1.1 responder on 127.0.0.1 that counts every request it reads. Replies
    /// with a fixed status and <c>Connection: close</c>, or never replies when hanging.
    /// </summary>
    private sealed class CountingServer : IAsyncDisposable
    {
        private readonly TcpListener _listener = new(IPAddress.Loopback, 0);
        private readonly CancellationTokenSource _cts = new();
        private readonly ConcurrentBag<TcpClient> _clients = new();
        private int _requests;

        public CountingServer(bool hang, int status, IReadOnlyDictionary<string, string> headers)
        {
            _listener.Start();
            Url = $"http://127.0.0.1:{((IPEndPoint)_listener.LocalEndpoint).Port}/probe";
            _ = Task.Run(() => AcceptLoop(hang, status, headers));
        }

        public string Url { get; }

        public int Requests => Volatile.Read(ref _requests);

        private async Task AcceptLoop(bool hang, int status, IReadOnlyDictionary<string, string> headers)
        {
            while (!_cts.IsCancellationRequested)
            {
                TcpClient client;
                try
                {
                    client = await _listener.AcceptTcpClientAsync(_cts.Token);
                }
                catch
                {
                    return;
                }

                _clients.Add(client);
                _ = Task.Run(async () =>
                {
                    try
                    {
                        var stream = client.GetStream();
                        if (!await ReadRequestHeadAsync(stream, _cts.Token))
                        {
                            return;
                        }

                        Interlocked.Increment(ref _requests);
                        if (hang)
                        {
                            return; // Never answer; the socket is closed at teardown.
                        }

                        var sb = new StringBuilder($"HTTP/1.1 {status} X\r\nContent-Length: 0\r\nConnection: close\r\n");
                        foreach (var kvp in headers)
                        {
                            sb.Append($"{kvp.Key}: {kvp.Value}\r\n");
                        }

                        sb.Append("\r\n");
                        await stream.WriteAsync(Encoding.ASCII.GetBytes(sb.ToString()), _cts.Token);
                        client.Close();
                    }
                    catch
                    {
                        // Client went away; nothing to count.
                    }
                });
            }
        }

        public ValueTask DisposeAsync()
        {
            _cts.Cancel();
            _listener.Stop();
            foreach (var c in _clients)
            {
                c.Dispose();
            }

            _cts.Dispose();
            return ValueTask.CompletedTask;
        }
    }

    /// <summary>
    /// Raw socket server that reads each request and never answers, recording when each
    /// connection arrived and when the CLIENT closed it (Read returns 0 or the socket resets).
    /// </summary>
    private sealed class HangingSocketServer : IAsyncDisposable
    {
        private readonly TcpListener _listener = new(IPAddress.Loopback, 0);
        private readonly CancellationTokenSource _cts = new();
        private readonly Stopwatch _clock = Stopwatch.StartNew();
        private readonly List<Conn> _connections = new();
        private readonly SemaphoreSlim _arrived = new(0);

        public HangingSocketServer()
        {
            _listener.Start();
            Url = $"http://127.0.0.1:{((IPEndPoint)_listener.LocalEndpoint).Port}/hang";
            _ = Task.Run(AcceptLoop);
        }

        public string Url { get; }

        public int ConnectionCount
        {
            get
            {
                lock (_connections)
                {
                    return _connections.Count;
                }
            }
        }

        public async Task<Conn> WaitForConnection(int index, TimeSpan timeout)
        {
            var deadline = _clock.Elapsed + timeout;
            while (_clock.Elapsed < deadline)
            {
                lock (_connections)
                {
                    if (_connections.Count > index)
                    {
                        return _connections[index];
                    }
                }

                await _arrived.WaitAsync(TimeSpan.FromMilliseconds(50));
            }

            throw new TimeoutException($"connection #{index + 1} never arrived");
        }

        private async Task AcceptLoop()
        {
            while (!_cts.IsCancellationRequested)
            {
                TcpClient client;
                try
                {
                    client = await _listener.AcceptTcpClientAsync(_cts.Token);
                }
                catch
                {
                    return;
                }

                var conn = new Conn(client, _clock.Elapsed);
                lock (_connections)
                {
                    _connections.Add(conn);
                }

                _arrived.Release();
                _ = Task.Run(async () =>
                {
                    var buf = new byte[4096];
                    try
                    {
                        var stream = client.GetStream();
                        while (await stream.ReadAsync(buf, _cts.Token) > 0)
                        {
                            // Swallow the request; never answer.
                        }
                    }
                    catch (OperationCanceledException) when (_cts.IsCancellationRequested)
                    {
                        return; // Teardown, not a client close.
                    }
                    catch
                    {
                        // A reset counts as closed too.
                    }

                    conn.Closed.TrySetResult(_clock.Elapsed);
                });
            }
        }

        public ValueTask DisposeAsync()
        {
            _cts.Cancel();
            _listener.Stop();
            lock (_connections)
            {
                foreach (var c in _connections)
                {
                    c.Client.Dispose();
                }
            }

            _cts.Dispose();
            return ValueTask.CompletedTask;
        }

        public sealed class Conn(TcpClient client, TimeSpan arrivedAt)
        {
            public TcpClient Client { get; } = client;

            public TimeSpan ArrivedAt { get; } = arrivedAt;

            public TaskCompletionSource<TimeSpan> Closed { get; } = new(TaskCreationOptions.RunContinuationsAsynchronously);

            public async Task<TimeSpan?> WaitClosed(TimeSpan timeout)
            {
                var done = await Task.WhenAny(Closed.Task, Task.Delay(timeout));
                return done == Closed.Task ? Closed.Task.Result : null;
            }
        }
    }
}
