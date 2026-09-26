using Avalonia.Controls;
using RorolalaDesktop.Contract;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A dock view that is never drawn.
/// </summary>
/// <remarks>
/// The view's control is deliberately absent. The dock manager holds it and hands it to the area,
/// and nothing here draws, so a stand-in with no control is honest about what is exercised — the
/// bookkeeping rather than the rendering.
/// </remarks>
internal sealed class FakeDockView : IDockView
{
    /// <inheritdoc />
    public Control View => null!;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderCommand> HeaderCommands => [];
}
