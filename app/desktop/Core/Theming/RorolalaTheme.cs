using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Styling;
using RorolalaDesktop.Docking;
using IThemeProvider = RorolalaDesktop.Contract.IThemeProvider;

namespace RorolalaDesktop.Theming;

/// <summary>
/// The built-in theme: the shell's chrome, its type, and its spacing.
/// </summary>
/// <remarks>
/// It is not a plugin, though it is treated exactly like one: the host registers it, so it is there
/// even with no plugins at all, and it is the default value of <c>theme</c> in <c>preference.json</c>.
/// <para>
/// It stays an overlay on the base theme, and most of what it does is shape rather than colour. Every
/// brush it names is one of Fluent's own resources, read as the element's variant has it, so light
/// and dark and the user's accent colour follow Fluent's with no second palette to keep in step; what
/// is decided here is the typeface, the sizes, the spacing, the radii, which surfaces read as chrome,
/// and how the dock a region is showing is marked. A second palette is the thing this deliberately
/// does not have: it would have to be kept in step with Fluent's, in two variants, by hand.
/// </para>
/// <para>
/// The shell marks the surfaces it owns with classes, which is how a rule here addresses a dock
/// without knowing what one is: <see cref="MainWindow.MenuBarClass"/> on the menu bar,
/// <see cref="DockArea.HeadersClass"/> on a region's header strip,
/// <see cref="DockArea.TitleClass"/> on a dock's header — with <see cref="DockArea.SelectedClass"/>
/// on the one being shown — and <see cref="DockArea.SplitterClass"/> on the grab between regions.
/// A theme a plugin supplies is free to use the same marks (Section 10).
/// </para>
/// </remarks>
internal sealed class RorolalaTheme : IThemeProvider
{
    /// <summary>The id <c>preference.json</c> names this theme by.</summary>
    public const string Id = "rorolala.theme.default";

    /// <summary>The size body text is set at.</summary>
    private const double BodySize = 13.0;

    /// <summary>The height of a row of a list, a tree, or a field: short enough to scan, tall enough to hit.</summary>
    private const double RowHeight = 26.0;

    /// <summary>How rounded a control is.</summary>
    private static readonly CornerRadius Radius = new(6);

    /// <summary>How rounded a small mark is, such as a dock header or a menu item.</summary>
    private static readonly CornerRadius Tight = new(4);

    /// <summary>
    /// The typeface the program is set in.
    /// </summary>
    /// <remarks>
    /// Named through the font collection rather than by the family alone, and that is the whole point
    /// of the long form: a plain <c>Inter</c> is looked for among the system's fonts, is not there,
    /// and falls back to Noto Sans without saying so — which is what this program was set in while it
    /// shipped a face it never used. The collection is the one <c>WithInterFont</c> adds. Glyphs
    /// Inter has none of — Chinese, most emoji — come from the fallback the font manager chooses for
    /// them, which is what a family is for.
    /// </remarks>
    private static readonly FontFamily Face = new("fonts:Inter#Inter");

    /// <summary>
    /// A tint over whatever is behind, for the bands that read as chrome: darkening in a light
    /// variant, lightening in a dark one.
    /// </summary>
    private const string Tint = "SystemControlBackgroundListLowBrush";

    /// <summary>
    /// The same tint, one step stronger, for the two places that have to stand out from a band of it:
    /// the grab between regions, and the header of the dock a region is showing.
    /// </summary>
    /// <remarks>
    /// One step, not two. A tint of forty percent or more — which Fluent's brushes make available —
    /// leaves black text on grey at 3.7:1 in the light variant, under what body text needs, and these
    /// sit behind the very labels they have to be read with.
    /// </remarks>
    private const string DeeperTint = "SystemControlBackgroundBaseLowBrush";

    /// <summary>A hairline, for a rule between chrome and content.</summary>
    private const string Line = "SystemControlForegroundBaseLowBrush";

    /// <summary>The accent, for the mark that says which dock is being shown.</summary>
    private const string Accent = "SystemControlHighlightAccentBrush";

    /// <inheritdoc />
    public string ThemeId => Id;

    /// <inheritdoc />
    public IReadOnlyList<IStyle> Styles { get; } = [.. Type(), .. Chrome(), .. Content()];

    /// <summary>What the whole program is set in.</summary>
    private static Style[] Type() =>
        [
            // Text is not a control, so it does not take the rule below; both are needed.
            On(
                selector => selector.OfType<TextBlock>(),
                new Setter(TextBlock.FontFamilyProperty, Face),
                new Setter(TextBlock.FontSizeProperty, BodySize)
            ),
            On(
                selector => selector.OfType<TemplatedControl>(),
                new Setter(TemplatedControl.FontFamilyProperty, Face),
                new Setter(TemplatedControl.FontSizeProperty, BodySize)
            ),
        ];

