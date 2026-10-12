using Avalonia.Controls;
using RorolalaDesktop.Contract;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A dock view that is never drawn.
/// </summary>
/// <remarks>
/// The view's control is deliberately absent. The dock manager holds it and hands it to the area,
/// and nothing here draws, so a stand-in with no control is honest about what is exercised — the
/// bookkeeping rather than the rendering. A test that has to watch where a control ends up gives it
/// one, which is the one thing that makes this view a control tree rather than a placeholder.
/// </remarks>
internal sealed class FakeDockView : IDockView
{
    /// <summary>The control this view is, or nothing when the test does not draw it.</summary>
    public Control? Control { get; init; }

    /// <inheritdoc />
    public Control View => Control!;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderCommand> HeaderCommands => [];
}
