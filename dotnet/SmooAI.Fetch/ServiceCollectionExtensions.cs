using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Logging.Abstractions;

namespace SmooAI.Fetch;

/// <summary>
/// DI registration helpers that wire <see cref="SmooFetch"/> through
/// <see cref="IHttpClientFactory"/> so the underlying <see cref="HttpClient"/>
/// is pooled correctly (no socket exhaustion).
/// </summary>
public static class ServiceCollectionExtensions
{
    /// <summary>
    /// Register <see cref="SmooFetch"/> as a singleton backed by a named
    /// <see cref="IHttpClientFactory"/> client. The resulting <see cref="SmooFetch"/> is resolvable
    /// from DI and shares the factory's handler pooling.
    /// </summary>
    /// <param name="services">DI container.</param>
    /// <param name="configure">Optional options configuration. Called once at registration time.</param>
    /// <param name="clientName">Named client key used for <see cref="IHttpClientFactory"/>. Defaults to <see cref="SmooFetch.DefaultHttpClientName"/>.</param>
    public static IServiceCollection AddSmooFetch(
        this IServiceCollection services,
        Action<SmooFetchOptions>? configure = null,
        string clientName = SmooFetch.DefaultHttpClientName)
    {
        ArgumentNullException.ThrowIfNull(services);

        var options = new SmooFetchOptions();
        configure?.Invoke(options);

        services.AddHttpClient(clientName, client =>
        {
            // Per-request timeout is enforced by SmooFetch via a CancellationTokenSource.
            client.Timeout = System.Threading.Timeout.InfiniteTimeSpan;
            if (!string.IsNullOrEmpty(options.BaseUrl))
            {
                client.BaseAddress = new Uri(options.BaseUrl);
            }
        });

        services.AddSingleton(sp =>
        {
            var factory = sp.GetRequiredService<IHttpClientFactory>();
            var logger = sp.GetService<ILogger<SmooFetch>>() ?? NullLogger<SmooFetch>.Instance;
            var client = factory.CreateClient(clientName);
            return SmooFetch.CreateFromFactory(client, options, logger);
        });

        return services;
    }
}
