using Avalonia;
using Avalonia.Animation;
using Avalonia.Controls;
using Avalonia.Controls.Presenters;
using Avalonia.Controls.Primitives;
using Avalonia.Controls.Shapes;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Styling;
using RorolalaDesktop.Docking;

namespace RorolalaDesktop.Theming;

/// <summary>
/// The look: one accent, flat rectangles, and one-pixel edges.
/// </summary>
/// <remarks>
/// It is the program's own and there is only one of it. What a run chooses is the variant it is drawn
/// in and the accent, and both are read before the window is made (Section 10). Everything else here is
/// stated rather than configured, which is what makes the program look like one thing.
/// <para>
/// The design is a Win10 one: every surface is a rectangle, every edge is one pixel, hover is a
/// neutral tint rather than the accent, and the accent is spent on one thing — what is selected.
/// Nothing is rounded, nothing is raised, nothing moves; what is animated is colour and opacity only.
/// </para>
/// <para>
/// The accent is written into <em>the base theme's own</em> accent resources rather than applied
/// control by control. The base theme draws a selected row, a checked box and a text selection from
/// those resources, so naming them once is what makes the whole program accented instead of an
/// accented patch on a blue theme — and it is why the few rules below are about geometry and ink
/// rather than about colour.
/// </para>
/// <para>
/// The ink on the accent needs a rule of its own, because the base theme writes white there and white
/// is unreadable on a light accent — and which colour the accent is belongs to the user.
/// </para>
/// <para>
/// The three tints the chrome is drawn with are this theme's own, because the base theme has none to
/// borrow: its neutrals are all opaque, and a band of chrome has to be a tint of whatever is behind
/// it to sit on the window and on the content alike. They are given per variant, as the ink of that
/// variant.
/// </para>
/// <para>
/// The shell marks the surfaces it owns with classes, which is how a rule here addresses a dock
/// without knowing what one is: <see cref="MainWindow.MenuBarClass"/> on the menu bar,
/// <see cref="DockArea.HeadersClass"/> on a region's header strip,
/// <see cref="DockArea.TitleClass"/> on a dock's header — with <see cref="DockArea.SelectedClass"/>
/// on the one being shown — <see cref="DockArea.SplitterClass"/> on the grab between regions, and
/// <see cref="DockArea.DropZoneClass"/> on where a dragged dock would land.
/// </para>
/// </remarks>
internal sealed class RorolalaTheme
{
    /// <summary>
    /// The look, in the one colour the user chooses.
    /// </summary>
    /// <remarks>
    /// One colour is configured and everything accented follows from it: the accent family the base
    /// theme fills a selected row, a checked box and a selection of text from; the accent's held state;
    /// and the ink that can be read on it. Nothing else is derived, because every colour derived from a
    /// colour is another colour that can disagree with it.
    /// </remarks>
    /// <param name="accent">The colour anything accented is drawn in.</param>
    public RorolalaTheme(Color accent)
    {
        _accent = accent;

        // The accent a held surface takes: the same colour one step down, by a factor rather than by a
        // second colour picked by hand, so that every accent has one. On lemon this is `#A6DE00`, which
        // is the held colour that was picked for it by hand.
        _held = Scaled(accent, 0.87);

        // What is written on the accent: black or white, whichever can be read on it. The base theme
        // writes white there, which is right for the blue it was written for and unreadable on a light
        // accent — and which colour the accent is belongs to the user, so it cannot be known here.
        _ink = InkOn(accent);

        Styles = [Palette(), .. Type(), .. Chrome(), .. Content(), .. Parts()];
    }

    /// <summary>The size every word is set at.</summary>
    private const double BodySize = 13.0;

    /// <summary>The height of a row of a list, a tree, or a field: short enough to scan, tall enough to hit.</summary>
    private const double RowHeight = 26.0;

    /// <summary>How rounded anything is: not at all.</summary>
    private static readonly CornerRadius Square = new(0);

    /// <summary>The one edge every surface wears.</summary>
    private static readonly Thickness Edge = new(1);

    /// <summary>How long a colour or an opacity takes to arrive.</summary>
    private static readonly TimeSpan Fade = TimeSpan.FromMilliseconds(100);

