using System.Net;
using System.Net.Http.Headers;

namespace SmooAI.Fetch;

/// <summary>
/// Exception thrown when an HTTP request completes with a non-success status code.
/// Mirrors the <c>HTTPResponseError</c> contract from the TypeScript @smooai/fetch library:
/// carries the response status, body, and headers so callers can inspect and branch on them.
/// </summary>
public sealed class HttpResponseError : Exception
{
    /// <summary>HTTP status code returned by the server.</summary>
    public HttpStatusCode StatusCode { get; }

    /// <summary>HTTP reason phrase (e.g. "Not Found").</summary>
    public string? ReasonPhrase { get; }

    /// <summary>Raw response body as a string. Empty if the body could not be read.</summary>
    public string Body { get; }

    /// <summary>True when the response's Content-Type indicated JSON and the body parsed successfully.</summary>
    public bool IsJson { get; }

    /// <summary>Response headers, merged with content headers.</summary>
    public IReadOnlyDictionary<string, IReadOnlyList<string>> Headers { get; }

    /// <summary>Request URI that produced this error, when available.</summary>
    public Uri? RequestUri { get; }

    /// <summary>HTTP method of the request that produced this error, when available.</summary>
    public HttpMethod? RequestMethod { get; }

    public HttpResponseError(
        HttpStatusCode statusCode,
        string? reasonPhrase,
        string body,
        bool isJson,
        IReadOnlyDictionary<string, IReadOnlyList<string>> headers,
        Uri? requestUri,
        HttpMethod? requestMethod,
        string? prefix = null)
        : base(BuildMessage(statusCode, reasonPhrase, body, isJson, prefix))
    {
        StatusCode = statusCode;
        ReasonPhrase = reasonPhrase;
        Body = body;
        IsJson = isJson;
        Headers = headers;
        RequestUri = requestUri;
        RequestMethod = requestMethod;
    }

    internal static HttpResponseError FromResponse(HttpResponseMessage response, string body, bool isJson, string? prefix = null)
    {
        var headers = CollectHeaders(response.Headers, response.Content?.Headers);
        return new HttpResponseError(
            response.StatusCode,
            response.ReasonPhrase,
            body,
            isJson,
            headers,
            response.RequestMessage?.RequestUri,
            response.RequestMessage?.Method,
            prefix);
    }

    private static IReadOnlyDictionary<string, IReadOnlyList<string>> CollectHeaders(
        HttpResponseHeaders responseHeaders,
        HttpContentHeaders? contentHeaders)
    {
        var result = new Dictionary<string, IReadOnlyList<string>>(StringComparer.OrdinalIgnoreCase);
        foreach (var header in responseHeaders)
        {
            result[header.Key] = header.Value.ToList();
        }

        if (contentHeaders is not null)
        {
            foreach (var header in contentHeaders)
            {
                result[header.Key] = header.Value.ToList();
            }
        }

        return result;
    }

    private static string BuildMessage(HttpStatusCode status, string? reasonPhrase, string body, bool isJson, string? prefix)
    {
        var extracted = isJson ? TryExtractJsonErrorMessage(body) : null;
        var detail = string.IsNullOrWhiteSpace(extracted)
            ? string.IsNullOrEmpty(body) ? "Unknown error" : body
            : extracted;
        var header = string.IsNullOrEmpty(prefix) ? string.Empty : prefix + "; ";
        var statusLine = $"HTTP Error Response: {(int)status} {reasonPhrase ?? status.ToString()}";
        return $"{header}{detail}; {statusLine}";
    }

    private static string? TryExtractJsonErrorMessage(string body)
    {
        if (string.IsNullOrWhiteSpace(body))
        {
            return null;
        }

        try
        {
            using var doc = System.Text.Json.JsonDocument.Parse(body);
            var root = doc.RootElement;
            if (root.ValueKind != System.Text.Json.JsonValueKind.Object)
            {
                return null;
            }

            if (root.TryGetProperty("error", out var errorProp))
            {
                if (errorProp.ValueKind == System.Text.Json.JsonValueKind.String)
                {
                    return errorProp.GetString();
                }

                if (errorProp.ValueKind == System.Text.Json.JsonValueKind.Object)
                {
                    var parts = new List<string>();
                    if (errorProp.TryGetProperty("type", out var typeProp) && typeProp.ValueKind == System.Text.Json.JsonValueKind.String)
                    {
                        parts.Add($"({typeProp.GetString()})");
                    }

                    if (errorProp.TryGetProperty("code", out var codeProp) && codeProp.ValueKind == System.Text.Json.JsonValueKind.String)
                    {
                        parts.Add($"({codeProp.GetString()})");
                    }

                    if (errorProp.TryGetProperty("message", out var msgProp) && msgProp.ValueKind == System.Text.Json.JsonValueKind.String)
                    {
                        parts.Add(msgProp.GetString()!);
                    }

                    if (parts.Count > 0)
                    {
                        return string.Join(": ", parts);
                    }
                }
            }

            if (root.TryGetProperty("errorMessages", out var msgs) && msgs.ValueKind == System.Text.Json.JsonValueKind.Array)
            {
                var list = new List<string>();
                foreach (var item in msgs.EnumerateArray())
                {
                    if (item.ValueKind == System.Text.Json.JsonValueKind.String)
                    {
                        var s = item.GetString();
                        if (!string.IsNullOrEmpty(s))
                        {
                            list.Add(s);
                        }
                    }
                }

                if (list.Count > 0)
                {
                    return string.Join("; ", list);
                }
            }
        }
        catch (System.Text.Json.JsonException)
        {
            // Not JSON we can parse — fall back to raw body.
        }

        return null;
    }
}

/// <summary>
/// Thrown when a retry budget is exhausted. Carries the final <see cref="HttpResponseError"/>
/// so callers can still see the last server response.
/// </summary>
public sealed class RetryExhaustedError : Exception
{
    /// <summary>The final HTTP response error that triggered retry exhaustion, if any.</summary>
    public HttpResponseError? FinalResponse { get; }

    public RetryExhaustedError(string message, HttpResponseError? finalResponse = null, Exception? inner = null)
        : base(message, inner)
    {
        FinalResponse = finalResponse;
    }
}
