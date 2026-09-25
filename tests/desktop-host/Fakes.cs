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

/// <summary>
/// A theme that is never applied.
/// </summary>
/// <remarks>
/// Nothing here needs Avalonia's styling system, so the stand-in states an id and no styles, which
/// is what theme selection reads.
/// </remarks>
internal sealed class FakeTheme : RorolalaDesktop.Contract.IThemeProvider
{
    /// <summary>Makes a theme with one id.</summary>
    /// <param name="themeId">The id <c>preference.json</c> would name.</param>
    public FakeTheme(string themeId) => ThemeId = themeId;

    /// <inheritdoc />
    public string ThemeId { get; }

    /// <inheritdoc />
    public IReadOnlyList<Avalonia.Styling.IStyle> Styles => [];
}