    /// <summary>How long a drop zone takes to light up, which is a fade rather than a step.</summary>
    private static readonly TimeSpan Light = TimeSpan.FromMilliseconds(120);

    /// <summary>
    /// The typeface the program is set in.
    /// </summary>
    /// <remarks>
    /// Named through the font collection rather than by the family alone, and that is the whole point
    /// of the long form: a plain <c>Inter</c> is looked for among the system's fonts, is not there,
    /// and falls back to Noto Sans without saying so — which is what this program was set in while it
    /// shipped a face it never used. The collection is the one <c>WithInterFont</c> adds.
    /// </remarks>
    private static readonly FontFamily Face = new("fonts:Inter#Inter");

    /// <summary>A band of chrome: a tint of the variant's own ink, drawn by this theme.</summary>
    private const string Tint = "rorolala.theme.tint";

    /// <summary>The same tint one step stronger, for the two places that have to stand out from a band.</summary>
    private const string DeeperTint = "rorolala.theme.tint.deeper";

    /// <summary>A hairline, for the rule between chrome and content and for the edge of a control.</summary>
    private const string Line = "rorolala.theme.line";

    /// <summary>The red a close is everywhere, which is the one thing in the program that is not the accent.</summary>
    private static readonly Color Closed = Color.FromRgb(0xC4, 0x2B, 0x1C);

    /// <summary>The same red, while it is held.</summary>
    private static readonly Color ClosedDeep = Color.FromRgb(0x9E, 0x22, 0x16);

    /// <summary>The colour anything accented is drawn in.</summary>
    private readonly Color _accent;

    /// <summary>The accent a held or pressed surface takes.</summary>
    private readonly Color _held;

    /// <summary>What is written on a surface filled with the accent.</summary>
    private readonly Color _ink;

    /// <summary>The styles the look is made of, built once when the look is.</summary>
    public IReadOnlyList<IStyle> Styles { get; }

    /// <summary>
    /// The accent, written into the base theme's own accent resources, and the tints of this theme's
    /// own.
    /// </summary>
    /// <remarks>
    /// A style carries resources as readily as it carries setters, and a style added after the base
    /// theme is read after it — which is what lets an overlay redefine a colour the base theme named.
    /// The entries are given twice, once per variant, because a lookup reads the dictionary of the
    /// variant it is resolving for and never a default one.
    /// <para>
    /// Both the colours and the brushes are named, because the base theme's brushes are built from its
    /// colours only in one of the two variants: naming the colours alone left half the program blue in
    /// the other, which is what happened the first time this was tried.
    /// </para>
    /// </remarks>
    private Style Palette()
    {
        var style = new Style(selector => selector.OfType<Window>());

        style.Resources = new ResourceDictionary
        {
            ThemeDictionaries =
            {
                [ThemeVariant.Light] = Accents(
                    Color.FromArgb(0x0F, 0x00, 0x00, 0x00),
                    Color.FromArgb(0x1F, 0x00, 0x00, 0x00),
                    Color.FromArgb(0x33, 0x00, 0x00, 0x00)
                ),
                [ThemeVariant.Dark] = Accents(
                    Color.FromArgb(0x0F, 0xFF, 0xFF, 0xFF),
                    Color.FromArgb(0x1F, 0xFF, 0xFF, 0xFF),
                    Color.FromArgb(0x33, 0xFF, 0xFF, 0xFF)
                ),
            },
        };

        return style;
    }

