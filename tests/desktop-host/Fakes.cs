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
    public IReadOnlyList<DockHeaderItem> Header => [];
}

/// <summary>
/// A dock view that keeps what it was asked to show, which is how a test reads what a reveal came to.
/// </summary>
/// <remarks>
/// A dock that a place is shown in is told the same request it is opened with (see <see cref="IDockOpened"/>), so what
/// it was told is the whole answer: a test that asked for a path can read the path the dock was given rather than
/// guess it from a control.
/// </remarks>
internal sealed class RecordingDockView : IDockView, IDockOpened
{
    /// <summary>The control this view is, so that a dock it belongs to can be laid out.</summary>
    public Control Control { get; } = new Border { Background = Avalonia.Media.Brushes.Gray };

    /// <summary>What the dock has been asked to show, in the order it was asked.</summary>
    public List<DockRequest> Requests { get; } = [];

    /// <inheritdoc />
    public Control View => Control;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderItem> Header => [];

    /// <inheritdoc />
    public void Opened(DockRequest request) => Requests.Add(request);
}
