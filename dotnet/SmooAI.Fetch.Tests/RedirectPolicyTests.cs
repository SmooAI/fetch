using System.Net;
using SmooAI.Fetch;

namespace SmooAI.Fetch.Tests;

/// <summary>
/// th-86dc77 — redirects were followed unconditionally (SocketsHttpHandler defaults
/// AllowAutoRedirect to true) with no way for a caller to decline.
///
/// Following one defeats an SSRF check performed on the original host — a 302 to an
/// internal address bypasses a guard applied to the original hostname — and RFC 8461
/// forbids it outright when fetching an MTA-STS policy.
/// </summary>
public class RedirectPolicyTests
{
    private sealed record Reply(bool Arrived);

    /// <summary>Serves a 302 at /start pointing at /landing, and 200 at /landing.</summary>
    private static (HttpListener listener, string url, Func<bool> landed) StartRedirectingServer()
    {
        var landedFlag = false;
        var port = GetFreePort();
        var prefix = $"http://127.0.0.1:{port}/";
        var listener = new HttpListener();
        listener.Prefixes.Add(prefix);
        listener.Start();

        _ = Task.Run(async () =>
        {
            while (listener.IsListening)
            {
                HttpListenerContext ctx;
                try
                {
                    ctx = await listener.GetContextAsync();
                }
                catch
                {
                    return;
                }

                if (ctx.Request.Url!.AbsolutePath == "/landing")
                {
                    landedFlag = true;
                    ctx.Response.StatusCode = 200;
                    ctx.Response.ContentType = "application/json";
                    var body = System.Text.Encoding.UTF8.GetBytes("{\"arrived\":true}");
                    await ctx.Response.OutputStream.WriteAsync(body);
                }
                else
                {
                    // The 302 carries a JSON body on purpose. Without one, a
                    // deserialization failure would surface as the SAME
                    // HttpResponseError this library raises for a bad status,
                    // and the throwing-path test below could not tell the two
                    // apart.
                    ctx.Response.StatusCode = 302;
                    ctx.Response.RedirectLocation = $"{prefix}landing";
                    ctx.Response.ContentType = "application/json";
                    var redirectBody = System.Text.Encoding.UTF8.GetBytes("{\"arrived\":false}");
                    await ctx.Response.OutputStream.WriteAsync(redirectBody);
                }

                ctx.Response.Close();
            }
        });

        return (listener, prefix, () => landedFlag);
    }

    private static int GetFreePort()
    {
        var l = new System.Net.Sockets.TcpListener(System.Net.IPAddress.Loopback, 0);
        l.Start();
        var port = ((System.Net.IPEndPoint)l.LocalEndpoint).Port;
        l.Stop();
        return port;
    }

    [Fact]
    public void FollowRedirects_defaults_to_true()
    {
        Assert.True(new SmooFetchOptions().FollowRedirects);
    }

    [Fact]
    public async Task A_redirect_is_not_followed_when_the_caller_opts_out()
    {
        var (listener, url, landed) = StartRedirectingServer();
        try
        {
            var fetch = SmooFetchBuilder.Create()
                .WithNoRetry()
                .WithFollowRedirects(false)
                .Build();

            using var request = new HttpRequestMessage(HttpMethod.Get, $"{url}start");
            var response = await fetch.SendAsync(request);

            Assert.Equal(HttpStatusCode.Found, response.StatusCode);
            // The landing route is mounted so following would visibly succeed —
            // this asserts the hop did not happen, not that it 404'd.
            Assert.False(landed(), "the redirect was followed despite WithFollowRedirects(false)");
        }
        finally
        {
            listener.Stop();
        }
    }

    [Fact]
    public async Task Redirects_are_followed_by_default()
    {
        var (listener, url, landed) = StartRedirectingServer();
        try
        {
            var fetch = SmooFetchBuilder.Create().WithNoRetry().Build();

            using var request = new HttpRequestMessage(HttpMethod.Get, $"{url}start");
            var response = await fetch.SendAsync(request);

            Assert.Equal(HttpStatusCode.OK, response.StatusCode);
            Assert.True(landed(), "default behaviour must be unchanged");
        }
        finally
        {
            listener.Stop();
        }
    }

    /// <summary>
    /// The throwing path. <c>SendAsync</c> never raises, so it cannot show that a
    /// deliberately-unfollowed 3xx is returned rather than thrown — without the
    /// <c>IsAcceptable</c> change the option would be honoured and then immediately
    /// undone by an <see cref="HttpResponseError"/>.
    /// </summary>
    [Fact]
    public async Task An_unfollowed_redirect_is_not_thrown_on_the_json_path()
    {
        var (listener, url, _) = StartRedirectingServer();
        try
        {
            var fetch = SmooFetchBuilder.Create()
                .WithNoRetry()
                .WithFollowRedirects(false)
                .Build();

            var reply = await fetch.GetAsync<Reply>($"{url}start");

            // Reached deserialization at all, which means the 3xx passed the
            // status gate rather than being thrown as an HttpResponseError.
            Assert.False(reply.Arrived);
        }
        finally
        {
            listener.Stop();
        }
    }
}