    /// <summary>
    /// What one variant's palette is.
    /// </summary>
    /// <param name="tint">A band of chrome.</param>
    /// <param name="deeper">The same, one step stronger.</param>
    /// <param name="line">A hairline.</param>
    private ResourceDictionary Accents(Color tint, Color deeper, Color line) =>
        new()
        {
            // The accent family, which is what the base theme fills a selected row, opens a drop-down
            // and clicks a box with. The alphas are the base theme's own, so the washes stay washes.
            ["ThemeAccentColor"] = _accent,
            ["ThemeAccentColor2"] = WithAlpha(_accent, 0x99),
            ["ThemeAccentColor3"] = WithAlpha(_accent, 0x66),
            ["ThemeAccentColor4"] = WithAlpha(_accent, 0x33),
            ["ThemeAccentBrush"] = Fill(_accent),
            ["ThemeAccentBrush2"] = Fill(WithAlpha(_accent, 0x99)),
            ["ThemeAccentBrush3"] = Fill(WithAlpha(_accent, 0x66)),
            ["ThemeAccentBrush4"] = Fill(WithAlpha(_accent, 0x33)),

            // What sits on the accent: the base theme writes white there, which a light accent cannot
            // carry.
            ["HighlightForegroundColor"] = _ink,
            ["HighlightForegroundBrush"] = Fill(_ink),

            // The highlight the base theme keeps apart from its accent family — a tick, a selection of
            // text — which would otherwise stay blue while everything else went accented.
            ["HighlightColor"] = _accent,
            ["HighlightBrush"] = Fill(_accent),
            ["HighlightColor2"] = _held,
            ["HighlightBrush2"] = Fill(_held),

            [Tint] = Fill(tint),
            [DeeperTint] = Fill(deeper),
            [Line] = Fill(line),
        };

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
    /// The menu bar and every region's header strip take one band of tint, so the window reads as
    /// chrome over content whichever regions are open. A dock header is flat and square, and every one
    /// of them keeps a one-pixel bottom edge that is drawn in nothing until the dock is the one being
    /// shown, when it is drawn in the accent: a header that grew an edge when selected would shift the
    /// headers beside it sideways under the very pointer that selected it.
    /// </remarks>
    private Style[] Chrome() =>
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

