using System.Diagnostics;
using System.Text.RegularExpressions;
using Microsoft.Extensions.DependencyInjection;
using SmooAI.Fetch;
using WireMock.RequestBuilders;
using WireMock.ResponseBuilders;
using WireMock.Server;

namespace SmooAI.Fetch.Tests;

/// <summary>
/// Trace-context propagation on egress, asserted at the WIRE — what the server
/// actually received, not what we believe we set.
///
/// The gap these guard: api-prime EXTRACTS <c>traceparent</c> on ingress, but if
/// nothing INJECTS it on egress every service-to-service call begins a new root
/// trace. Measured 2026-08-14 over three hours: 34,961 traces touched one
/// service, 4 touched two.
///
/// On .NET this is the framework's job, not ours. <c>SocketsHttpHandler</c>
/// installs <c>DiagnosticsHandler</c> as the OUTERMOST stage of its chain, and
/// that stage injects W3C headers via <see cref="DistributedContextPropagator"/>
/// whenever an <see cref="Activity"/> is in play. These tests exist to prove that
/// holds through <see cref="SmooFetch"/>'s per-attempt request cloning, its retry
/// pipeline, and redirects — and to fail loudly if a future handler change
/// (a custom <c>HttpMessageHandler</c>, <c>ActivityHeadersPropagator = null</c>)
/// silently removes it.
/// </summary>
public class TraceContextPropagationTests : IAsyncLifetime
{
    // A valid traceparent: version 00, non-zero 16-byte trace id, non-zero 8-byte
    // parent id. The negative lookaheads are the point — an all-zero id is the
    // failure mode that poisons a downstream trace, so "well-formed" must exclude it.
    private static readonly Regex WellFormed = new(
        "^00-(?![0]{32})[0-9a-f]{32}-(?![0]{16})[0-9a-f]{16}-[0-9a-f]{2}$",
        RegexOptions.Compiled);

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

