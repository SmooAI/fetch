using System.Diagnostics;
using SmooAI.Fetch;

namespace SmooAI.Fetch.Tests;

/// <summary>
/// Mirrors the Rust <c>connect_timeout_tests.rs</c> port: a connect to a non-routable
/// black-hole IP with a short ConnectTimeout must fail in ~that window, not stall until
/// the whole-request Timeout.
/// </summary>
public class ConnectTimeoutTests
{
    // RFC 5737 / non-routable black hole — SYNs get dropped, so connect hangs until timeout.
    private const string BlackHoleUrl = "http://10.255.255.1:80/";

    private sealed record Reply(bool Ok);

    [Fact]
    public async Task ConnectTimeout_fails_fast_before_whole_timeout()
    {
        var fetch = SmooFetchBuilder.Create()
            .WithNoRetry()
            .WithConnectTimeout(TimeSpan.FromMilliseconds(500))
            .WithTimeout(TimeSpan.FromSeconds(5))
            .Build();

        var sw = Stopwatch.StartNew();
        await Assert.ThrowsAnyAsync<Exception>(() => fetch.GetAsync<Reply>(BlackHoleUrl));
        sw.Stop();

        // ~0.5s connect timeout, not the 5s whole-request timeout. Generous ceiling for CI jitter.
        Assert.True(sw.Elapsed < TimeSpan.FromSeconds(3), $"expected fast connect-timeout failure, took {sw.Elapsed}");
    }
}
