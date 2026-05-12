using System.Diagnostics;
using SmooAI.Fetch;
using WireMock.RequestBuilders;
using WireMock.ResponseBuilders;
using WireMock.Server;

namespace SmooAI.Fetch.Tests;

public class RateLimiterTests : IAsyncLifetime
{
    private WireMockServer _server = null!;

    public Task InitializeAsync()
    {
        _server = WireMockServer.Start();
        return Task.CompletedTask;
    }

    public Task DisposeAsync()
    {
        _server.Stop();
        _server.Dispose();
        return Task.CompletedTask;
    }

    private sealed record PingResponse(string Pong);

    [Fact]
    public async Task RateLimit_three_per_second_allows_first_three_immediately_and_queues_remaining()
    {
        _server
            .Given(Request.Create().WithPath("/ping").UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"pong\":\"yes\"}"));

        var fetch = SmooFetchBuilder.Create()
            .WithBaseUrl(_server.Urls[0])
            .WithRetry(RetryPolicy.None)
            .WithRateLimit(maxRequests: 3, window: TimeSpan.FromSeconds(1))
            .Build();

        var sw = Stopwatch.StartNew();
        var tasks = Enumerable.Range(0, 5)
            .Select(_ => fetch.GetAsync<PingResponse>("/ping"))
            .ToArray();
        var results = await Task.WhenAll(tasks);
        sw.Stop();

        Assert.Equal(5, results.Length);
        Assert.All(results, r => Assert.Equal("yes", r.Pong));

        // 5 requests with max=3 / window=1s must take at least one window for
        // the 4th & 5th to acquire permits, but well under two windows.
        Assert.True(
            sw.Elapsed >= TimeSpan.FromMilliseconds(900),
            $"Expected >=900ms (one window) for the 4th/5th request to wait, got {sw.ElapsedMilliseconds}ms");
        Assert.True(
            sw.Elapsed < TimeSpan.FromSeconds(3),
            $"Expected <3s total, got {sw.ElapsedMilliseconds}ms");
    }

    [Fact]
    public async Task OnRejected_callback_fires_for_each_request_that_must_wait()
    {
        _server
            .Given(Request.Create().WithPath("/ping").UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"pong\":\"yes\"}"));

        var rejected = 0;
        var fetch = SmooFetchBuilder.Create()
            .WithBaseUrl(_server.Urls[0])
            .WithRetry(RetryPolicy.None)
            .WithRateLimit(
                maxRequests: 3,
                window: TimeSpan.FromSeconds(1),
                onRejected: _ => Interlocked.Increment(ref rejected))
            .Build();

        // Fire 5 sequentially so the rejection accounting is deterministic.
        for (var i = 0; i < 5; i++)
        {
            await fetch.GetAsync<PingResponse>("/ping");
        }

        // First 3 acquired immediately; the 4th has to wait so OnRejected fires
        // at least once. (Subsequent calls may or may not be rejected depending
        // on when the previous waits release relative to the sliding window,
        // so we only assert the lower bound.)
        Assert.True(rejected >= 1, $"Expected OnRejected to fire at least once, got {rejected}");
    }

    [Fact]
    public async Task OnRejected_callback_fires_under_burst_load()
    {
        _server
            .Given(Request.Create().WithPath("/ping").UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"pong\":\"yes\"}"));

        var rejected = 0;
        var fetch = SmooFetchBuilder.Create()
            .WithBaseUrl(_server.Urls[0])
            .WithRetry(RetryPolicy.None)
            .WithRateLimit(
                maxRequests: 3,
                window: TimeSpan.FromSeconds(1),
                onRejected: _ => Interlocked.Increment(ref rejected))
            .Build();

        // Fire all 5 in parallel — only the first 3 can acquire a permit
        // immediately, so the remaining 2 must hit the OnRejected path.
        var tasks = Enumerable.Range(0, 5)
            .Select(_ => fetch.GetAsync<PingResponse>("/ping"))
            .ToArray();
        await Task.WhenAll(tasks);

        Assert.Equal(5, tasks.Length);
        Assert.True(rejected >= 2, $"Expected OnRejected to fire at least twice under burst load, got {rejected}");
    }

    [Fact]
    public async Task RateLimit_state_is_shared_across_calls_on_same_client()
    {
        // Issuing 3 calls fills the window. A 4th must wait ~1s before being
        // sent, proving the limiter state is held on the client instance rather
        // than reconstructed per fetch().
        _server
            .Given(Request.Create().WithPath("/ping").UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"pong\":\"yes\"}"));

        var fetch = SmooFetchBuilder.Create()
            .WithBaseUrl(_server.Urls[0])
            .WithRetry(RetryPolicy.None)
            .WithRateLimit(maxRequests: 3, window: TimeSpan.FromMilliseconds(800))
            .Build();

        await fetch.GetAsync<PingResponse>("/ping");
        await fetch.GetAsync<PingResponse>("/ping");
        await fetch.GetAsync<PingResponse>("/ping");

        var sw = Stopwatch.StartNew();
        await fetch.GetAsync<PingResponse>("/ping");
        sw.Stop();

        // Tolerance is wide because: (a) the first 3 calls consume part of the
        // 800ms window before the 4th is issued, so the actual wait is the
        // remaining window slice (~600ms in dev, less on slower CI), and
        // (b) System.Threading.RateLimiting releases at segment boundaries.
        // The assertion only needs to prove the limiter state is *shared* —
        // any non-trivial wait does that. Use 300ms as a robust floor.
        Assert.True(
            sw.Elapsed >= TimeSpan.FromMilliseconds(300),
            $"Expected 4th call to wait a non-trivial fraction of the window, got {sw.ElapsedMilliseconds}ms");
    }
}
