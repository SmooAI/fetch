using System.Diagnostics;
using System.Text.Json;
using SmooAI.Fetch;

namespace SmooAI.Fetch.Tests;

/// <summary>
/// A connect to a non-routable black-hole IP with a short ConnectTimeout must fail in
/// ~that window, not stall until the whole-request Timeout.
///
/// The knobs come from spec/connect-timeout-corpus.json, shared with the other four
/// ports (copied next to the test assembly by the csproj) — see the corpus for why
/// they are not inlined here.
/// </summary>
public class ConnectTimeoutTests
{
    private sealed record Reply(bool Ok);

    private sealed record Corpus(string BlackHoleUrl, int ConnectTimeoutMs, int WholeRequestTimeoutMs, int MaxElapsedMs);

    private static Corpus LoadCorpus()
    {
        var json = File.ReadAllText("connect-timeout-corpus.json");
        var corpus = JsonSerializer.Deserialize<Corpus>(json, new JsonSerializerOptions { PropertyNameCaseInsensitive = true });
        Assert.NotNull(corpus);
        Assert.False(string.IsNullOrEmpty(corpus!.BlackHoleUrl), "connect-timeout corpus is missing blackHoleUrl");
        Assert.True(corpus.ConnectTimeoutMs > 0 && corpus.MaxElapsedMs > 0, "connect-timeout corpus is missing knobs");
        return corpus;
    }

    [Fact]
    public async Task ConnectTimeout_fails_fast_before_whole_timeout()
    {
        var corpus = LoadCorpus();

        var fetch = SmooFetchBuilder.Create()
            .WithNoRetry()
            .WithConnectTimeout(TimeSpan.FromMilliseconds(corpus.ConnectTimeoutMs))
            .WithTimeout(TimeSpan.FromMilliseconds(corpus.WholeRequestTimeoutMs))
            .Build();

        var sw = Stopwatch.StartNew();
        await Assert.ThrowsAnyAsync<Exception>(() => fetch.GetAsync<Reply>(corpus.BlackHoleUrl));
        sw.Stop();

        Assert.True(
            sw.Elapsed < TimeSpan.FromMilliseconds(corpus.MaxElapsedMs),
            $"expected fast connect-timeout failure (<{corpus.MaxElapsedMs}ms), took {sw.ElapsedMilliseconds}ms");
    }
}
