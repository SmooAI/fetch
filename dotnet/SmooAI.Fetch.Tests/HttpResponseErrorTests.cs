using System.Net;
using SmooAI.Fetch;

namespace SmooAI.Fetch.Tests;

public class HttpResponseErrorTests
{
    [Fact]
    public void Message_includes_nested_json_error_object()
    {
        var err = new HttpResponseError(
            HttpStatusCode.UnprocessableEntity,
            "Unprocessable Entity",
            body: "{\"error\":{\"type\":\"validation_error\",\"code\":\"missing_field\",\"message\":\"name is required\"}}",
            isJson: true,
            headers: new Dictionary<string, IReadOnlyList<string>>(),
            requestUri: null,
            requestMethod: null);

        Assert.Contains("(validation_error)", err.Message);
        Assert.Contains("(missing_field)", err.Message);
        Assert.Contains("name is required", err.Message);
        Assert.Contains("422", err.Message);
    }

    [Fact]
    public void Message_includes_errorMessages_array()
    {
        var err = new HttpResponseError(
            HttpStatusCode.BadRequest,
            "Bad Request",
            body: "{\"errorMessages\":[\"a is bad\",\"b is bad\"]}",
            isJson: true,
            headers: new Dictionary<string, IReadOnlyList<string>>(),
            requestUri: null,
            requestMethod: null);

        Assert.Contains("a is bad; b is bad", err.Message);
    }

    [Fact]
    public void Message_falls_back_to_raw_body_for_non_json()
    {
        var err = new HttpResponseError(
            HttpStatusCode.BadGateway,
            "Bad Gateway",
            body: "<html>upstream</html>",
            isJson: false,
            headers: new Dictionary<string, IReadOnlyList<string>>(),
            requestUri: null,
            requestMethod: null);

        Assert.Contains("<html>upstream</html>", err.Message);
        Assert.Contains("502", err.Message);
    }

    [Fact]
    public void Message_handles_empty_body()
    {
        var err = new HttpResponseError(
            HttpStatusCode.InternalServerError,
            "Internal Server Error",
            body: string.Empty,
            isJson: false,
            headers: new Dictionary<string, IReadOnlyList<string>>(),
            requestUri: null,
            requestMethod: null);

        Assert.Contains("Unknown error", err.Message);
        Assert.Contains("500", err.Message);
    }
}
