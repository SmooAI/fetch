using System.Net;
using Polly;
using Polly.Retry;

namespace SmooAI.Fetch;

/// <summary>
/// Declarative retry configuration. Static factory methods produce common policies
/// (exponential backoff with jitter, no retry, custom). Consumed by <see cref="SmooFetch"/>.
/// </summary>
public sealed class RetryPolicy
{
    /// <summary>Maximum retry attempts (not counting the initial call).</summary>
    public int MaxRetries { get; init; }

    /// <summary>Base delay used to compute the wait between retries.</summary>
    public TimeSpan BaseDelay { get; init; } = TimeSpan.FromMilliseconds(500);

    /// <summary>Upper bound clamp for any single retry delay.</summary>
    public TimeSpan MaxDelay { get; init; } = TimeSpan.FromSeconds(30);

    /// <summary>Exponential factor applied to each retry (e.g. 2.0 doubles the delay).</summary>
    public double BackoffFactor { get; init; } = 2.0;

    /// <summary>Enable randomized jitter (+/- <see cref="JitterFraction"/>) on each wait.</summary>
    public bool UseJitter { get; init; } = true;

    /// <summary>Fraction of the computed delay used as +/- jitter range. 0.5 = ±50%.</summary>
    public double JitterFraction { get; init; } = 0.5;

    /// <summary>
    /// When true, retry on transient exceptions (timeouts, DNS failures, socket errors).
    /// Always true for the default policy.
    /// </summary>
    public bool RetryOnTransientExceptions { get; init; } = true;

    /// <summary>
    /// Honor <c>Retry-After</c> header on 429/503 responses. When present, replaces the
    /// computed backoff delay with the server's suggested wait.
    /// </summary>
    public bool HonorRetryAfterHeader { get; init; } = true;

    /// <summary>HTTP status codes that trigger a retry. Defaults to 429 + 5xx.</summary>
    public IReadOnlyCollection<HttpStatusCode> RetryStatusCodes { get; init; } = DefaultRetryStatusCodes;

    private static readonly IReadOnlyCollection<HttpStatusCode> DefaultRetryStatusCodes = new[]
    {
        HttpStatusCode.RequestTimeout,
        (HttpStatusCode)429,
        HttpStatusCode.InternalServerError,
        HttpStatusCode.BadGateway,
        HttpStatusCode.ServiceUnavailable,
        HttpStatusCode.GatewayTimeout,
    };

    /// <summary>A retry policy that never retries.</summary>
    public static RetryPolicy None { get; } = new() { MaxRetries = 0 };

    /// <summary>Default policy: 2 retries, 500 ms base, exponential factor 2, jitter ±50%, honors Retry-After.</summary>
    public static RetryPolicy Default { get; } = new()
    {
        MaxRetries = 2,
        BaseDelay = TimeSpan.FromMilliseconds(500),
        BackoffFactor = 2.0,
        UseJitter = true,
        JitterFraction = 0.5,
        HonorRetryAfterHeader = true,
    };

    /// <summary>Fluent helper: exponential backoff with the given max retries.</summary>
    public static RetryPolicy ExponentialBackoff(int maxRetries, TimeSpan? baseDelay = null, TimeSpan? maxDelay = null)
    {
        return new RetryPolicy
        {
            MaxRetries = maxRetries,
            BaseDelay = baseDelay ?? TimeSpan.FromMilliseconds(500),
            MaxDelay = maxDelay ?? TimeSpan.FromSeconds(30),
            BackoffFactor = 2.0,
            UseJitter = true,
            JitterFraction = 0.5,
            HonorRetryAfterHeader = true,
        };
    }

    internal bool ShouldRetryStatus(HttpStatusCode status)
    {
        foreach (var code in RetryStatusCodes)
        {
            if (code == status)
            {
                return true;
            }
        }

        return false;
    }

    internal TimeSpan ComputeDelay(int attempt, Random rng)
    {
        // attempt is 1-based for the first retry.
        var pow = Math.Pow(BackoffFactor, Math.Max(0, attempt - 1));
        var baseMs = BaseDelay.TotalMilliseconds * pow;
        if (UseJitter && JitterFraction > 0)
        {
            var jitterRange = baseMs * JitterFraction;
            var offset = (rng.NextDouble() * 2 - 1) * jitterRange;
            baseMs += offset;
        }

        if (baseMs < 0)
        {
            baseMs = 0;
        }

        var capped = Math.Min(baseMs, MaxDelay.TotalMilliseconds);
        return TimeSpan.FromMilliseconds(capped);
    }

    internal ResiliencePipeline<HttpResponseMessage> BuildPipeline()
    {
        if (MaxRetries <= 0)
        {
            return ResiliencePipeline<HttpResponseMessage>.Empty;
        }

        var rng = new Random();
        var policy = this;

        return new ResiliencePipelineBuilder<HttpResponseMessage>()
            .AddRetry(new RetryStrategyOptions<HttpResponseMessage>
            {
                MaxRetryAttempts = MaxRetries,
                BackoffType = DelayBackoffType.Constant,
                UseJitter = false,
                ShouldHandle = args =>
                {
                    if (args.Outcome.Exception is not null)
                    {
                        return ValueTask.FromResult(policy.ShouldRetryException(args.Outcome.Exception));
                    }

                    var response = args.Outcome.Result;
                    return ValueTask.FromResult(response is not null && policy.ShouldRetryStatus(response.StatusCode));
                },
                DelayGenerator = args =>
                {
                    var attempt = args.AttemptNumber + 1;
                    TimeSpan delay;
                    if (policy.HonorRetryAfterHeader && args.Outcome.Result is { } resp)
                    {
                        var retryAfter = ReadRetryAfter(resp);
                        if (retryAfter is { } ra)
                        {
                            delay = ra < policy.MaxDelay ? ra : policy.MaxDelay;
                            return ValueTask.FromResult<TimeSpan?>(delay);
                        }
                    }

                    delay = policy.ComputeDelay(attempt, rng);
                    return ValueTask.FromResult<TimeSpan?>(delay);
                },
            })
            .Build();
    }

    private bool ShouldRetryException(Exception ex)
    {
        if (!RetryOnTransientExceptions)
        {
            return false;
        }

        return ex is HttpRequestException
            or TaskCanceledException
            or OperationCanceledException
            or TimeoutException;
    }

    private static TimeSpan? ReadRetryAfter(HttpResponseMessage response)
    {
        var ra = response.Headers.RetryAfter;
        if (ra is null)
        {
            return null;
        }

        if (ra.Delta is { } delta)
        {
            return delta;
        }

        if (ra.Date is { } date)
        {
            var now = DateTimeOffset.UtcNow;
            var wait = date - now;
            if (wait > TimeSpan.Zero)
            {
                return wait;
            }
        }

        return null;
    }
}
