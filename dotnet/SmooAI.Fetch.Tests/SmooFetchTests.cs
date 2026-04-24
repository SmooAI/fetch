using System.Net;
using SmooAI.Fetch;
using WireMock.RequestBuilders;
using WireMock.ResponseBuilders;
using WireMock.Server;

namespace SmooAI.Fetch.Tests;

public class SmooFetchTests : IAsyncLifetime
{
    private WireMockServer _server = null!;
    private SmooFetch _fetch = null!;

    public Task InitializeAsync()
    {
        _server = WireMockServer.Start();
        if (_server.Urls.Length == 0)
        {
            throw new InvalidOperationException("WireMock did not start with any URL bound.");
        }

        _fetch = SmooFetch.Create(opts =>
        {
            opts.BaseUrl = _server.Urls[0];
            opts.RetryPolicy = RetryPolicy.None;
            opts.Timeout = TimeSpan.FromSeconds(5);
        });
        return Task.CompletedTask;
    }

    public Task DisposeAsync()
    {
        _server.Stop();
        _server.Dispose();
        return Task.CompletedTask;
    }

    private sealed record User(string Id, string Name);
    private sealed record CreateUserDto(string Name);

    [Fact]
    public async Task GetAsync_deserializes_json()
    {
        _server
            .Given(Request.Create().WithPath("/users/me").UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"id\":\"u1\",\"name\":\"Brent\"}"));

        var user = await _fetch.GetAsync<User>("/users/me");

        Assert.Equal("u1", user.Id);
        Assert.Equal("Brent", user.Name);
    }

    [Fact]
    public async Task PostAsync_serializes_body_and_deserializes_response()
    {
        _server
            .Given(Request.Create().WithPath("/users").UsingPost().WithBody("*Brent*"))
            .RespondWith(Response.Create()
                .WithStatusCode(201)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"id\":\"u2\",\"name\":\"Brent\"}"));

        var created = await _fetch.PostAsync<CreateUserDto, User>("/users", new CreateUserDto("Brent"));

        Assert.Equal("u2", created.Id);
    }

    [Fact]
    public async Task Non2xx_throws_HttpResponseError_with_status_and_body()
    {
        _server
            .Given(Request.Create().WithPath("/nope").UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(404)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"error\":{\"message\":\"not here\"}}"));

        var ex = await Assert.ThrowsAsync<HttpResponseError>(async () => await _fetch.GetAsync<User>("/nope"));
        Assert.Equal(HttpStatusCode.NotFound, ex.StatusCode);
        Assert.Contains("not here", ex.Body);
        Assert.True(ex.IsJson);
        Assert.Contains("not here", ex.Message);
    }

    [Fact]
    public async Task AuthTokenProvider_injects_bearer_header()
    {
        _server
            .Given(Request.Create().WithPath("/secure").UsingGet()
                .WithHeader("Authorization", "Bearer abc123"))
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"id\":\"x\",\"name\":\"y\"}"));

        var fetch = SmooFetch.Create(opts =>
        {
            opts.BaseUrl = _server.Urls[0];
            opts.RetryPolicy = RetryPolicy.None;
            opts.AuthTokenProvider = _ => Task.FromResult<string?>("abc123");
        });

        var user = await fetch.GetAsync<User>("/secure");
        Assert.Equal("x", user.Id);
    }

    [Fact]
    public async Task AuthTokenProvider_null_token_skips_header()
    {
        _server
            .Given(Request.Create().WithPath("/open").UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"id\":\"o\",\"name\":\"p\"}"));

        var fetch = SmooFetch.Create(opts =>
        {
            opts.BaseUrl = _server.Urls[0];
            opts.RetryPolicy = RetryPolicy.None;
            opts.AuthTokenProvider = _ => Task.FromResult<string?>(null);
        });

        var user = await fetch.GetAsync<User>("/open");
        Assert.Equal("o", user.Id);

        var received = _server.LogEntries.Single();
        Assert.False(received.RequestMessage.Headers!.ContainsKey("Authorization"));
    }

    [Fact]
    public async Task DefaultHeaders_are_applied()
    {
        _server
            .Given(Request.Create().WithPath("/ping").UsingGet()
                .WithHeader("X-Tenant", "smoo"))
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"id\":\"p\",\"name\":\"q\"}"));

        var fetch = SmooFetch.Create(opts =>
        {
            opts.BaseUrl = _server.Urls[0];
            opts.RetryPolicy = RetryPolicy.None;
            opts.DefaultHeaders["X-Tenant"] = "smoo";
        });

        var user = await fetch.GetAsync<User>("/ping");
        Assert.Equal("p", user.Id);
    }

    [Fact]
    public async Task Absolute_url_bypasses_baseurl()
    {
        _server
            .Given(Request.Create().WithPath("/abs").UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"id\":\"a\",\"name\":\"b\"}"));

        var fetch = SmooFetch.Create(opts =>
        {
            opts.BaseUrl = "https://unused.example";
            opts.RetryPolicy = RetryPolicy.None;
        });

        var user = await fetch.GetAsync<User>($"{_server.Urls[0]}/abs");
        Assert.Equal("a", user.Id);
    }

    [Fact]
    public async Task Post_with_no_response_body_returns_on_success()
    {
        _server
            .Given(Request.Create().WithPath("/noop").UsingPost())
            .RespondWith(Response.Create().WithStatusCode(204));

        await _fetch.PostAsync<CreateUserDto>("/noop", new CreateUserDto("x"));
    }
}
