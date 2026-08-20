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
        // One call fills the window; the next must wait for it to roll over,
        // proving the limiter state is held on the client instance rather than
        // reconstructed per fetch().
        //
        // A single priming call is load-bearing. This used to prime with THREE
        // calls against an 800ms window and time the fourth — so whenever a cold
        // WireMock made those three take longer than the window, it had already
        // rolled over and the timed call waited 0ms. That is the flake that went
        // red on CI (`got 0ms`) while passing everywhere else. Priming once
        // against a window far longer than any plausible single request leaves
        // the assertion measuring the limiter instead of the machine.
        _server
            .Given(Request.Create().WithPath("/ping").UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"pong\":\"yes\"}"));

        var window = TimeSpan.FromSeconds(3);
        var fetch = SmooFetchBuilder.Create()
            .WithBaseUrl(_server.Urls[0])
            .WithRetry(RetryPolicy.None)
            .WithRateLimit(maxRequests: 1, window: window)
            .Build();

        var primed = Stopwatch.StartNew();
        await fetch.GetAsync<PingResponse>("/ping");
        primed.Stop();

        var sw = Stopwatch.StartNew();
        await fetch.GetAsync<PingResponse>("/ping");
        sw.Stop();

        // The second call waits out whatever is left of the window after the
        // first one. Assert against that remainder rather than a fixed floor, so
        // a slow priming call shrinks the expectation instead of failing it.
        var expected = window - primed.Elapsed - TimeSpan.FromMilliseconds(500);
        Assert.True(
            expected > TimeSpan.Zero,
            $"Priming call took {primed.ElapsedMilliseconds}ms of a {window.TotalMilliseconds}ms window — the machine is too slow for this test to mean anything");
        Assert.True(
            sw.Elapsed >= expected,
            $"Expected the 2nd call to wait out the window (≥{expected.TotalMilliseconds:F0}ms after a {primed.ElapsedMilliseconds}ms priming call), got {sw.ElapsedMilliseconds}ms");
    }
}
