namespace SmooAI.Fetch;

/// <summary>
/// Configuration for an in-process circuit breaker. When configured, consecutive
/// failures up to <see cref="FailureThreshold"/> trip the breaker open for
/// <see cref="OpenDuration"/>, after which a half-open probe is allowed.
///
/// Mirrors the cross-port `CircuitBreakerOptions` shape (Rust / Python / Go).
/// </summary>
public sealed record CircuitBreakerOptions(
    int FailureThreshold,
    TimeSpan OpenDuration,
    int HalfOpenSamplingDuration = 1);
