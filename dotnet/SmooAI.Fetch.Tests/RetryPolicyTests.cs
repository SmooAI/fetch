using System.Net;
using SmooAI.Fetch;
using WireMock.RequestBuilders;
using WireMock.ResponseBuilders;
using WireMock.Server;

namespace SmooAI.Fetch.Tests;

public class RetryPolicyTests : IAsyncLifetime
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

    private sealed record Thing(string Ok);

    [Fact]
    public async Task Retries_on_500_then_succeeds()
    {
        _server
            .Given(Request.Create().WithPath("/flaky").UsingGet())
            .InScenario("flaky")
            .WillSetStateTo("first-failed")
            .RespondWith(Response.Create().WithStatusCode(500).WithBody("boom"));

        _server
            .Given(Request.Create().WithPath("/flaky").UsingGet())
            .InScenario("flaky")
            .WhenStateIs("first-failed")
            .WillSetStateTo("second-failed")
            .RespondWith(Response.Create().WithStatusCode(502).WithBody("boom2"));

        _server
            .Given(Request.Create().WithPath("/flaky").UsingGet())
            .InScenario("flaky")
            .WhenStateIs("second-failed")
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"ok\":\"yes\"}"));

        var fetch = SmooFetch.Create(opts =>
        {
            opts.BaseUrl = _server.Urls[0];
            opts.RetryPolicy = new RetryPolicy
            {
                MaxRetries = 3,
                BaseDelay = TimeSpan.FromMilliseconds(1),
                MaxDelay = TimeSpan.FromMilliseconds(5),
                UseJitter = false,
                BackoffFactor = 1.0,
            };
        });

        var result = await fetch.GetAsync<Thing>("/flaky");
        Assert.Equal("yes", result.Ok);
        Assert.Equal(3, _server.LogEntries.Count());
    }

    [Fact]
    public async Task Exhausts_retries_and_throws_last_HttpResponseError()
    {
        _server
            .Given(Request.Create().WithPath("/always-500").UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(500)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"error\":\"broken\"}"));

        var fetch = SmooFetch.Create(opts =>
        {
            opts.BaseUrl = _server.Urls[0];
            opts.RetryPolicy = new RetryPolicy
            {
                MaxRetries = 2,
                BaseDelay = TimeSpan.FromMilliseconds(1),
                UseJitter = false,
                BackoffFactor = 1.0,
            };
        });

        var ex = await Assert.ThrowsAsync<HttpResponseError>(async () => await fetch.GetAsync<Thing>("/always-500"));
        Assert.Equal(HttpStatusCode.InternalServerError, ex.StatusCode);
        // 1 initial + 2 retries = 3 requests
        Assert.Equal(3, _server.LogEntries.Count());
    }

    [Fact]
    public async Task Does_not_retry_on_400()
    {
        _server
            .Given(Request.Create().WithPath("/bad").UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(400)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"error\":\"nope\"}"));

        var fetch = SmooFetch.Create(opts =>
        {
            opts.BaseUrl = _server.Urls[0];
            opts.RetryPolicy = RetryPolicy.ExponentialBackoff(maxRetries: 3);
        });

        await Assert.ThrowsAsync<HttpResponseError>(async () => await fetch.GetAsync<Thing>("/bad"));
        Assert.Single(_server.LogEntries);
    }

    [Fact]
    public async Task Honors_RetryAfter_delta_seconds_on_429()
    {
        _server
            .Given(Request.Create().WithPath("/ratelimited").UsingGet())
            .InScenario("429")
            .WillSetStateTo("retry-now")
            .RespondWith(Response.Create()
                .WithStatusCode(429)
                .WithHeader("Retry-After", "0")
                .WithBody("slow down"));

        _server
            .Given(Request.Create().WithPath("/ratelimited").UsingGet())
            .InScenario("429")
            .WhenStateIs("retry-now")
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"ok\":\"k\"}"));

        var fetch = SmooFetch.Create(opts =>
        {
            opts.BaseUrl = _server.Urls[0];
            opts.RetryPolicy = new RetryPolicy
            {
                MaxRetries = 1,
                BaseDelay = TimeSpan.FromMilliseconds(1),
                MaxDelay = TimeSpan.FromSeconds(2),
                UseJitter = false,
                HonorRetryAfterHeader = true,
            };
        });

        var result = await fetch.GetAsync<Thing>("/ratelimited");
        Assert.Equal("k", result.Ok);
        Assert.Equal(2, _server.LogEntries.Count());
    }

    [Fact]
    public void RetryPolicy_None_has_zero_retries()
    {
        var policy = RetryPolicy.None;
        Assert.Equal(0, policy.MaxRetries);
    }

    [Fact]
    public void ExponentialBackoff_factory_sets_attempt_count()
    {
        var policy = RetryPolicy.ExponentialBackoff(5);
        Assert.Equal(5, policy.MaxRetries);
        Assert.True(policy.UseJitter);
        Assert.True(policy.HonorRetryAfterHeader);
    }
}
