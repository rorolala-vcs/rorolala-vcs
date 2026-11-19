using Xunit;

namespace RolaSharp.IntegrationTests;

public sealed class CancellationTests
{
    [Fact]
    public async Task A_cancelled_read_is_a_read_error()
    {
        using var cancellation = new CancellationTokenSource();
        await cancellation.CancelAsync();

        var scope = new LayoutScope(".", "main") { NoCheck = true };

        var error = await Assert.ThrowsAsync<RolaReadError>(
            () => scope.ReadAsync(cancellation.Token)
        );

        Assert.Equal(RolaReadErrorKind.Cancelled, error.Kind);
    }
}