    private static void Ok(WireMockServer server, string path) =>
        server
            .Given(Request.Create().WithPath(path).UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"ok\":\"yes\"}"));

    private IReadOnlyList<string?> CapturedTraceparents() =>
        _server.LogEntries
            .Select(e => e.RequestMessage.Headers is { } h && h.TryGetValue("traceparent", out var v)
                ? v.FirstOrDefault()
                : null)
            .ToList();

    private SmooFetch Fetch(Action<SmooFetchOptions>? extra = null) =>
        SmooFetch.Create(opts =>
        {
            opts.BaseUrl = _server.Urls[0];
            opts.RetryPolicy = RetryPolicy.None;
            opts.Timeout = TimeSpan.FromSeconds(5);
            extra?.Invoke(opts);
        });

    /// <summary>
    /// Production shape: an OpenTelemetry-style <see cref="ActivityListener"/> scoped
    /// to our own source. Scoping matters — a listener that matched every source
    /// would also match "System.Net.Http", changing which code path
    /// DiagnosticsHandler takes and leaking into sibling tests running in parallel.
    /// </summary>
    private static (ActivitySource Source, ActivityListener Listener) Tracing(string name)
    {
        var source = new ActivitySource(name);
        var listener = new ActivityListener
        {
            ShouldListenTo = s => s.Name == name,
            Sample = (ref ActivityCreationOptions<ActivityContext> _) => ActivitySamplingResult.AllDataAndRecorded,
        };
        ActivitySource.AddActivityListener(listener);
        return (source, listener);
    }

    [Fact]
    public async Task Current_activity_is_propagated_as_a_traceparent_header()
    {
        Ok(_server, "/x");
        var (source, listener) = Tracing(nameof(Current_activity_is_propagated_as_a_traceparent_header));
        using (source)
        using (listener)
        {
            using var caller = source.StartActivity("caller");
            Assert.NotNull(caller);

            await Fetch().GetAsync<Thing>("/x");

            var traceparent = Assert.Single(CapturedTraceparents());
            Assert.NotNull(traceparent);
            Assert.Matches(WellFormed, traceparent);
            // Same trace, and a CHILD span — the outbound call is its own span, so
            // the parent-id must not be the caller's own span id.
            Assert.StartsWith($"00-{caller.TraceId.ToHexString()}-", traceparent, StringComparison.Ordinal);
            Assert.DoesNotContain(caller.SpanId.ToHexString(), traceparent, StringComparison.Ordinal);
        }
    }

    [Fact]
    public async Task No_current_activity_means_no_traceparent_header()
    {
        Ok(_server, "/x");
        Activity.Current = null;

        await Fetch().GetAsync<Thing>("/x");

        // Never an all-zero "00-000…-000…-00": a downstream service either rejects
        // that or, worse, adopts it and poisons its own trace. Absent is correct.
        Assert.Null(Assert.Single(CapturedTraceparents()));
    }

    [Fact]
    public async Task Caller_supplied_traceparent_is_never_overwritten()
    {
        const string caller = "00-11111111111111111111111111111111-2222222222222222-01";
        Ok(_server, "/x");
        var (source, listener) = Tracing(nameof(Caller_supplied_traceparent_is_never_overwritten));
        using (source)
        using (listener)
        {
            using var activity = source.StartActivity("caller");
            Assert.NotNull(activity);

            await Fetch(o => o.DefaultHeaders["traceparent"] = caller).GetAsync<Thing>("/x");

            // A client that silently rewrites an intentional header is worse than
            // one that does nothing.
            Assert.Equal(caller, Assert.Single(CapturedTraceparents()));
        }
    }

    [Fact]
    public async Task Each_retry_attempt_carries_its_own_fresh_traceparent()
    {
        _server
            .Given(Request.Create().WithPath("/flaky").UsingGet())
            .InScenario("flaky")
            .WillSetStateTo("failed-once")
            .RespondWith(Response.Create().WithStatusCode(500).WithBody("boom"));
        _server
            .Given(Request.Create().WithPath("/flaky").UsingGet())
            .InScenario("flaky")
            .WhenStateIs("failed-once")
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"ok\":\"yes\"}"));

        var (source, listener) = Tracing(nameof(Each_retry_attempt_carries_its_own_fresh_traceparent));
        using (source)
        using (listener)
        {
            using var caller = source.StartActivity("caller");
            Assert.NotNull(caller);

            await Fetch(o => o.RetryPolicy = new RetryPolicy
            {
                MaxRetries = 3,
                BaseDelay = TimeSpan.FromMilliseconds(1),
                MaxDelay = TimeSpan.FromMilliseconds(5),
                UseJitter = false,
                BackoffFactor = 1.0,
            }).GetAsync<Thing>("/flaky");

            var captured = CapturedTraceparents();
            Assert.Equal(2, captured.Count);
            Assert.All(captured, tp =>
            {
                Assert.NotNull(tp);
                Assert.Matches(WellFormed, tp);
                Assert.StartsWith($"00-{caller.TraceId.ToHexString()}-", tp, StringComparison.Ordinal);
            });

            // The point of the test: attempt two is a NEW span, not a stale copy of
            // attempt one. SmooFetch clones the pristine request per attempt, so the
            // retry re-enters DiagnosticsHandler and gets a fresh parent-id.
            Assert.NotEqual(captured[0], captured[1]);
        }
    }

    [Fact]
    public async Task Redirect_hops_each_carry_a_traceparent()
    {
        _server
            .Given(Request.Create().WithPath("/from").UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(302)
                .WithHeader("Location", $"{_server.Urls[0]}/to"));
        Ok(_server, "/to");

        var (source, listener) = Tracing(nameof(Redirect_hops_each_carry_a_traceparent));
        using (source)
        using (listener)
        {
            using var caller = source.StartActivity("caller");
            Assert.NotNull(caller);

            await Fetch().GetAsync<Thing>("/from");

            var captured = CapturedTraceparents();
            Assert.Equal(2, captured.Count);
            Assert.All(captured, tp =>
            {
                Assert.NotNull(tp);
                Assert.Matches(WellFormed, tp);
                Assert.StartsWith($"00-{caller.TraceId.ToHexString()}-", tp, StringComparison.Ordinal);
            });
        }
    }

    [Fact]
    public async Task Sampled_out_activity_still_propagates_with_the_not_recorded_flag()
    {
        Ok(_server, "/x");
        var source = new ActivitySource(nameof(Sampled_out_activity_still_propagates_with_the_not_recorded_flag));
        var listener = new ActivityListener
        {
            ShouldListenTo = s => s.Name == source.Name,
            // What a head-based sampler does to a span it drops. It still yields a
            // real span context, so the trace stitches across the hop even though
            // nothing is recorded — that is the whole point of the W3C flags byte.
            Sample = (ref ActivityCreationOptions<ActivityContext> _) => ActivitySamplingResult.PropagationData,
        };
        ActivitySource.AddActivityListener(listener);
        using (source)
        using (listener)
        {
            using var caller = source.StartActivity("caller");
            Assert.NotNull(caller);
            Assert.False(caller.Recorded);

            await Fetch().GetAsync<Thing>("/x");

            var traceparent = Assert.Single(CapturedTraceparents());
            Assert.NotNull(traceparent);
            Assert.Matches(WellFormed, traceparent);
            Assert.EndsWith("-00", traceparent, StringComparison.Ordinal);
            Assert.StartsWith($"00-{caller.TraceId.ToHexString()}-", traceparent, StringComparison.Ordinal);
        }

        // Note the sibling case: a sampler returning ActivitySamplingResult.None
        // makes StartActivity return null, leaving Activity.Current null and no
        // header on the wire — covered by No_current_activity_means_no_traceparent_header.
    }

    [Fact]
    public async Task HttpClientFactory_registered_client_also_propagates()
    {
        Ok(_server, "/x");
        var services = new ServiceCollection();
        services.AddSmooFetch(opts =>
        {
            opts.BaseUrl = _server.Urls[0];
            opts.RetryPolicy = RetryPolicy.None;
        });
        using var provider = services.BuildServiceProvider();

        var (source, listener) = Tracing(nameof(HttpClientFactory_registered_client_also_propagates));
        using (source)
        using (listener)
        {
            using var caller = source.StartActivity("caller");
            Assert.NotNull(caller);

            // The DI path builds its handler chain through IHttpClientFactory, which
            // wraps the primary handler in logging handlers. Those sit OUTSIDE the
            // primary handler, so DiagnosticsHandler is still in the chain.
            await provider.GetRequiredService<SmooFetch>().GetAsync<Thing>("/x");

            var traceparent = Assert.Single(CapturedTraceparents());
            Assert.NotNull(traceparent);
            Assert.Matches(WellFormed, traceparent);
            Assert.StartsWith($"00-{caller.TraceId.ToHexString()}-", traceparent, StringComparison.Ordinal);
        }
    }
}
