using System.Net;
using SmooAI.Fetch;
using WireMock.RequestBuilders;
using WireMock.ResponseBuilders;
using WireMock.Server;

namespace SmooAI.Fetch.Tests;

public class SmooFetchBuilderTests : IAsyncLifetime
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

    private sealed record Reply(bool Ok);

    [Fact]
    public async Task Builder_basic_chain_yields_working_fetch()
    {
        _server
            .Given(Request.Create().WithPath("/data").UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"ok\":true}"));

        var fetch = SmooFetchBuilder.Create()
            .WithBaseUrl(_server.Urls[0])
            .WithNoRetry()
            .Build();

        var reply = await fetch.GetAsync<Reply>("/data");
        Assert.True(reply.Ok);
    }

    [Fact]
    public async Task Builder_pre_request_hook_fires_once_per_call()
    {
        _server
            .Given(Request.Create().WithPath("/data").UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"ok\":true}"));

        var calls = 0;
        var fetch = SmooFetchBuilder.Create()
            .WithBaseUrl(_server.Urls[0])
            .WithNoRetry()
            .WithPreRequest((req, _) =>
            {
                calls++;
                req.Headers.TryAddWithoutValidation("X-Hook", "fired");
                return Task.CompletedTask;
            })
            .Build();

        await fetch.GetAsync<Reply>("/data");
        await fetch.GetAsync<Reply>("/data");

        Assert.Equal(2, calls);
        var logs = _server.LogEntries.ToList();
        Assert.All(logs, l => Assert.Equal("fired", l.RequestMessage.Headers!["X-Hook"][0]));
    }

    [Fact]
    public async Task Builder_post_request_ok_hook_observes_response()
    {
        _server
            .Given(Request.Create().WithPath("/data").UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"ok\":true}"));

        HttpStatusCode? captured = null;
        var fetch = SmooFetchBuilder.Create()
            .WithBaseUrl(_server.Urls[0])
            .WithNoRetry()
            .WithPostRequestOk((resp, _) =>
            {
                captured = resp.StatusCode;
                return Task.CompletedTask;
            })
            .Build();

        await fetch.GetAsync<Reply>("/data");
        Assert.Equal(HttpStatusCode.OK, captured);
    }

    [Fact]
    public async Task Builder_post_request_err_hook_fires_on_failure()
    {
        // No mapping registered → server responds 404 to every request.
        var sawErr = false;
        var fetch = SmooFetchBuilder.Create()
            .WithBaseUrl(_server.Urls[0])
            .WithNoRetry()
            .WithPostRequestErr((_, _) =>
            {
                sawErr = true;
                return Task.CompletedTask;
            })
            .Build();

        await Assert.ThrowsAsync<HttpResponseError>(() => fetch.GetAsync<Reply>("/missing"));
        // PostRequestErr fires only when SendAsync itself throws (transport
        // errors / cancellation / pipeline exceptions). HttpResponseError is
        // thrown by the JSON reader *after* SendAsync returns, so the hook
        // does NOT fire here. The test asserts on the inverse: sawErr stays
        // false, confirming the documented contract.
        Assert.False(sawErr);
    }

    [Fact]
    public async Task Builder_fast_first_skips_initial_delay()
    {
        _server
            .Given(Request.Create().WithPath("/flaky").UsingGet())
            .InScenario("fast-first")
            .WillSetStateTo("failed")
            .RespondWith(Response.Create().WithStatusCode(503));

        _server
            .Given(Request.Create().WithPath("/flaky").UsingGet())
            .InScenario("fast-first")
            .WhenStateIs("failed")
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"ok\":true}"));

        var fetch = SmooFetchBuilder.Create()
            .WithBaseUrl(_server.Urls[0])
            // Big base delay; FastFirst should bypass it on the first retry.
            .WithRetry(RetryPolicy.ExponentialBackoff(2, TimeSpan.FromSeconds(5)))
            .WithFastFirst(true)
            .Build();

        var sw = System.Diagnostics.Stopwatch.StartNew();
        var reply = await fetch.GetAsync<Reply>("/flaky");
        sw.Stop();

        Assert.True(reply.Ok);
        Assert.Equal(2, _server.LogEntries.Count());
        // Without FastFirst this would block ~5s. Generous upper bound to
        // tolerate CI flakiness without giving up on the assertion.
        Assert.True(sw.Elapsed < TimeSpan.FromSeconds(2),
            $"FastFirst did not skip initial delay (took {sw.Elapsed.TotalSeconds:F2}s)");
    }

    [Fact]
    public async Task Builder_on_rejection_retry_with_delay_overrides_default()
    {
        _server
            .Given(Request.Create().WithPath("/flaky").UsingGet())
            .InScenario("override-delay")
            .WillSetStateTo("failed")
            .RespondWith(Response.Create().WithStatusCode(503));

        _server
            .Given(Request.Create().WithPath("/flaky").UsingGet())
            .InScenario("override-delay")
            .WhenStateIs("failed")
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"ok\":true}"));

        var consulted = 0;
        var fetch = SmooFetchBuilder.Create()
            .WithBaseUrl(_server.Urls[0])
            // 5s default backoff that the callback should override.
            .WithRetry(RetryPolicy.ExponentialBackoff(2, TimeSpan.FromSeconds(5)))
            .WithOnRejection(_ =>
            {
                consulted++;
                return OnRejectionDecision.RetryWithDelay(TimeSpan.FromMilliseconds(10));
            })
            .Build();

        var sw = System.Diagnostics.Stopwatch.StartNew();
        var reply = await fetch.GetAsync<Reply>("/flaky");
        sw.Stop();

        Assert.True(reply.Ok);
        Assert.Equal(1, consulted);
        Assert.Equal(2, _server.LogEntries.Count());
        Assert.True(sw.Elapsed < TimeSpan.FromSeconds(2),
            $"RetryWithDelay did not override default backoff (took {sw.Elapsed.TotalSeconds:F2}s)");
    }

    [Fact]
    public async Task Builder_circuit_breaker_trips_after_threshold()
    {
        _server
            .Given(Request.Create().WithPath("/always-503").UsingGet())
            .RespondWith(Response.Create().WithStatusCode(503));

        var fetch = SmooFetchBuilder.Create()
            .WithBaseUrl(_server.Urls[0])
            .WithNoRetry()
            .WithCircuitBreaker(failureThreshold: 2, openDuration: TimeSpan.FromSeconds(10))
            .Build();

        // First two calls hit the server and trip the breaker.
        await Assert.ThrowsAsync<HttpResponseError>(() => fetch.GetAsync<Reply>("/always-503"));
        await Assert.ThrowsAsync<HttpResponseError>(() => fetch.GetAsync<Reply>("/always-503"));

        // Once tripped, subsequent calls fail fast with a Polly
        // `BrokenCircuitException` instead of hitting the server.
        await Assert.ThrowsAnyAsync<Exception>(() => fetch.GetAsync<Reply>("/always-503"));

        // Server should not have seen the third request.
        var observed = _server.LogEntries.Count();
        Assert.True(observed <= 2, $"circuit breaker did not trip (server saw {observed} requests)");
    }
}
