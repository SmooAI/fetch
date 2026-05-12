using System.Text.Json;

namespace SmooAI.Fetch;

/// <summary>
/// Fluent builder for <see cref="SmooFetch"/>. Wraps the existing
/// <see cref="SmooFetchOptions"/> + <see cref="SmooFetch.Create(System.Action{SmooFetchOptions})"/>
/// factory so callers get the same ergonomics as the TS / Python / Rust / Go
/// `FetchBuilder` ports.
///
/// <example>
/// <code>
/// var fetch = SmooFetchBuilder.Create()
///     .WithBaseUrl("https://api.example.com")
///     .WithRetry(RetryPolicy.ExponentialBackoff(3))
///     .WithFastFirst(true)
///     .WithOnRejection(ctx => ctx.LastStatus == HttpStatusCode.TooManyRequests
///         ? OnRejectionDecision.RetryWithDelay(TimeSpan.FromSeconds(2))
///         : OnRejectionDecision.Default())
///     .WithHooks(new LifecycleHooks
///     {
///         PreRequest = (req, _) => { /* mutate */ return Task.CompletedTask; },
///     })
///     .WithAuthTokenProvider(async _ => "tok")
///     .Build();
/// </code>
/// </example>
/// </summary>
public sealed class SmooFetchBuilder
{
    private readonly SmooFetchOptions _options = new();

    private SmooFetchBuilder()
    {
    }

    /// <summary>Create a new builder seeded with default options.</summary>
    public static SmooFetchBuilder Create() => new();

    /// <summary>Set the base URL used to resolve relative paths.</summary>
    public SmooFetchBuilder WithBaseUrl(string baseUrl)
    {
        _options.BaseUrl = baseUrl;
        return this;
    }

    /// <summary>Set the retry policy (overrides the default).</summary>
    public SmooFetchBuilder WithRetry(RetryPolicy policy)
    {
        _options.RetryPolicy = policy;
        return this;
    }

    /// <summary>Disable retries.</summary>
    public SmooFetchBuilder WithNoRetry()
    {
        _options.RetryPolicy = RetryPolicy.None;
        return this;
    }

    /// <summary>Set the per-request timeout.</summary>
    public SmooFetchBuilder WithTimeout(TimeSpan timeout)
    {
        _options.Timeout = timeout;
        return this;
    }

    /// <summary>Register an async auth-token provider.</summary>
    public SmooFetchBuilder WithAuthTokenProvider(AuthTokenProvider provider, string scheme = "Bearer")
    {
        _options.AuthTokenProvider = provider;
        _options.AuthScheme = scheme;
        return this;
    }

    /// <summary>Add a default header that is applied to every request.</summary>
    public SmooFetchBuilder WithDefaultHeader(string name, string value)
    {
        _options.DefaultHeaders[name] = value;
        return this;
    }

    /// <summary>Override the JSON serialization options.</summary>
    public SmooFetchBuilder WithJsonOptions(JsonSerializerOptions options)
    {
        _options.JsonOptions = options;
        return this;
    }

    /// <summary>Set lifecycle hooks.</summary>
    public SmooFetchBuilder WithHooks(LifecycleHooks hooks)
    {
        _options.Hooks = hooks;
        return this;
    }

    /// <summary>Register a pre-request hook (composes with existing hooks).</summary>
    public SmooFetchBuilder WithPreRequest(Func<HttpRequestMessage, CancellationToken, Task> hook)
    {
        var existing = _options.Hooks ?? new LifecycleHooks();
        _options.Hooks = new LifecycleHooks
        {
            PreRequest = hook,
            PostRequestOk = existing.PostRequestOk,
            PostRequestErr = existing.PostRequestErr,
        };
        return this;
    }

    /// <summary>Register a post-request success hook (composes with existing hooks).</summary>
    public SmooFetchBuilder WithPostRequestOk(Func<HttpResponseMessage, CancellationToken, Task> hook)
    {
        var existing = _options.Hooks ?? new LifecycleHooks();
        _options.Hooks = new LifecycleHooks
        {
            PreRequest = existing.PreRequest,
            PostRequestOk = hook,
            PostRequestErr = existing.PostRequestErr,
        };
        return this;
    }