            On(
                selector => selector.OfType<Button>().Class(DockArea.TitleClass),
                new Setter(TemplatedControl.BackgroundProperty, Brushes.Transparent),
                new Setter(TemplatedControl.BorderBrushProperty, Brushes.Transparent),
                new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(0, 0, 0, 1)),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<Button>().Class(DockArea.TitleClass).Class(DockArea.SelectedClass),
                Brushed(TemplatedControl.BackgroundProperty, DeeperTint),
                Fixed(TemplatedControl.BorderBrushProperty, _accent),
                new Setter(TemplatedControl.FontWeightProperty, FontWeight.SemiBold)
            ),

            // The base theme repaints a hovered button's edge itself, which would take the accent
            // line off the header of the dock being shown for as long as the pointer rested on it.
            On(
                selector =>
                    selector
                        .OfType<Button>()
                        .Class(DockArea.TitleClass)
                        .Class(DockArea.SelectedClass)
                        .Class(":pointerover")
                        .Template()
                        .Name("PART_ContentPresenter"),
                Fixed(ContentPresenter.BorderBrushProperty, _accent)
            ),

            // The button that closes a region's shown dock, at the far end of the strip: nothing at
            // all until the pointer is on it, and then the red a close is everywhere.
            On(
                selector => selector.OfType<Button>().Class(DockArea.CloseClass),
                new Setter(TemplatedControl.BackgroundProperty, Brushes.Transparent),
                new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(0)),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(9, 2)),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),

            // The fill is drawn on the part rather than on the button, and carries an activator, for
            // the reason the underline rule above does: the base theme paints a hovered button's own
            // part, and would otherwise have the last word on exactly the state this is about.
            On(
                selector =>
                    selector
                        .OfType<Button>()
                        .Class(DockArea.CloseClass)
                        .Class(":pointerover")
                        .Template()
                        .Name("PART_ContentPresenter"),
                Fixed(ContentPresenter.BackgroundProperty, Closed),
                new Setter(ContentPresenter.ForegroundProperty, Brushes.White),
                new Setter(ContentPresenter.CornerRadiusProperty, Square),
                new Setter(ContentPresenter.TransitionsProperty, Fading())
            ),
            On(
                selector =>
                    selector
                        .OfType<Button>()
                        .Class(DockArea.CloseClass)
                        .Class(":pressed")
                        .Template()
                        .Name("PART_ContentPresenter"),
                Fixed(ContentPresenter.BackgroundProperty, ClosedDeep),
                new Setter(ContentPresenter.ForegroundProperty, Brushes.White)
            ),

            // A region with nothing in it hides its splitter, so one that is visible is one that can be
            // dragged. It draws nothing until the pointer is on it, and then a hairline through its
            // middle: the grab has to stay wide enough to hit, and a line that wide would be a bar.
            // That is what the fixed column and row classes are for — which way the hairline runs is
            // the one thing a splitter's own geometry cannot say.
            On(
                selector => selector.OfType<GridSplitter>().Class(DockArea.SplitterClass),
                new Setter(TemplatedControl.BackgroundProperty, Brushes.Transparent),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector =>
                    selector
                        .OfType<GridSplitter>()
                        .Class(DockArea.SplitterClass)
                        .Class(DockArea.SplitterColumnsClass)
                        .Class(":pointerover"),
                new Setter(TemplatedControl.BackgroundProperty, Hairline(vertical: true))
            ),
            On(
                selector =>
                    selector
                        .OfType<GridSplitter>()
                        .Class(DockArea.SplitterClass)
                        .Class(DockArea.SplitterRowsClass)
                        .Class(":pointerover"),
                new Setter(TemplatedControl.BackgroundProperty, Hairline(vertical: false))
            ),

            // Where a dragged dock could land. All of them are drawn while a drag is on, because they are
            // the question — which regions are there to land in — and the one being aimed at is drawn in
            // the accent, because it is the answer. The others are a tint of the variant's ink: white over
            // a dark program, black over a light one, which is what the tint family is.
            On(
                selector => selector.OfType<Border>().Class(DockArea.DropZoneClass),
                new Setter(Border.BorderThicknessProperty, Edge),
                new Setter(Border.CornerRadiusProperty, Square),
                Brushed(Border.BackgroundProperty, DeeperTint),
                Brushed(Border.BorderBrushProperty, Line),
                new Setter(Border.TransitionsProperty, Fading(Light))
            ),
            On(
                selector =>
                    selector
                        .OfType<Border>()
                        .Class(DockArea.DropZoneClass)
                        .Class(DockArea.DropTargetClass),
                Fixed(Border.BackgroundProperty, WithAlpha(_accent, 0x33)),
                Fixed(Border.BorderBrushProperty, _accent)
            ),
        ];

    /// <summary>The controls the kernel and the plugins build their content out of.</summary>
    /// <remarks>
    /// Geometry and spacing only: the base theme's own brushes already say what a control is made of and
    /// change with the variant, and the accent is named in that theme's own resources, so nothing here
    /// names a colour except the edge a surface wears.
    /// <para>
    /// A rule here cannot make a tooltip's text smaller by setting the size on the tooltip: the rule
    /// for <see cref="TextBlock"/> sets a size on every text, and a set value beats an inherited one.
    /// </para>
    /// </remarks>
    private static Style[] Content() =>
        [
            On(
                selector => selector.OfType<Button>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(12, 4)),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, Line),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<TextBox>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 4)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                // A field is a row tall and the ink in it is one line, so the two are not the same height, and
                // the base theme aligns the whole of a field's content by this: left as it comes, the line
                // stands at the top of the box. Left alone across, since that same alignment sizes the field's
                // content where it is set — narrowed to the text, the field would no longer be typed in past
                // its end.
                new Setter(TextBox.VerticalContentAlignmentProperty, VerticalAlignment.Center)
            ),
            On(
                selector => selector.OfType<ComboBox>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 2)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, Line)
            ),
            On(
                selector => selector.OfType<ListBoxItem>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 3)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Square)
            ),
            On(
                selector => selector.OfType<TreeViewItem>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(2, 2)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Square)
            ),
            On(
                selector => selector.OfType<MenuItem>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(10, 5)),
                new Setter(TemplatedControl.CornerRadiusProperty, Square)
            ),
            On(
                selector => selector.OfType<CheckBox>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(Layoutable.MinHeightProperty, RowHeight)
            ),
            On(
                selector => selector.OfType<NumericUpDown>(),
                new Setter(Layoutable.MinHeightProperty, RowHeight)
            ),
            On(
                selector => selector.OfType<ToolTip>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 4)),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, Line)
            ),
        ];

    /// <summary>
    /// The template parts the base theme colours itself, and the few the accent makes wrong.
    /// </summary>
    /// <remarks>
    /// The base theme draws most states on a control's template rather than on the control — a selected
    /// row's fill, a focused field's edge, the tick in a box — so a rule for the control cannot reach
    /// them, and the part has to be named. Every rule here carries an activator as well as the part
    /// name: Avalonia ranks an activated setter above a template binding and above a plain setter,
    /// which is what it takes to be heard over the base theme's styling of the same place.
    /// <para>
    /// The fills mostly need no rule at all, since they follow the accent resources; the ones that do
    /// are the box that is filled with it and the ink that cannot be white on it. The rest is the
    /// field's edge and the fades.
    /// </para>
    /// </remarks>
    private Style[] Parts() =>
        [
            Ink(selector => selector.OfType<ListBoxItem>(), "PART_ContentPresenter"),
            Ink(selector => selector.OfType<ComboBoxItem>(), "PART_ContentPresenter"),
            Ink(selector => selector.OfType<TreeViewItem>(), "PART_HeaderPresenter"),

            // A checked box is filled with the accent and ticked in the ink that can be read on it,
            // where the base theme leaves the box empty and draws the tick in the accent.
            On(
                selector => selector.OfType<CheckBox>().Class(":checked").Template().Name("border"),
                Fixed(Border.BackgroundProperty, _accent),
                Fixed(Border.BorderBrushProperty, _accent)
            ),
            On(
                selector => selector.OfType<CheckBox>().Class(":checked").Template().Name("checkMark"),
                new Setter(Shape.FillProperty, new SolidColorBrush(_ink))
            ),

            // A field keeps its one pixel when it takes focus, and takes it in the accent: the base
            // theme brightens its border instead, which is a change of the same weight and no help
            // about where the keyboard is.
            On(
                selector => selector.OfType<TextBox>().Class(":focus").Template().Name("border"),
                new Setter(Border.BorderThicknessProperty, Edge),
                Fixed(Border.BorderBrushProperty, _accent)
            ),

            // Every part that changes colour on a state fades into it, which is all the motion there
            // is: a fade is what a flat surface can do without moving anything under the pointer.
            On(
                selector => selector.OfType<TemplatedControl>().Template().Name("PART_ContentPresenter"),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<TemplatedControl>().Template().Name("PART_HeaderPresenter"),
                new Setter(ContentPresenter.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<TemplatedControl>().Template().Name("border"),
                new Setter(Border.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<TemplatedControl>().Template().Name("checkMark"),
                new Setter(Shape.TransitionsProperty, Fading())
            ),
        ];

    /// <summary>
    /// What is written on a surface filled with the accent.
    /// </summary>
    /// <remarks>
    /// The base theme fills a selected row with the accent and then puts white on it, which is right
    /// for the blue it was written for and unreadable on a light accent. Black or white by contrast is
    /// what any accent can carry.
    /// </remarks>
    private Style Ink(Func<Selector?, Selector> ofType, string part) =>
        On(
            selector => ofType(selector).Class(":selected").Template().Name(part),
            new Setter(ContentPresenter.ForegroundProperty, new SolidColorBrush(_ink))
        );

    /// <summary>
    /// A hairline of the accent, through the middle of a splitter.
    /// </summary>
    /// <remarks>
    /// A gradient with hard stops rather than a colour, because there is nothing to put a line in: a
    /// splitter is one surface with no template part a theme may address, so the only way to draw one
    /// pixel of line inside four pixels of grip is to paint the grip clear and the middle of it not.
    /// </remarks>
    /// <param name="vertical">Whether the line runs down the splitter rather than across it.</param>
    private IBrush Hairline(bool vertical)
    {
        // Half a pixel, as the fraction of the band's width the stop offsets are in.
        var half = 0.5 / DockArea.SplitterSize;
        var clear = Colors.Transparent;

        return new LinearGradientBrush
        {
            StartPoint = vertical
                ? new RelativePoint(0, 0.5, RelativeUnit.Relative)
                : new RelativePoint(0.5, 0, RelativeUnit.Relative),
            EndPoint = vertical
                ? new RelativePoint(1, 0.5, RelativeUnit.Relative)
                : new RelativePoint(0.5, 1, RelativeUnit.Relative),
            GradientStops =
            {
                new GradientStop(clear, 0),
                new GradientStop(clear, 0.5 - half),
                new GradientStop(_accent, 0.5 - half),
                new GradientStop(_accent, 0.5 + half),
                new GradientStop(clear, 0.5 + half),
                new GradientStop(clear, 1),
            },
        };
    }

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
    /// A setter whose value is one of the base theme's resources.
    /// </summary>
    /// <remarks>
    /// The reference is resolved where the element is rather than here, so a colour is the one the
    /// element's own variant has. Resolving it here would fix it to the variant that was in force when
    /// the theme was applied, and a system that changed variant while the program ran would leave the
    /// overlay behind — half of the window following the system and half of it not.
    /// </remarks>
    private static Setter Brushed(AvaloniaProperty property, string resource) =>
        new(property, new DynamicResourceExtension(resource));

    /// <summary>A setter whose value is one of this theme's own colours.</summary>
    private static Setter Fixed(AvaloniaProperty property, Color colour) =>
        new(property, new SolidColorBrush(colour));

    /// <summary>
    /// What is written on a surface filled with an accent.
    /// </summary>
    /// <remarks>
    /// Black or white, whichever can be read on the accent. It is the one thing about a chosen accent
    /// that cannot be read off the colour by eye, which is why it is worked out rather than picked, and
    /// why it is open to the tests as well as used here.
    /// </remarks>
    /// <param name="accent">The colour anything accented is drawn in.</param>
    /// <returns>The ink to write on it.</returns>
    internal static Color InkOn(Color accent) =>
        Contrasts(Colors.Black, accent) >= Contrasts(Colors.White, accent)
            ? Colors.Black
            : Colors.White;

    /// <summary>
    /// One colour stepped towards black by a factor.
    /// </summary>
    /// <remarks>
    /// A factor rather than a colour: the accent is the user's to choose, and a held state that was a
    /// second chosen colour would be one every other accent did without.
    /// </remarks>
    private static Color Scaled(Color colour, double factor) =>
        Color.FromRgb(
            (byte)Math.Round(colour.R * factor),
            (byte)Math.Round(colour.G * factor),
            (byte)Math.Round(colour.B * factor)
        );

    /// <summary>
    /// How much one colour stands out from another, as the ratio a reader's legibility is held to.
    /// </summary>
    /// <remarks>
    /// The one number that answers "can this be read on that" without knowing which colour either is,
    /// which is what a chosen accent leaves to be worked out.
    /// </remarks>
    private static double Contrasts(Color one, Color other)
    {
        var (lighter, darker) = Luminance(one) > Luminance(other) ? (one, other) : (other, one);

        return (Luminance(lighter) + 0.05) / (Luminance(darker) + 0.05);
    }

    /// <summary>A colour's relative luminance, as the contrast ratio is defined over.</summary>
    private static double Luminance(Color colour) =>
        (0.2126 * Channel(colour.R)) + (0.7152 * Channel(colour.G)) + (0.0722 * Channel(colour.B));

    /// <summary>One channel, linearised from the value a display writes.</summary>
    private static double Channel(byte value)
    {
        var encoded = value / 255.0;

        return encoded <= 0.03928 ? encoded / 12.92 : Math.Pow((encoded + 0.055) / 1.055, 2.4);
    }

    /// <summary>The fade a surface takes a new colour with.</summary>
    private static Transitions Fading() => Fading(Fade);

    /// <summary>The fade a surface arrives with, over the given time.</summary>
    private static Transitions Fading(TimeSpan duration) =>
        new()
        {
            new BrushTransition
            {
                Property = TemplatedControl.BackgroundProperty,
                Duration = duration,
            },
            new BrushTransition
            {
                Property = TemplatedControl.BorderBrushProperty,
                Duration = duration,
            },
            new DoubleTransition { Property = Visual.OpacityProperty, Duration = duration },
        };

    /// <summary>A brush of one colour.</summary>
    private static IBrush Fill(Color colour) => new SolidColorBrush(colour);

    /// <summary>The same colour, at another alpha.</summary>
    private static Color WithAlpha(Color colour, byte alpha) =>
        Color.FromArgb(alpha, colour.R, colour.G, colour.B);
}
