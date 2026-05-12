namespace SmooAI.Fetch;

/// <summary>
/// Lifecycle hooks invoked before / after each HTTP request. Mirrors the
/// `LifecycleHooks` types in the TS / Python / Rust / Go ports.
/// </summary>
public sealed class LifecycleHooks
{
    /// <summary>
    /// Invoked just before the request is sent (after default headers and the
    /// auth-token provider have been applied, but before the underlying
    /// <see cref="HttpClient"/> sends it). May mutate the request.
    /// </summary>
    public Func<HttpRequestMessage, CancellationToken, Task>? PreRequest { get; init; }

    /// <summary>
    /// Invoked after a successful (2xx) response. Receives the raw
    /// <see cref="HttpResponseMessage"/>. The response is still owned by the caller —
    /// hooks should not dispose it.
    /// </summary>
    public Func<HttpResponseMessage, CancellationToken, Task>? PostRequestOk { get; init; }

    /// <summary>
    /// Invoked when a request errors out (after retries are exhausted). Receives the
    /// terminal exception. The hook cannot replace the exception in .NET (mirrors a
    /// minimal logging hook surface); to substitute the error, throw a new one and the
    /// caller will observe it.
    /// </summary>
    public Func<Exception, CancellationToken, Task>? PostRequestErr { get; init; }
}
