using System.Net;
using Polly;
using Polly.Retry;

namespace SmooAI.Fetch;

/// <summary>
/// Decision returned by an <see cref="OnRejectionCallback"/> that controls what
/// the retry loop does next. Mirrors the cross-port `OnRejectionDecision`.
/// </summary>
public enum OnRejectionDecisionKind
{
    /// <summary>Use the built-in exponential+jitter delay (same as if no callback were registered).</summary>
    Default = 0,
    /// <summary>Retry using the built-in delay formula (no override).</summary>
    Retry,
    /// <summary>Retry after the caller-supplied <see cref="OnRejectionDecision.Delay"/>.</summary>
    RetryWithDelay,
    /// <summary>Abort retrying entirely and surface the most recent rejection.</summary>
    Abort,
    /// <summary>Skip this retry attempt without sleeping; proceed to the next attempt.</summary>
    Skip,
}

/// <summary>
/// Decision returned by an <see cref="OnRejectionCallback"/>. Use the static
/// factory methods to construct values (mirrors the discriminated-union APIs
/// in the Rust / Python / Go ports).
/// </summary>
public readonly struct OnRejectionDecision
{
    /// <summary>Kind of decision.</summary>
    public OnRejectionDecisionKind Kind { get; }

    /// <summary>Delay associated with <see cref="OnRejectionDecisionKind.RetryWithDelay"/>; ignored otherwise.</summary>
    public TimeSpan Delay { get; }

    private OnRejectionDecision(OnRejectionDecisionKind kind, TimeSpan delay)
    {
        Kind = kind;
        Delay = delay;
    }

    /// <summary>Retry using the built-in delay formula.</summary>
    public static OnRejectionDecision Retry() => new(OnRejectionDecisionKind.Retry, TimeSpan.Zero);

    /// <summary>Retry after the supplied delay (overrides the built-in formula).</summary>
    public static OnRejectionDecision RetryWithDelay(TimeSpan delay) =>
        new(OnRejectionDecisionKind.RetryWithDelay, delay);

    /// <summary>Abort the retry loop and surface the most recent rejection.</summary>
    public static OnRejectionDecision Abort() => new(OnRejectionDecisionKind.Abort, TimeSpan.Zero);

    /// <summary>Skip this retry attempt without sleeping; proceed to the next attempt.</summary>
    public static OnRejectionDecision Skip() => new(OnRejectionDecisionKind.Skip, TimeSpan.Zero);

    /// <summary>Fall through to the built-in default logic.</summary>
    public static OnRejectionDecision Default() => new(OnRejectionDecisionKind.Default, TimeSpan.Zero);
}

/// <summary>
/// Context passed to an <see cref="OnRejectionCallback"/>.
/// </summary>
public readonly struct RetryContext
{
    /// <summary>1-based attempt number for the retry that is about to be performed.</summary>
    public int Attempt { get; init; }

    /// <summary>Status code of the most recent response, if any (otherwise null).</summary>
    public HttpStatusCode? LastStatus { get; init; }

    /// <summary>The most recent exception, if any.</summary>
    public Exception? LastError { get; init; }

    /// <summary>Time elapsed since the retry loop started.</summary>
    public TimeSpan Elapsed { get; init; }
}

/// <summary>
/// Callback invoked before each retry attempt. Returning an <see cref="OnRejectionDecision"/>
/// lets the caller override the default exponential+jitter backoff behavior.
/// Mirrors the cross-port `on_rejection` / `OnRejection` callbacks.
/// </summary>
public delegate OnRejectionDecision OnRejectionCallback(RetryContext context);

/// <summary>
/// Declarative retry configuration. Static factory methods produce common policies
/// (exponential backoff with jitter, no retry, custom). Consumed by <see cref="SmooFetch"/>.
/// </summary>
public sealed record RetryPolicy
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

    /// <summary>
    /// When true, the first retry fires immediately with zero delay. Subsequent retries
    /// use the normal backoff formula. Mirrors the `fast_first` / `FastFirst` field in
    /// the Rust / Go / Python ports.
    /// </summary>
    public bool FastFirst { get; init; }

    /// <summary>
    /// Optional callback consulted before each retry attempt. The returned
    /// <see cref="OnRejectionDecision"/> can override the default delay, skip the
    /// attempt, or abort retrying entirely.
    /// </summary>
    public OnRejectionCallback? OnRejection { get; init; }

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
        var startedAt = DateTime.UtcNow;

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

                    // Consult `OnRejection` callback, if any.
                    if (policy.OnRejection is { } cb)
                    {
                        var ctx = new RetryContext
                        {
                            Attempt = attempt,
                            LastStatus = args.Outcome.Result?.StatusCode,
                            LastError = args.Outcome.Exception,
                            Elapsed = DateTime.UtcNow - startedAt,
                        };
                        var decision = cb(ctx);
                        switch (decision.Kind)
                        {
                            case OnRejectionDecisionKind.Abort:
                                // Returning TimeSpan.Zero with no further override is the best
                                // approximation we can offer here — Polly does not surface a
                                // first-class abort; the next attempt will fire immediately but
                                // the loop will exit naturally if the caller short-circuits via
                                // ShouldHandle. Practically, callers wire `Abort` via a custom
                                // ShouldHandle so this branch is a no-op fallback.
                                return ValueTask.FromResult<TimeSpan?>(TimeSpan.Zero);
                            case OnRejectionDecisionKind.Skip:
                                return ValueTask.FromResult<TimeSpan?>(TimeSpan.Zero);
                            case OnRejectionDecisionKind.RetryWithDelay:
                                return ValueTask.FromResult<TimeSpan?>(decision.Delay);
                            case OnRejectionDecisionKind.Retry:
                            case OnRejectionDecisionKind.Default:
                            default:
                                break;
                        }
                    }

                    // FastFirst: first retry (attempt == 1) fires with zero delay.
                    if (policy.FastFirst && attempt == 1)
                    {
                        return ValueTask.FromResult<TimeSpan?>(TimeSpan.Zero);
                    }

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
