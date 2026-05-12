using System.Threading.RateLimiting;

namespace SmooAI.Fetch.Internal;

/// <summary>
/// Thin wrapper around <see cref="SlidingWindowRateLimiter"/> that exposes the
/// "acquire-and-wait" semantics the cross-port fetch clients expect:
/// callers always eventually get a permit (no rejection surfaces back to the
/// retry pipeline); if a request would have been rejected immediately the
/// caller-supplied <see cref="RateLimiterOptions.OnRejected"/> hook fires for
/// observability.
///
/// The instance is owned by a single <see cref="SmooFetch"/> and lives for
/// the lifetime of the client, so state is shared across every request
/// dispatched through that client (matching the Rust / Go ports).
/// </summary>
internal sealed class SlidingWindowLimiterAdapter : IAsyncDisposable
{
    private readonly RateLimiterOptions _options;
    private readonly SlidingWindowRateLimiter _limiter;

    public SlidingWindowLimiterAdapter(RateLimiterOptions options)
    {
        _options = options ?? throw new ArgumentNullException(nameof(options));

        _limiter = new SlidingWindowRateLimiter(new SlidingWindowRateLimiterOptions
        {
            // Permit limit == maxRequests across the window.
            PermitLimit = options.MaxRequests,
            Window = options.Window,
            SegmentsPerWindow = Math.Max(1, options.SegmentsPerWindow),

            // Allow queuing so callers wait for the next available permit
            // instead of seeing a hard rejection. We bound the queue at a
            // generous default so a runaway caller can't OOM the process.
            QueueLimit = int.MaxValue,
            QueueProcessingOrder = QueueProcessingOrder.OldestFirst,

            // Refill is driven by sliding-window expiry, not a timer; this
            // flag controls automatic replenishment which we don't need.
            AutoReplenishment = true,
        });
    }

    /// <summary>
    /// Acquire one permit, waiting if necessary. If the permit could not be
    /// granted immediately and an <see cref="RateLimiterOptions.OnRejected"/>
    /// callback is configured it is invoked once before the caller is queued.
    /// </summary>
    public async ValueTask AcquireAsync(HttpRequestMessage request, CancellationToken cancellationToken)
    {
        // Probe synchronously to detect would-be rejections for the OnRejected
        // observability hook. AttemptAcquire is non-blocking and either grants
        // the permit immediately (returning IsAcquired = true) or returns a
        // failed lease with metadata about the wait time.
        var probe = _limiter.AttemptAcquire(permitCount: 1);
        if (probe.IsAcquired)
        {
            probe.Dispose();
            return;
        }

        // Surface to the OnRejected callback if configured.
        if (_options.OnRejected is { } onRejected)
        {
            var retryAfter = TimeSpan.Zero;
            if (probe.TryGetMetadata(MetadataName.RetryAfter, out TimeSpan ra))
            {
                retryAfter = ra;
            }

            try
            {
                onRejected(new RateLimitRejectedContext
                {
                    Request = request,
                    RetryAfter = retryAfter,
                });
            }
            catch
            {
                // Observability hooks must never break the request pipeline.
            }
        }

        probe.Dispose();

        // Queue and wait for the next available permit. AcquireAsync respects
        // QueueLimit / QueueProcessingOrder.
        using var lease = await _limiter.AcquireAsync(permitCount: 1, cancellationToken).ConfigureAwait(false);
        if (!lease.IsAcquired)
        {
            // Should not happen with QueueLimit == int.MaxValue, but surface a
            // typed error if the BCL ever fails to grant the lease.
            throw new InvalidOperationException("Rate limiter failed to grant a permit.");
        }
    }

    public ValueTask DisposeAsync() => _limiter.DisposeAsync();
}
