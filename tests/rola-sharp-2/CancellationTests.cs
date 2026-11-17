using Xunit;

namespace RolaSharp.V2.IntegrationTests;

public sealed class CancellationTests
{
    [Fact]
    public async Task A_cancelled_read_is_a_read_error()
    {
        using var cancellation = new CancellationTokenSource();
        await cancellation.CancelAsync();

        var scoop = new LayoutScoop(".", "main") { NoCheck = true };

        var error = await Assert.ThrowsAsync<RolaReadError>(
            () => scoop.ReadAsync(cancellation.Token)
        );

        Assert.Equal(RolaReadErrorKind.Cancelled, error.Kind);
    }
}
