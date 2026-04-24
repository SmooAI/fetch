using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Logging.Abstractions;
using Polly;

namespace SmooAI.Fetch;

/// <summary>
/// Resilient HTTP client with Polly-based retry, per-request timeout, typed JSON
/// request/response helpers, auth token injection, and <see cref="HttpResponseError"/> on non-2xx.
///
/// Prefer constructing via <see cref="Create(Action{SmooFetchOptions})"/> for standalone use,
/// or register via <see cref="ServiceCollectionExtensions.AddSmooFetch(Microsoft.Extensions.DependencyInjection.IServiceCollection, Action{SmooFetchOptions}?, string)"/>
/// for DI scenarios with <see cref="IHttpClientFactory"/>.
/// </summary>
public sealed class SmooFetch
{
    private readonly HttpClient _httpClient;
    private readonly bool _ownsHttpClient;
    private readonly SmooFetchOptions _options;
    private readonly ILogger<SmooFetch> _logger;
    private readonly ResiliencePipeline<HttpResponseMessage> _pipeline;

    /// <summary>Constant used when registering the typed client with <see cref="IHttpClientFactory"/>.</summary>
    public const string DefaultHttpClientName = "SmooAI.Fetch";

    internal SmooFetch(HttpClient httpClient, bool ownsHttpClient, SmooFetchOptions options, ILogger<SmooFetch>? logger)
    {
        _httpClient = httpClient ?? throw new ArgumentNullException(nameof(httpClient));
        _ownsHttpClient = ownsHttpClient;
        _options = options ?? throw new ArgumentNullException(nameof(options));
        _logger = logger ?? NullLogger<SmooFetch>.Instance;
        _pipeline = options.RetryPolicy.BuildPipeline();
    }

    /// <summary>
    /// Create a standalone <see cref="SmooFetch"/> with a self-managed <see cref="HttpClient"/>.
    /// For production / DI scenarios, prefer the <see cref="IHttpClientFactory"/> extension.
    /// </summary>
    public static SmooFetch Create(Action<SmooFetchOptions>? configure = null)
    {
        var options = new SmooFetchOptions();
        configure?.Invoke(options);
        if (options.RequireHttpClientFactory)
        {
            throw new InvalidOperationException(
                "SmooFetchOptions.RequireHttpClientFactory is true — register via AddSmooFetch and resolve from DI instead of Create().");
        }

        var handler = new HttpClientHandler();
        var client = new HttpClient(handler) { Timeout = System.Threading.Timeout.InfiniteTimeSpan };
        return new SmooFetch(client, ownsHttpClient: true, options, logger: null);
    }

    internal static SmooFetch CreateFromFactory(HttpClient client, SmooFetchOptions options, ILogger<SmooFetch> logger)
    {
        // HttpClientFactory-managed clients must not have their Timeout set (we enforce it per-request via CTS).
        client.Timeout = System.Threading.Timeout.InfiniteTimeSpan;
        return new SmooFetch(client, ownsHttpClient: false, options, logger);
    }

    /// <summary>
    /// GET and deserialize the JSON body to <typeparamref name="TResponse"/>.
    /// Throws <see cref="HttpResponseError"/> on non-2xx.
    /// </summary>
    public Task<TResponse> GetAsync<TResponse>(string path, CancellationToken cancellationToken = default)
        => SendJsonAsync<TResponse>(HttpMethod.Get, path, body: null, hasBody: false, cancellationToken);

    /// <summary>POST a JSON body and deserialize the JSON response.</summary>
    public Task<TResponse> PostAsync<TRequest, TResponse>(string path, TRequest body, CancellationToken cancellationToken = default)
        => SendJsonAsync<TResponse>(HttpMethod.Post, path, body, hasBody: true, cancellationToken);

    /// <summary>POST a JSON body with no meaningful response payload.</summary>
    public Task PostAsync<TRequest>(string path, TRequest body, CancellationToken cancellationToken = default)
        => SendJsonNoResponseAsync(HttpMethod.Post, path, body, hasBody: true, cancellationToken);

    /// <summary>PUT a JSON body and deserialize the JSON response.</summary>
    public Task<TResponse> PutAsync<TRequest, TResponse>(string path, TRequest body, CancellationToken cancellationToken = default)
        => SendJsonAsync<TResponse>(HttpMethod.Put, path, body, hasBody: true, cancellationToken);

