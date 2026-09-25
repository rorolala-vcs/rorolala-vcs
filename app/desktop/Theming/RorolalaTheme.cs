using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Styling;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Theming;

/// <summary>
/// The built-in theme — a Simple placeholder.
/// </summary>
/// <remarks>
/// It is not a plugin, though it is treated exactly like one: the host registers it, so it is there
/// even with no plugins at all, and it is the default value of <c>theme</c> in
/// <c>preference.json</c>.
/// <para>
/// It is a placeholder for a real design (Section 19): for now it sets a base font size and a little
/// spacing, and leaves the accent colour to the base theme. When the design arrives, this is what
/// changes, and nothing else.
/// </para>
/// </remarks>
internal sealed class RorolalaTheme : IThemeProvider
{
    /// <summary>The id <c>preference.json</c> names this theme by.</summary>
    public const string Id = "rorolala.theme.default";

    /// <summary>The size text is set at when this theme is in force.</summary>
    private const double BaseFontSize = 13.0;

    /// <inheritdoc />
    public string ThemeId => Id;

    /// <inheritdoc />
    public IReadOnlyList<IStyle> Styles { get; } =
        [
            TextRules(),
            SpacingRules(),
        ];

    /// <summary>The font the theme sets.</summary>
    private static IStyle TextRules()
    {
        var text = new Style(selector => selector.OfType<TextBlock>());
        text.Setters.Add(new Setter(TextBlock.FontSizeProperty, BaseFontSize));

        return text;
    }

    /// <summary>The spacing the theme sets.</summary>
    private static IStyle SpacingRules()
    {
        var buttons = new Style(selector => selector.OfType<Button>());
        buttons.Setters.Add(new Setter(TemplatedControl.PaddingProperty, new Thickness(10, 4)));

        return buttons;
    }
}