    /// <summary>Register a post-request error hook (composes with existing hooks).</summary>
    public SmooFetchBuilder WithPostRequestErr(Func<Exception, CancellationToken, Task> hook)
    {
        var existing = _options.Hooks ?? new LifecycleHooks();
        _options.Hooks = new LifecycleHooks
        {
            PreRequest = existing.PreRequest,
            PostRequestOk = existing.PostRequestOk,
            PostRequestErr = hook,
        };
        return this;
    }

    /// <summary>
    /// Configure a circuit breaker. The retry pipeline is wrapped so consecutive
    /// failures up to <paramref name="failureThreshold"/> trip the breaker open for
    /// <paramref name="openDuration"/>, after which a half-open probe is allowed.
    /// </summary>
    public SmooFetchBuilder WithCircuitBreaker(int failureThreshold, TimeSpan openDuration, int halfOpenSamplingDuration = 1)
    {
        if (failureThreshold < 1)
        {
            throw new ArgumentOutOfRangeException(nameof(failureThreshold), "failureThreshold must be >= 1");
        }

        _options.CircuitBreaker = new CircuitBreakerOptions(failureThreshold, openDuration, halfOpenSamplingDuration);
        return this;
    }

    /// <summary>Disable the circuit breaker (if previously configured).</summary>
    public SmooFetchBuilder WithNoCircuitBreaker()
    {
        _options.CircuitBreaker = null;
        return this;
    }

    /// <summary>
    /// Configure an in-process sliding-window rate limiter. Every outgoing
    /// request must acquire a permit before being dispatched, and rate-limit
    /// rejections flow through the existing retry pipeline (so they are
    /// retried with backoff like any other transient failure). The optional
    /// <paramref name="onRejected"/> callback fires for observability.
    /// </summary>
    public SmooFetchBuilder WithRateLimit(int maxRequests, TimeSpan window, Action<RateLimitRejectedContext>? onRejected = null)
    {
        _options.RateLimiter = new RateLimiterOptions(maxRequests, window)
        {
            OnRejected = onRejected,
        };
        return this;
    }

    /// <summary>Apply a fully-constructed <see cref="RateLimiterOptions"/> instance.</summary>
    public SmooFetchBuilder WithRateLimit(RateLimiterOptions options)
    {
        ArgumentNullException.ThrowIfNull(options);
        _options.RateLimiter = options;
        return this;
    }

    /// <summary>Disable the rate limiter (if previously configured).</summary>
    public SmooFetchBuilder WithNoRateLimit()
    {
        _options.RateLimiter = null;
        return this;
    }

    /// <summary>
    /// Toggle <see cref="RetryPolicy.FastFirst"/> on the currently-configured retry
    /// policy. When the retry policy has no retries configured this re-enables retries
    /// using <see cref="RetryPolicy.Default"/>.
    /// </summary>
    public SmooFetchBuilder WithFastFirst(bool fastFirst = true)
    {
        var current = _options.RetryPolicy.MaxRetries > 0 ? _options.RetryPolicy : RetryPolicy.Default;
        _options.RetryPolicy = current with { FastFirst = fastFirst };
        return this;
    }

    /// <summary>
    /// Register an <see cref="OnRejectionCallback"/> on the currently-configured
    /// retry policy.
    /// </summary>
    public SmooFetchBuilder WithOnRejection(OnRejectionCallback callback)
    {
        ArgumentNullException.ThrowIfNull(callback);
        var current = _options.RetryPolicy.MaxRetries > 0 ? _options.RetryPolicy : RetryPolicy.Default;
        _options.RetryPolicy = current with { OnRejection = callback };
        return this;
    }

    /// <summary>Build the configured <see cref="SmooFetch"/>.</summary>
    public SmooFetch Build()
    {
        return SmooFetch.Create(o =>
        {
            o.BaseUrl = _options.BaseUrl;
            o.RetryPolicy = _options.RetryPolicy;
            o.Timeout = _options.Timeout;
            o.AuthTokenProvider = _options.AuthTokenProvider;
            o.AuthScheme = _options.AuthScheme;
            o.JsonOptions = _options.JsonOptions;
            o.Hooks = _options.Hooks;
            o.CircuitBreaker = _options.CircuitBreaker;
            o.RateLimiter = _options.RateLimiter;
            foreach (var kvp in _options.DefaultHeaders)
            {
                o.DefaultHeaders[kvp.Key] = kvp.Value;
            }
        });
    }
}