    /// <summary>PATCH a JSON body and deserialize the JSON response.</summary>
    public Task<TResponse> PatchAsync<TRequest, TResponse>(string path, TRequest body, CancellationToken cancellationToken = default)
        => SendJsonAsync<TResponse>(new HttpMethod("PATCH"), path, body, hasBody: true, cancellationToken);

    /// <summary>DELETE with no request body; returns the deserialized JSON response.</summary>
    public Task<TResponse> DeleteAsync<TResponse>(string path, CancellationToken cancellationToken = default)
        => SendJsonAsync<TResponse>(HttpMethod.Delete, path, body: null, hasBody: false, cancellationToken);

    /// <summary>DELETE with no request body and no response payload.</summary>
    public Task DeleteAsync(string path, CancellationToken cancellationToken = default)
        => SendJsonNoResponseAsync(HttpMethod.Delete, path, body: null, hasBody: false, cancellationToken);

    /// <summary>
    /// Low-level send. Applies the configured retry policy, timeout, default headers, and auth token.
    /// Callers own the returned <see cref="HttpResponseMessage"/> (its content and disposal).
    /// Does not throw <see cref="HttpResponseError"/>; inspect <see cref="HttpResponseMessage.IsSuccessStatusCode"/>.
    /// </summary>
    public async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        await PrepareRequestAsync(request, cancellationToken).ConfigureAwait(false);

        var response = await _pipeline.ExecuteAsync(async ct =>
        {
            using var linkedCts = CancellationTokenSource.CreateLinkedTokenSource(ct);
            linkedCts.CancelAfter(_options.Timeout);
            var attemptRequest = await CloneRequestAsync(request).ConfigureAwait(false);
            try
            {
                return await _httpClient.SendAsync(attemptRequest, HttpCompletionOption.ResponseHeadersRead, linkedCts.Token).ConfigureAwait(false);
            }
            finally
            {
                attemptRequest.Dispose();
            }
        }, cancellationToken).ConfigureAwait(false);

