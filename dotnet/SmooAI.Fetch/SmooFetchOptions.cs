using System.Text.Json;

namespace SmooAI.Fetch;

/// <summary>
/// Delegate that resolves an auth token (e.g. Bearer) at request time.
/// Return null to skip auth header injection for a particular call.
/// </summary>
public delegate Task<string?> AuthTokenProvider(CancellationToken cancellationToken = default);

/// <summary>
/// Configuration options for <see cref="SmooFetch"/>.
/// </summary>
public sealed class SmooFetchOptions
{
    /// <summary>Base URL applied to relative paths. Optional; absolute URLs bypass it.</summary>
    public string? BaseUrl { get; set; }

    /// <summary>Retry policy. Defaults to <see cref="RetryPolicy.Default"/>.</summary>
    public RetryPolicy RetryPolicy { get; set; } = RetryPolicy.Default;

    /// <summary>Per-request timeout. Defaults to 10 seconds, matching the TS implementation.</summary>
    public TimeSpan Timeout { get; set; } = TimeSpan.FromSeconds(10);

    /// <summary>Optional auth token provider. When set, the returned token is added as <c>Authorization: Bearer {token}</c>.</summary>
    public AuthTokenProvider? AuthTokenProvider { get; set; }

    /// <summary>Name of the auth scheme prepended to the token. Defaults to <c>Bearer</c>.</summary>
    public string AuthScheme { get; set; } = "Bearer";

    /// <summary>
    /// Default headers added to every request. Request-level headers override these.
    /// </summary>
    public IDictionary<string, string> DefaultHeaders { get; } = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);

    /// <summary>JSON serialization options used for request + response payloads.</summary>
    public JsonSerializerOptions JsonOptions { get; set; } = new(JsonSerializerDefaults.Web);

    /// <summary>
    /// When true, a <see cref="System.Net.Http.IHttpClientFactory"/>-managed client
    /// is required and constructor injection will throw if one is not supplied.
    /// Defaults to false so library consumers can still use <see cref="SmooFetch.Create(Action{SmooFetchOptions})"/>.
    /// </summary>
    public bool RequireHttpClientFactory { get; set; }
}