    /// <summary>
    /// The surfaces the shell owns: the menu bar, the dock headers, and the grabs between regions.
    /// </summary>
    /// <remarks>
    /// The menu bar and every region's header strip take one tint, so the window reads as chrome over
    /// content whichever regions are open. The dock headers are flat and only their top rule is
    /// coloured, which is what makes a row of them read as tabs rather than as a row of buttons.
    /// </remarks>
    private static Style[] Chrome() =>
        [
            On(
                selector => selector.OfType<Menu>().Class(MainWindow.MenuBarClass),
                Brushed(TemplatedControl.BackgroundProperty, Tint),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(4, 0))
            ),
            On(
                selector => selector.OfType<Border>().Class(DockArea.HeadersClass),
                Brushed(Border.BackgroundProperty, Tint),
                Brushed(Border.BorderBrushProperty, Line),
                new Setter(Border.BorderThicknessProperty, new Thickness(0, 0, 0, 1))
            ),

            // The rule is the same width on every header, and drawn in nothing when the dock is not
            // the one being shown: a header that grew a border when it was selected would move the
            // other headers sideways under the pointer that selected it.
            On(
                selector => selector.OfType<Button>().Class(DockArea.TitleClass),
                new Setter(TemplatedControl.BackgroundProperty, Brushes.Transparent),
                new Setter(TemplatedControl.BorderBrushProperty, Brushes.Transparent),
                new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(0, 2, 0, 0)),
                new Setter(TemplatedControl.CornerRadiusProperty, Tight)
            ),
            On(
                selector => selector.OfType<Button>().Class(DockArea.TitleClass).Class(DockArea.SelectedClass),
                Brushed(TemplatedControl.BackgroundProperty, DeeperTint),
                Brushed(TemplatedControl.BorderBrushProperty, Accent),
                new Setter(TemplatedControl.FontWeightProperty, FontWeight.SemiBold)
            ),

            // A region with nothing in it hides its splitter, so one that is visible is one that can
            // be dragged; the accent under the pointer is what says so.
            On(
                selector => selector.OfType<GridSplitter>().Class(DockArea.SplitterClass),
                Brushed(TemplatedControl.BackgroundProperty, DeeperTint)
            ),
            On(
                selector =>
                    selector.OfType<GridSplitter>().Class(DockArea.SplitterClass).Class(":pointerover"),
                Brushed(TemplatedControl.BackgroundProperty, Accent)
            ),
        ];

    /// <summary>The controls the kernel and the plugins build their content out of.</summary>
    /// <remarks>
    /// Padding, height and radius, for the most part: Fluent's own brushes already say what a control
    /// is made of and change with the variant, and its hover, pressed and disabled states are drawn on
    /// a control's template rather than on the control, so nothing here hides them.
    /// <para>
    /// A rule here cannot make a tooltip's text smaller by setting the size on the tooltip: the rule
    /// for <see cref="TextBlock"/> sets a size on every text, and a set value beats an inherited one.
    /// The padding is what is left, which is the part a tooltip wanted anyway.
    /// </para>
    /// </remarks>
    private static Style[] Content() =>
        [
            On(
                selector => selector.OfType<Button>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(10, 4)),
                new Setter(TemplatedControl.CornerRadiusProperty, Radius)
            ),
            On(
                selector => selector.OfType<ListBoxItem>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 3)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Tight)
            ),
            On(
                selector => selector.OfType<TreeViewItem>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(4, 2)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Tight)
            ),
            On(
                selector => selector.OfType<TextBox>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 4)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Radius)
            ),
            On(
                selector => selector.OfType<ComboBox>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 2)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Radius)
            ),
            On(
                selector => selector.OfType<MenuItem>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(10, 5)),
                new Setter(TemplatedControl.CornerRadiusProperty, Tight)
            ),
            On(
                selector => selector.OfType<ToolTip>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 4))
            ),
        ];

    /// <summary>A style over a selector, carrying the setters it was given.</summary>
    private static Style On(Func<Selector?, Selector> selector, params SetterBase[] setters)
    {
        var style = new Style(selector);

        foreach (var setter in setters)
        {
            style.Setters.Add(setter);
        }

        return style;
    }

    /// <summary>
    /// A setter whose value is one of Fluent's resources.
    /// </summary>
    /// <remarks>
    /// The reference is resolved where the element is rather than here, so a colour is the one the
    /// element's own variant has. Resolving it here would fix it to the variant that was in force
    /// when the theme was applied, and a system that changes variant while the program runs would
    /// leave the overlay behind — half of the window following the system and half of it not.
    /// </remarks>
    private static Setter Brushed(AvaloniaProperty property, string resource) =>
        new(property, new DynamicResourceExtension(resource));
}
