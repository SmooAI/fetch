using Microsoft.Extensions.DependencyInjection;
using SmooAI.Fetch;
using WireMock.RequestBuilders;
using WireMock.ResponseBuilders;
using WireMock.Server;

namespace SmooAI.Fetch.Tests;

public class DependencyInjectionTests : IAsyncLifetime
{
    private WireMockServer _server = null!;

    public Task InitializeAsync()
    {
        _server = WireMockServer.Start();
        return Task.CompletedTask;
    }

    public Task DisposeAsync()
    {
        _server.Stop();
        _server.Dispose();
        return Task.CompletedTask;
    }

    private sealed record Thing(string Value);

    [Fact]
    public async Task AddSmooFetch_registers_singleton_backed_by_HttpClientFactory()
    {
        _server
            .Given(Request.Create().WithPath("/thing").UsingGet())
            .RespondWith(Response.Create()
                .WithStatusCode(200)
                .WithHeader("Content-Type", "application/json")
                .WithBody("{\"value\":\"ok\"}"));

        var services = new ServiceCollection();
        services.AddSmooFetch(opts =>
        {
            opts.BaseUrl = _server.Urls[0];
            opts.RetryPolicy = RetryPolicy.None;
        });

        using var provider = services.BuildServiceProvider();
        var fetch1 = provider.GetRequiredService<SmooFetch>();
        var fetch2 = provider.GetRequiredService<SmooFetch>();

        Assert.Same(fetch1, fetch2);

        var thing = await fetch1.GetAsync<Thing>("/thing");
        Assert.Equal("ok", thing.Value);
    }

    [Fact]
    public void Create_throws_when_RequireHttpClientFactory_is_true()
    {
        Assert.Throws<InvalidOperationException>(() => SmooFetch.Create(opts =>
        {
            opts.RequireHttpClientFactory = true;
        }));
    }
}