        return response;
    }

    private async Task<TResponse> SendJsonAsync<TResponse>(
        HttpMethod method,
        string path,
        object? body,
        bool hasBody,
        CancellationToken cancellationToken)
    {
        using var request = BuildRequest(method, path, body, hasBody);
        using var response = await SendAsync(request, cancellationToken).ConfigureAwait(false);
        return await ReadJsonResponseAsync<TResponse>(response, cancellationToken).ConfigureAwait(false);
    }

    private async Task SendJsonNoResponseAsync(
        HttpMethod method,
        string path,
        object? body,
        bool hasBody,
        CancellationToken cancellationToken)
    {
        using var request = BuildRequest(method, path, body, hasBody);
        using var response = await SendAsync(request, cancellationToken).ConfigureAwait(false);
        if (!response.IsSuccessStatusCode)
        {
            await ThrowHttpResponseErrorAsync(response, cancellationToken).ConfigureAwait(false);
        }
    }

    private HttpRequestMessage BuildRequest(HttpMethod method, string path, object? body, bool hasBody)
    {
        var uri = ResolveUri(path);
        var request = new HttpRequestMessage(method, uri);

        if (hasBody)
        {
            var json = body is null
                ? "null"
                : JsonSerializer.Serialize(body, body.GetType(), _options.JsonOptions);
            request.Content = new StringContent(json, Encoding.UTF8, "application/json");
        }

        return request;
    }

    private Uri ResolveUri(string path)
    {
        if (Uri.TryCreate(path, UriKind.Absolute, out var absolute)
            && (absolute.Scheme == Uri.UriSchemeHttp || absolute.Scheme == Uri.UriSchemeHttps))
        {
            return absolute;
        }

        if (string.IsNullOrEmpty(_options.BaseUrl))
        {
            throw new InvalidOperationException(
                $"Relative path '{path}' cannot be resolved because SmooFetchOptions.BaseUrl is not set.");
        }

        var baseUri = new Uri(EnsureTrailingSlash(_options.BaseUrl), UriKind.Absolute);
        var relative = path.StartsWith('/') ? path[1..] : path;
        return new Uri(baseUri, relative);
    }

    private static string EnsureTrailingSlash(string url)
    {
        return url.EndsWith('/') ? url : url + "/";
    }

    private async Task PrepareRequestAsync(HttpRequestMessage request, CancellationToken cancellationToken)
    {
        foreach (var kvp in _options.DefaultHeaders)
        {
            if (!request.Headers.Contains(kvp.Key))
            {
                request.Headers.TryAddWithoutValidation(kvp.Key, kvp.Value);
            }
        }

        if (_options.AuthTokenProvider is { } provider && request.Headers.Authorization is null)
        {
            var token = await provider(cancellationToken).ConfigureAwait(false);
            if (!string.IsNullOrEmpty(token))
            {
                request.Headers.Authorization = new AuthenticationHeaderValue(_options.AuthScheme, token);
            }
        }
    }

    private static async Task<HttpRequestMessage> CloneRequestAsync(HttpRequestMessage source)
    {
        var clone = new HttpRequestMessage(source.Method, source.RequestUri)
        {
            Version = source.Version,
            VersionPolicy = source.VersionPolicy,
        };

        foreach (var header in source.Headers)
        {
            clone.Headers.TryAddWithoutValidation(header.Key, header.Value);
        }

        foreach (var option in source.Options)
        {
            clone.Options.TryAdd(option.Key, option.Value);
        }

        if (source.Content is not null)
        {
            var bytes = await source.Content.ReadAsByteArrayAsync().ConfigureAwait(false);
            var cloneContent = new ByteArrayContent(bytes);
            foreach (var header in source.Content.Headers)
            {
                cloneContent.Headers.TryAddWithoutValidation(header.Key, header.Value);
            }

            clone.Content = cloneContent;
        }

        return clone;
    }

    private async Task<TResponse> ReadJsonResponseAsync<TResponse>(HttpResponseMessage response, CancellationToken cancellationToken)
    {
        if (!response.IsSuccessStatusCode)
        {
            await ThrowHttpResponseErrorAsync(response, cancellationToken).ConfigureAwait(false);
        }

        // 204 No Content / empty body support for reference types.
        if (response.StatusCode == System.Net.HttpStatusCode.NoContent || response.Content.Headers.ContentLength == 0)
        {
            return default!;
        }

        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            var result = await JsonSerializer.DeserializeAsync<TResponse>(stream, _options.JsonOptions, cancellationToken).ConfigureAwait(false);
            return result!;
        }
        catch (JsonException ex)
        {
            var body = await SafeReadBodyAsync(response).ConfigureAwait(false);
            throw new HttpResponseError(
                response.StatusCode,
                response.ReasonPhrase,
                body,
                isJson: false,
                headers: BuildHeadersView(response),
                requestUri: response.RequestMessage?.RequestUri,
                requestMethod: response.RequestMessage?.Method,
                prefix: $"Response JSON deserialization failed: {ex.Message}");
        }
    }

    private static async Task ThrowHttpResponseErrorAsync(HttpResponseMessage response, CancellationToken cancellationToken)
    {
        var body = await SafeReadBodyAsync(response, cancellationToken).ConfigureAwait(false);
        var contentType = response.Content.Headers.ContentType?.MediaType ?? string.Empty;
        var isJson = contentType.Contains("json", StringComparison.OrdinalIgnoreCase);
        throw HttpResponseError.FromResponse(response, body, isJson);
    }

    private static async Task<string> SafeReadBodyAsync(HttpResponseMessage response, CancellationToken cancellationToken = default)
    {
        try
        {
            return await response.Content.ReadAsStringAsync(cancellationToken).ConfigureAwait(false);
        }
        catch
        {
            return string.Empty;
        }
    }

    private static IReadOnlyDictionary<string, IReadOnlyList<string>> BuildHeadersView(HttpResponseMessage response)
    {
        var result = new Dictionary<string, IReadOnlyList<string>>(StringComparer.OrdinalIgnoreCase);
        foreach (var header in response.Headers)
        {
            result[header.Key] = header.Value.ToList();
        }

        foreach (var header in response.Content.Headers)
        {
            result[header.Key] = header.Value.ToList();
        }

        return result;
    }
}
