namespace SmooAI.Fetch;

/// <summary>
/// Context supplied to the <see cref="RateLimiterOptions.OnRejected"/> callback.
/// </summary>
public readonly struct RateLimitRejectedContext
{
    /// <summary>The request that was rejected.</summary>
    public HttpRequestMessage Request { get; init; }

    /// <summary>How long the limiter suggests waiting before trying again. May be <see cref="TimeSpan.Zero"/> if unknown.</summary>
    public TimeSpan RetryAfter { get; init; }
}

/// <summary>
/// Configuration for an in-process sliding-window rate limiter. When configured,
/// each outgoing request must acquire a permit from the limiter before being sent.
/// Acquisition is performed inside the retry pipeline so a rejection caused by
/// the limiter is treated like any other transient failure and may be retried.
///
/// Mirrors the cross-port `RateLimitOptions` shape (Rust / Python / Go) with a
/// .NET-idiomatic <c>OnRejected</c> callback for observability.
///
/// <para>
/// On .NET 8 and newer the limiter is implemented via
/// <c>System.Threading.RateLimiting.SlidingWindowRateLimiter</c>. The single
/// limiter instance is shared across all calls on the constructed
/// <see cref="SmooFetch"/>, so state is naturally shared like the Rust/Go ports.
/// </para>
/// </summary>
public sealed record RateLimiterOptions
{
    /// <summary>Maximum number of permits available within <see cref="Window"/>.</summary>
    public int MaxRequests { get; init; }

    /// <summary>Length of the sliding window.</summary>
    public TimeSpan Window { get; init; }

    /// <summary>
    /// Number of sub-segments used by the sliding window. Larger values produce
    /// smoother but more expensive accounting. Defaults to 8 which mirrors the
    /// BCL default and is plenty for typical HTTP traffic.
    /// </summary>
    public int SegmentsPerWindow { get; init; } = 8;

    /// <summary>
    /// Optional callback invoked when the limiter rejects (or would reject)
    /// a request. Fires for every rejection — including the rejections that
    /// the retry pipeline will subsequently retry — so callers can use it for
    /// metrics / logging without owning the retry decision.
    /// </summary>
    public Action<RateLimitRejectedContext>? OnRejected { get; init; }

    /// <summary>
    /// Create a new <see cref="RateLimiterOptions"/> instance with the given limits.
    /// </summary>
    public RateLimiterOptions(int maxRequests, TimeSpan window)
    {
        if (maxRequests < 1)
        {
            throw new ArgumentOutOfRangeException(nameof(maxRequests), "maxRequests must be >= 1");
        }

        if (window <= TimeSpan.Zero)
        {
            throw new ArgumentOutOfRangeException(nameof(window), "window must be greater than zero");
        }

        MaxRequests = maxRequests;
        Window = window;
    }
}
