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
/// The look: two colours, flat rectangles, one-pixel edges, and one raised action.
/// </summary>
/// <remarks>
/// It is the program's own and there is only one of it. What a run chooses is the variant it is drawn
/// in and the two colours, and all three are read before the window is made (Section 10). Everything
/// else here is stated rather than configured, which is what makes the program look like one thing.
/// <para>
/// The design is a minimal-flat one: every surface is a rectangle, every edge is one pixel, hover is a
/// neutral tint rather than a colour, and the two chosen colours are spent by role rather than by
/// taste. <b>Primary</b> carries the weight — it fills what is selected, what is checked, and the one
/// action a surface exists for — and <b>accent</b> is spent on the marks that ask for attention: the
/// hairline a splitter shows under the pointer, the edge of a field that has focus, the zone a dragged
/// dock is aimed at, and the flash of a press. Keeping the two apart is what stops an attention mark
/// being mistaken for a selected thing; a look with one colour cannot say either.
/// </para>
/// <para>
/// There is exactly one raised surface — the action a surface exists for, which wears the primary and
/// the look's one hard shadow — so that what to do is never a question with two answers. Everything
/// else is flat, and what moves is colour and opacity only: a press changes a colour and moves nothing
/// under the pointer. Selection is the one slow thing — a row turns to the primary over a fifth of a
/// second, so that a click reads as a change rather than as a blink — while everything else answers
/// within a frame or two.
/// </para>
/// <para>
/// The two colours are written into <em>the base theme's own</em> resources rather than applied control
/// by control, because that theme draws a selected row, a checked box and a text selection from its
/// accent family: the primary is named there once, and the whole program turns primary with it.
/// </para>
/// <para>
/// Everything the look paints is drawn from a resource rather than baked into the setter that names it,
/// and that is what makes the colours editable while the program runs: a resource replaced in place
/// reaches every control that took it, where a colour written into a setter would be fixed for the life
/// of the window (Section 10).
/// </para>
/// <para>
/// The shell marks the surfaces it owns with classes, which is how a rule here addresses a dock without
/// knowing what one is: <see cref="MainWindow.MenuBarClass"/> on the menu bar,
/// <see cref="DockArea.HeadersClass"/> on a region's header strip,
/// <see cref="DockArea.TitleClass"/> on a dock's header — with <see cref="DockArea.SelectedClass"/> on
/// the one being shown — <see cref="DockArea.CloseClass"/> on the button that closes it,
/// <see cref="DockArea.SplitterClass"/> on the grab between regions, and
/// <see cref="DockArea.DropZoneClass"/> on where a dragged dock would land.
/// </para>
/// </remarks>
internal sealed class RorolalaTheme
{
    /// <summary>
    /// The look, in the two colours the user chooses.
    /// </summary>
    /// <remarks>
    /// Two colours are configured and everything follows from them: the primary family the base theme
    /// fills a selected row, a checked box and a selection of text from; the primary a button is filled
    /// with, the darker one it drops to when pressed and the lighter one it lifts to under the pointer;
    /// the accent the attention marks are drawn in; and the ink that can be read on a filled surface.
    /// Nothing else is derived, because every colour derived from a colour is another colour that can
    /// disagree with it.
    /// </remarks>
    /// <param name="primary">The colour the brand and everything selected is drawn in.</param>
    /// <param name="accent">The colour the marks that ask for attention are drawn in.</param>
    public RorolalaTheme(Color primary, Color accent)
    {
        _primary = primary;
        _accent = accent;

        // The primary a pressed surface drops to, and the one a surface under the pointer lifts to. Held
        // is darker, bright is closer to white, and both are worked out from the primary by a factor
        // rather than picked by hand, so that every primary has a pair and neither can disagree with it.
        _primaryHeld = Darkened(primary, 0.80);
        _primaryBright = Lightened(primary, 0.18);

        // What is written on the primary: black or white, whichever can be read on it. The base theme
        // writes white there, which is right for the blue it was written for and unreadable on a light
        // colour — and which colour the primary is belongs to the user.
        _primaryInk = InkOn(primary);

        _light = Accents(
            Color.FromArgb(0x40, 0x00, 0x00, 0x00),
            Color.FromArgb(0x0F, 0x00, 0x00, 0x00),
            Color.FromArgb(0x1F, 0x00, 0x00, 0x00),
            Color.FromArgb(0x33, 0x00, 0x00, 0x00),
            Color.FromArgb(0x99, 0x00, 0x00, 0x00)
        );
        _dark = Accents(
            Color.FromArgb(0x8C, 0x00, 0x00, 0x00),
            Color.FromArgb(0x0F, 0xFF, 0xFF, 0xFF),
            Color.FromArgb(0x1F, 0xFF, 0xFF, 0xFF),
            Color.FromArgb(0x33, 0xFF, 0xFF, 0xFF),
            Color.FromArgb(0x99, 0xFF, 0xFF, 0xFF)
        );

        _palette = new ResourceDictionary
        {
            // A scrollbar is a hairline of chrome rather than a control: the base theme's eighteen
            // pixels of it are half a window's margin at this density.
            ["ScrollBarThickness"] = 12.0,
            ["ScrollBarThumbThickness"] = 7.0,

            // The accent-only marks, which do not differ by variant and so are stated once.
            [HairlineVertical] = Hairline(vertical: true),
            [HairlineHorizontal] = Hairline(vertical: false),

            // What a notification is marked with. Both are picked to carry on a light ground and a dark
            // one alike, because a mark is read against whichever the program is drawn in and neither
            // colour is the user's to choose — a severity is not a preference.
            [SeverityError] = Fill(Color.FromRgb(0xE5, 0x48, 0x4D)),
            [SeverityWarn] = Fill(Color.FromRgb(0xD4, 0xA0, 0x17)),

            ThemeDictionaries =
            {
                [ThemeVariant.Light] = _light,
                [ThemeVariant.Dark] = _dark,
            },
        };

        var palette = new Style(selector => selector.OfType<Window>()) { Resources = _palette };

        Styles = [palette, .. Type(), .. Roles(), .. Content(), .. Parts(), .. Chrome()];
    }

    /// <summary>The size every word is set at.</summary>
    private const double BodySize = 13.0;

    /// <summary>The height of a row of a list, a tree, or a field: short enough to scan, tall enough to hit.</summary>
    private const double RowHeight = 28.0;

    /// <summary>The height of a band of chrome: the menu bar, a regions's header strip, a toolbar.</summary>
    private const double BarHeight = 32.0;

    /// <summary>How rounded anything is: not at all.</summary>
    private static readonly CornerRadius Square = new(0);

    /// <summary>The one edge every surface wears.</summary>
    private static readonly Thickness Edge = new(1);

    /// <summary>How long a surface answers a pointer or a press with, which is a frame or two.</summary>
    private static readonly TimeSpan Fade = TimeSpan.FromMilliseconds(100);

    /// <summary>How long a press counts as quick: shorter than a fade, so a click reads as a flash.</summary>
    private static readonly TimeSpan Press = TimeSpan.FromMilliseconds(60);

    /// <summary>
    /// How long a row takes to turn to the primary when it is chosen.
    /// </summary>
    /// <remarks>
    /// The one slow thing in the look. A click that turns a row instantly reads as a blink; a fifth of a
    /// second reads as the row becoming chosen, which is what it is. It is longer than the ceiling the
    /// rest of the look keeps to, and by decision: selection is a change of state rather than feedback,
    /// and feedback is what has to be instant (Section 17).
    /// </remarks>
    private static readonly TimeSpan Select = TimeSpan.FromMilliseconds(200);

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

    /// <summary>
    /// The typeface a column of text is set in.
    /// </summary>
    /// <remarks>
    /// A face list rather than one name, because no monospace is shipped with the program and which one
    /// a system has is the system's business: the log is a table of aligned columns, and any monospace is
    /// better at that than the proportional face every other surface is set in.
    /// </remarks>
    private static readonly FontFamily Monospace = new(
        "Cascadia Mono, Consolas, DejaVu Sans Mono, Liberation Mono, monospace"
    );

    /// <summary>The primary, as a fill.</summary>
    private const string Primary = "rorolala.theme.primary";

    /// <summary>The primary a pressed surface drops to.</summary>
    private const string PrimaryHeld = "rorolala.theme.primary.held";

    /// <summary>The primary a surface under the pointer lifts to.</summary>
    private const string PrimaryBright = "rorolala.theme.primary.bright";

    /// <summary>What is written on a surface filled with the primary.</summary>
    private const string PrimaryInk = "rorolala.theme.primary.ink";

    /// <summary>The colour secondary text is written in, which is the variant's ink held back.</summary>
    private const string Muted = "rorolala.theme.muted";

    /// <summary>
    /// The class the one action a surface exists for wears.
    /// </summary>
    /// <remarks>
    /// The look has one raised surface and this is how a control asks to be it: a dialog fills its OK, and
    /// nothing else does. It is a class rather than a property the toolkit owns so that which button is
    /// the action is the shell's to say and the look's to draw, the same as every other mark here.
    /// </remarks>
    private const string PrimaryAction = "primary";

    /// <summary>The accent, as a fill.</summary>
    private const string Accent = "rorolala.theme.accent";

    /// <summary>The accent as a wash, for the fill of a mark rather than its edge.</summary>
    private const string AccentWash = "rorolala.theme.accent.wash";

    /// <summary>The hard shadow a button casts, in this variant's own darkness.</summary>
    private const string Shadow = "rorolala.theme.shadow";

    /// <summary>The hairline a splitter shows, running down it.</summary>
    private const string HairlineVertical = "rorolala.theme.hairline.vertical";

    /// <summary>The hairline a splitter shows, running across it.</summary>
    private const string HairlineHorizontal = "rorolala.theme.hairline.horizontal";

    /// <summary>A band of chrome: a tint of the variant's own ink, drawn by this theme.</summary>
    private const string Tint = "rorolala.theme.tint";

    /// <summary>The same tint one step stronger, for the two places that have to stand out from a band.</summary>
    private const string DeeperTint = "rorolala.theme.tint.deeper";

    /// <summary>A hairline, for the rule between chrome and content and for the edge of a control.</summary>
    private const string Line = "rorolala.theme.line";

    /// <summary>The mark a failure is raised with. It is the one red the program has, and it is not chosen.</summary>
    private const string SeverityError = "rorolala.theme.severity.error";

    /// <summary>The mark a warning is raised with. It is the only amber, and it is not chosen either.</summary>
    private const string SeverityWarn = "rorolala.theme.severity.warn";

    /// <summary>The red a close is everywhere, which is the one thing in the program that is not chosen.</summary>
    private static readonly Color Closed = Color.FromRgb(0xC4, 0x2B, 0x1C);

    /// <summary>The same red, while it is held.</summary>
    private static readonly Color ClosedDeep = Color.FromRgb(0x9E, 0x22, 0x16);

    /// <summary>The colour the brand and everything selected is drawn in.</summary>
    private readonly Color _primary;

    /// <summary>The primary a pressed surface drops to.</summary>
    private readonly Color _primaryHeld;

    /// <summary>The primary a surface under the pointer lifts to.</summary>
    private readonly Color _primaryBright;

    /// <summary>What is written on a surface filled with the primary.</summary>
    private readonly Color _primaryInk;

    /// <summary>The colour the marks that ask for attention are drawn in.</summary>
    private readonly Color _accent;

    /// <summary>The whole palette, which is what a recolor replaces in place.</summary>
    private readonly ResourceDictionary _palette;

    /// <summary>The palette of the light variant.</summary>
    private readonly ResourceDictionary _light;

    /// <summary>The palette of the dark variant.</summary>
    private readonly ResourceDictionary _dark;

    /// <summary>The styles the look is made of, built once when the look is.</summary>
    public IReadOnlyList<IStyle> Styles { get; }

    /// <summary>The whole palette, which is what a recolor replaces in place.</summary>
    internal ResourceDictionary Palette => _palette;

    /// <summary>The palette of the light variant, which is what a recolor replaces in place.</summary>
    internal ResourceDictionary LightPalette => _light;

    /// <summary>The palette of the dark variant, which is what a recolor replaces in place.</summary>
    internal ResourceDictionary DarkPalette => _dark;

    /// <summary>
    /// Writes this look's colours over an older look's palette, in place.
    /// </summary>
    /// <remarks>
    /// It replaces the entries of the dictionaries that are already attached rather than swapping the
    /// dictionaries for new ones, because a resource replaced in place is what raises the change that
    /// reaches every control that took it (Section 10). Nothing is added to <c>Application.Styles</c>
    /// here or ever after the window: the styles say how the program is shaped, and a shape does not
    /// change with a colour.
    /// </remarks>
    /// <param name="palette">The attached palette.</param>
    /// <param name="light">The attached light-variant palette.</param>
    /// <param name="dark">The attached dark-variant palette.</param>
    internal void Recolour(
        ResourceDictionary palette,
        ResourceDictionary light,
        ResourceDictionary dark
    )
    {
        Copy(_palette, palette);
        Copy(_light, light);
        Copy(_dark, dark);
    }

    /// <summary>Replaces every entry of one dictionary with those of another, in one change.</summary>
    /// <param name="from">What to copy.</param>
    /// <param name="to">What to copy it into.</param>
    private static void Copy(ResourceDictionary from, ResourceDictionary to) =>
        to.SetItems(from.Keys.OfType<object>().Select(key => new KeyValuePair<object, object?>(key, from[key])));

    /// <summary>
    /// What one variant's palette is.
    /// </summary>
    /// <summary>The colour the hard shadow is drawn in, which is darkest in the dark.</summary>
    /// <param name="tint">A band of chrome.</param>
    /// <param name="deeper">The same, one step stronger.</param>
    /// <param name="line">A hairline.</param>
    /// <param name="muted">The variant's ink, held back for secondary text.</param>
    private ResourceDictionary Accents(Color shadow, Color tint, Color deeper, Color line, Color muted) =>
        new()
        {
            // The primary family, which is what the base theme fills a selected row, opens a drop-down
            // and clicks a box with. The alphas are the base theme's own, so the washes stay washes.
            ["ThemeAccentColor"] = _primary,
            ["ThemeAccentColor2"] = WithAlpha(_primary, 0x99),
            ["ThemeAccentColor3"] = WithAlpha(_primary, 0x66),
            ["ThemeAccentColor4"] = WithAlpha(_primary, 0x33),
            ["ThemeAccentBrush"] = Fill(_primary),
            ["ThemeAccentBrush2"] = Fill(WithAlpha(_primary, 0x99)),
            ["ThemeAccentBrush3"] = Fill(WithAlpha(_primary, 0x66)),
            ["ThemeAccentBrush4"] = Fill(WithAlpha(_primary, 0x33)),

            // The highlight the base theme keeps apart from its accent family — a selection of text, a
            // tick — which would otherwise stay blue while everything else went primary.
            ["HighlightColor"] = _primary,
            ["HighlightBrush"] = Fill(_primary),
            ["HighlightColor2"] = _primaryHeld,
            ["HighlightBrush2"] = Fill(_primaryHeld),

            // What sits on the primary: the base theme writes white there, which a light primary cannot
            // carry.
            ["HighlightForegroundColor"] = _primaryInk,
            ["HighlightForegroundBrush"] = Fill(_primaryInk),

            [Primary] = Fill(_primary),
            [PrimaryHeld] = Fill(_primaryHeld),
            [PrimaryBright] = Fill(_primaryBright),
            [PrimaryInk] = Fill(_primaryInk),
            [Accent] = Fill(_accent),
            [AccentWash] = Fill(WithAlpha(_accent, 0x33)),
            [Shadow] = new BoxShadows(
                new BoxShadow
                {
                    OffsetX = Cast.OffsetX,
                    OffsetY = Cast.OffsetY,
                    Blur = Cast.Blur,
                    Spread = Cast.Spread,
                    Color = shadow,
                }
            ),
            [Tint] = Fill(tint),
            [DeeperTint] = Fill(deeper),
            [Line] = Fill(line),
            [Muted] = Fill(muted),
        };

    /// <summary>
    /// The shadow a button casts: two pixels to the right and two down, with no blur at all.
    /// </summary>
    /// <remarks>
    /// A hard shadow rather than a soft one, and that is the whole of the Win10 raised button: light
    /// comes from the top left, so the shadow falls to the bottom right and finishes where it is — no
    /// blur, because a blur is a surface that is trying to look like it is not flat.
    /// </remarks>
    private static readonly BoxShadow Cast = new()
    {
        OffsetX = 2,
        OffsetY = 2,
        Blur = 0,
        Spread = 0,
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
    /// What a piece of text is, said as a class rather than as a number written where the text is.
    /// </summary>
    /// <remarks>
    /// The look has four sizes and one colour for text that is not the main thing, and every surface that
    /// says text says which of them it is. A size written at the call site is a size that drifts, and the
    /// program stops looking like one program within a release or two.
    /// <para>
    /// They are classes on <see cref="TextBlock"/> rather than a rule per control because text is not a
    /// control: what is being said about it belongs to the text, and travels with it wherever it is put.
    /// </para>
    /// </remarks>
    private static Style[] Roles() =>
        [
            On(
                selector => selector.OfType<TextBlock>().Class("caption"),
                new Setter(TextBlock.FontSizeProperty, 11.0)
            ),
            On(
                selector => selector.OfType<TextBlock>().Class("muted"),
                Brushed(TextBlock.ForegroundProperty, Muted)
            ),
            On(
                selector => selector.OfType<TextBlock>().Class("section"),
                new Setter(TextBlock.FontWeightProperty, FontWeight.SemiBold)
            ),
            On(
                selector => selector.OfType<TextBlock>().Class("title"),
                new Setter(TextBlock.FontSizeProperty, 15.0),
                new Setter(TextBlock.FontWeightProperty, FontWeight.SemiBold)
            ),
            On(
                selector => selector.OfType<TextBlock>().Class("mono"),
                new Setter(TextBlock.FontFamilyProperty, Monospace)
            ),
        ];

    /// <summary>
    /// Every control the kernel and the plugins build their content out of: geometry, spacing, and the
    /// two colours by role.
    /// </summary>
    /// <remarks>
    /// The shape is minimal-flat: a control is a rectangle the same colour as what it sits on, with a
    /// one-pixel edge and a neutral hover, and the two chosen colours are spent on what is selected and on
    /// the one thing a surface is for. The two colours are named as resources rather than baked, so that
    /// changing one at runtime reaches every control that took it.
    /// <para>
    /// The dock's own buttons are excluded here rather than overridden later: a style that is always on
    /// and a style that turns on with a state do not settle by the order they were written, so a rule
    /// that must not apply is written so that it cannot.
    /// </para>
    /// </remarks>
    private Style[] Content() =>
        [
            // A button is flat: a surface the colour of what is behind it, an edge, and a neutral hover.
            // The look has one raised surface and this is not it, so pressing changes a colour rather than
            // a height (Section 10).
            On(
                selector => Pressable(selector),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(12, 5)),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, Line),
                new Setter(TemplatedControl.BackgroundProperty, Brushes.Transparent),
                new Setter(TemplatedControl.TransitionsProperty, Fading(Press))
            ),
            On(
                selector => Pressable(selector).Class(":pointerover"),
                Brushed(TemplatedControl.BackgroundProperty, Tint)
            ),
            On(
                selector => Pressable(selector).Class(":pressed"),
                Brushed(TemplatedControl.BackgroundProperty, DeeperTint)
            ),

            // The one raised thing there is: the action a surface exists for. It is filled with the primary
            // and wears the look's one hard shadow, so that a dialog has exactly one thing that looks like
            // the thing to do.
            On(
                selector => Pressable(selector).Class(PrimaryAction),
                Brushed(TemplatedControl.BackgroundProperty, Primary),
                Brushed(TemplatedControl.ForegroundProperty, PrimaryInk),
                Brushed(TemplatedControl.BorderBrushProperty, PrimaryHeld)
            ),
            On(
                selector => Pressable(selector).Class(PrimaryAction).Class(":pointerover"),
                Brushed(TemplatedControl.BackgroundProperty, PrimaryBright)
            ),
            On(
                selector => Pressable(selector).Class(PrimaryAction).Class(":pressed"),
                Brushed(TemplatedControl.BackgroundProperty, PrimaryHeld),
                Brushed(TemplatedControl.BorderBrushProperty, PrimaryHeld)
            ),

            On(
                selector => selector.OfType<TextBox>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 4)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, Line),
                // A field is a row tall and the ink in it is one line, so the two are not the same height,
                // and the base theme aligns the whole of a field's content by this: left as it comes, the
                // line stands at the top of the box. Left alone across, since that same alignment sizes the
                // field's content where it is set — narrowed to the text, the field would no longer be
                // typed in past its end.
                new Setter(TextBox.VerticalContentAlignmentProperty, VerticalAlignment.Center),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<ComboBox>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 2)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, Line),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<ComboBoxItem>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 3)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Square)
            ),
            On(
                selector => selector.OfType<ListBox>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                Brushed(TemplatedControl.BorderBrushProperty, Line)
            ),
            On(
                selector => selector.OfType<ListBoxItem>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 3)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Square)
            ),
            On(
                selector => selector.OfType<TreeView>(),
                Brushed(TemplatedControl.BorderBrushProperty, Line)
            ),
            On(
                selector => selector.OfType<TreeViewItem>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(2, 2)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Square)
            ),
            On(
                selector => selector.OfType<MenuItem>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(12, 5)),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<ContextMenu>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, Line)
            ),
            On(
                selector => selector.OfType<MenuFlyoutPresenter>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, Line)
            ),
            On(
                selector => selector.OfType<FlyoutPresenter>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, Line)
            ),
            On(
                selector => selector.OfType<TabItem>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(12, 6)),
                new Setter(TemplatedControl.CornerRadiusProperty, Square)
            ),
            On(
                selector => selector.OfType<CheckBox>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<RadioButton>(),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<NumericUpDown>(),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, Line)
            ),
            On(
                selector => selector.OfType<ButtonSpinner>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, Line)
            ),
            On(
                selector => selector.OfType<ProgressBar>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Square)
            ),
            On(
                selector => selector.OfType<Slider>(),
                new Setter(Layoutable.MinHeightProperty, RowHeight)
            ),
            On(
                selector => selector.OfType<Expander>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Square)
            ),
            On(
                selector => selector.OfType<GroupBox>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, Line)
            ),
            On(
                selector => selector.OfType<SplitButton>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, Line)
            ),
            On(
                selector => selector.OfType<DropDownButton>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, Line)
            ),
            On(
                selector => selector.OfType<Separator>(),
                new Setter(Layoutable.HeightProperty, 1.0),
                Brushed(TemplatedControl.BackgroundProperty, Line)
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
    /// The template parts the base theme colours itself, and the few the two colours make wrong.
    /// </summary>
    /// <remarks>
    /// The base theme draws most states on a control's template rather than on the control — a selected
    /// row's fill, a focused field's edge, the tick in a box — so a rule for the control cannot reach
    /// them, and the part has to be named. Every rule here carries an activator as well as the part
    /// name: Avalonia ranks an activated setter above a template binding and above a plain setter,
    /// which is what it takes to be heard over the base theme's styling of the same place.
    /// <para>
    /// The fills mostly need no rule at all, since they follow the accent resources; the ones that do
    /// are the button's fill and shadow, the box that is filled with the primary and the ink that cannot
    /// be white on it, the field's edge, and the fades.
    /// </para>
    /// </remarks>
    private Style[] Parts() =>
        [
            // The base theme paints a hovered or held button's own part, which would take the flat edge off
            // the one family of control the look gives an edge to; a held button's colour is therefore said
            // again here. The primary action is the same in the primary's held colour, and it is the only
            // part in the program that ever carries the shadow.
            On(
                selector => Pressable(selector).Template().Name("PART_ContentPresenter"),
                new Setter(ContentPresenter.TransitionsProperty, Fading(Press))
            ),
            On(
                selector =>
                    Pressable(selector).Class(":pointerover").Template().Name("PART_ContentPresenter"),
                Brushed(ContentPresenter.BorderBrushProperty, Line)
            ),
            On(
                selector =>
                    Pressable(selector).Class(":pressed").Template().Name("PART_ContentPresenter"),
                Brushed(ContentPresenter.BackgroundProperty, DeeperTint),
                Brushed(ContentPresenter.BorderBrushProperty, Line)
            ),
            On(
                selector =>
                    Pressable(selector).Class(PrimaryAction).Template().Name("PART_ContentPresenter"),
                Brushed(ContentPresenter.BoxShadowProperty, Shadow)
            ),
            On(
                selector =>
                    Pressable(selector)
                        .Class(PrimaryAction)
                        .Class(":pointerover")
                        .Template()
                        .Name("PART_ContentPresenter"),
                Brushed(ContentPresenter.BorderBrushProperty, PrimaryHeld)
            ),
            On(
                selector =>
                    Pressable(selector)
                        .Class(PrimaryAction)
                        .Class(":pressed")
                        .Template()
                        .Name("PART_ContentPresenter"),
                Brushed(ContentPresenter.BackgroundProperty, PrimaryHeld),
                Brushed(ContentPresenter.BorderBrushProperty, PrimaryHeld),
                new Setter(ContentPresenter.BoxShadowProperty, default(BoxShadows))
            ),

            Ink(selector => selector.OfType<ListBoxItem>(), "PART_ContentPresenter"),
            Ink(selector => selector.OfType<ComboBoxItem>(), "PART_ContentPresenter"),
            Ink(selector => selector.OfType<TreeViewItem>(), "PART_HeaderPresenter"),

            // A chosen row turns over a fifth of a second rather than a frame, which is the one slow
            // thing in the look (see `Select`). It is set on the part rather than on the item, because
            // the part is where the base theme paints the fill.
            On(
                selector => selector.OfType<ListBoxItem>().Template().Name("PART_ContentPresenter"),
                new Setter(ContentPresenter.TransitionsProperty, Fading(Select))
            ),
            On(
                selector => selector.OfType<ComboBoxItem>().Template().Name("PART_ContentPresenter"),
                new Setter(ContentPresenter.TransitionsProperty, Fading(Select))
            ),
            On(
                selector => selector.OfType<TreeViewItem>().Template().Name("PART_HeaderPresenter"),
                new Setter(ContentPresenter.TransitionsProperty, Fading(Select))
            ),

            // A checked box is filled with the primary and ticked in the ink that can be read on it,
            // where the base theme leaves the box empty and draws the tick in the accent.
            On(
                selector => selector.OfType<CheckBox>().Class(":checked").Template().Name("border"),
                Brushed(Border.BackgroundProperty, Primary),
                Brushed(Border.BorderBrushProperty, Primary)
            ),
            On(
                selector => selector.OfType<CheckBox>().Class(":checked").Template().Name("checkMark"),
                Brushed(Shape.FillProperty, PrimaryInk)
            ),
            On(
                selector => selector.OfType<CheckBox>().Class(":indeterminate").Template().Name("border"),
                Brushed(Border.BackgroundProperty, Primary),
                Brushed(Border.BorderBrushProperty, Primary)
            ),
            On(
                selector =>
                    selector.OfType<CheckBox>().Class(":indeterminate").Template().Name("indeterminateMark"),
                Brushed(Shape.FillProperty, PrimaryInk)
            ),

            // A radio is the same promise in a round shape, and takes the same fill and ink.
            On(
                selector => selector.OfType<RadioButton>().Class(":checked").Template().Name("border"),
                Brushed(Shape.FillProperty, Primary),
                Brushed(Shape.StrokeProperty, Primary)
            ),
            On(
                selector => selector.OfType<RadioButton>().Class(":checked").Template().Name("checkMark"),
                Brushed(Shape.FillProperty, PrimaryInk)
            ),

            // A field that has focus keeps its one pixel and takes it in the accent: the primary is what
            // is chosen, and the accent is what is asking for attention, which is what a focus is. The
            // base theme brightens the border instead, which is a change of the same weight and no help
            // about where the keyboard is.
            On(
                selector => selector.OfType<TextBox>().Class(":focus").Template().Name("border"),
                new Setter(Border.BorderThicknessProperty, Edge),
                Brushed(Border.BorderBrushProperty, Accent)
            ),

            // The track a slider is laid in is chrome, and the thumb in it is the primary — which it
            // follows from the accent resources, and needs no rule here.
            On(
                selector => selector.OfType<Slider>().Template().Name("TrackBackground"),
                new Setter(Border.BorderThicknessProperty, new Thickness(2)),
                Brushed(Border.BorderBrushProperty, Line)
            ),

            // Every part that changes colour on a state fades into it, which is most of the motion
            // there is: a fade is what a flat surface can do without moving anything under the pointer.
            On(
                selector => selector.OfType<TemplatedControl>().Template().Name("PART_ContentPresenter"),
                new Setter(ContentPresenter.TransitionsProperty, Fading())
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
    /// The surfaces the shell owns: the menu bar, the dock headers, and the grabs between regions.
    /// </summary>
    /// <remarks>
    /// The menu bar and every region's header strip take one band of tint, so the window reads as
    /// chrome over content whichever regions are open. A dock header is flat and square, and every one
    /// of them keeps a one-pixel bottom edge that is drawn in nothing until the dock is the one being
    /// shown, when it is drawn in the primary: a header that grew an edge when selected would shift the
    /// headers beside it sideways under the very pointer that selected it.
    /// </remarks>
    private Style[] Chrome() =>
        [
            On(
                selector => selector.OfType<Menu>().Class(MainWindow.MenuBarClass),
                Brushed(TemplatedControl.BackgroundProperty, Tint),
                new Setter(Layoutable.MinHeightProperty, BarHeight),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(4, 0))
            ),
            On(
                selector => selector.OfType<Border>().Class(DockArea.HeadersClass),
                Brushed(Border.BackgroundProperty, Tint),
                Brushed(Border.BorderBrushProperty, Line),
                new Setter(Border.BorderThicknessProperty, new Thickness(0, 0, 0, 1))
            ),

            // The edge is two pixels and transparent when the dock is not the one shown, so that the mark
            // of being chosen is a weight the header always has room for: a header that grew an edge when
            // selected would shift the headers beside it sideways under the very pointer that selected it.
            On(
                selector => selector.OfType<Button>().Class(DockArea.TitleClass),
                new Setter(TemplatedControl.BackgroundProperty, Brushes.Transparent),
                new Setter(TemplatedControl.BorderBrushProperty, Brushes.Transparent),
                new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(0, 0, 0, 2)),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<Button>().Class(DockArea.TitleClass).Class(DockArea.SelectedClass),
                Brushed(TemplatedControl.BackgroundProperty, DeeperTint),
                Brushed(TemplatedControl.BorderBrushProperty, Primary),
                new Setter(TemplatedControl.FontWeightProperty, FontWeight.SemiBold)
            ),
            // A held dock header keeps the primary line it earned by being the one shown, where the
            // container rule above would otherwise have it go transparent for as long as the press.
            On(
                selector => selector.OfType<Button>().Class(DockArea.TitleClass).Class(":pressed"),
                new Setter(TemplatedControl.BackgroundProperty, Brushes.Transparent)
            ),
            On(
                selector =>
                    selector
                        .OfType<Button>()
                        .Class(DockArea.TitleClass)
                        .Class(DockArea.SelectedClass)
                        .Class(":pressed"),
                Brushed(TemplatedControl.BackgroundProperty, DeeperTint)
            ),

            // The base theme repaints a hovered button's edge itself, which would take the primary line
            // off the header of the dock being shown for as long as the pointer rested on it.
            On(
                selector =>
                    selector
                        .OfType<Button>()
                        .Class(DockArea.TitleClass)
                        .Class(DockArea.SelectedClass)
                        .Class(":pointerover")
                        .Template()
                        .Name("PART_ContentPresenter"),
                Brushed(ContentPresenter.BorderBrushProperty, Primary)
            ),

            // The button that closes a region's shown dock, at the far end of the strip: nothing at
            // all until the pointer is on it, and then the red a close is everywhere.
            On(
                selector => selector.OfType<Button>().Class(DockArea.CloseClass),
                new Setter(TemplatedControl.BackgroundProperty, Brushes.Transparent),
                new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(0)),
                new Setter(TemplatedControl.CornerRadiusProperty, Square),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(10, 0)),
                new Setter(Layoutable.MinWidthProperty, 28.0),
                new Setter(Layoutable.MinHeightProperty, BarHeight),
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
                Brushed(TemplatedControl.BackgroundProperty, HairlineVertical)
            ),
            On(
                selector =>
                    selector
                        .OfType<GridSplitter>()
                        .Class(DockArea.SplitterClass)
                        .Class(DockArea.SplitterRowsClass)
                        .Class(":pointerover"),
                Brushed(TemplatedControl.BackgroundProperty, HairlineHorizontal)
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
                Brushed(Border.BackgroundProperty, AccentWash),
                Brushed(Border.BorderBrushProperty, Accent)
            ),
        ];

    /// <summary>
    /// The selector for a button that is a thing to press rather than part of the shell's chrome.
    /// </summary>
    /// <remarks>
    /// The dock's own buttons are excluded: a header and a close are chrome, not a thing to press, and
    /// a rule that is always on cannot be relied on to beat one that turns on with a state. Written so
    /// that it does not match, the question does not arise.
    /// </remarks>
    /// <param name="selector">Where the rule starts.</param>
    private static Selector Pressable(Selector? selector) =>
        selector!
            .OfType<Button>()
            .Not(previous => previous.Class(DockArea.TitleClass))
            .Not(previous => previous.Class(DockArea.CloseClass));

    /// <summary>
    /// What is written on a surface filled with the primary.
    /// </summary>
    /// <remarks>
    /// The base theme fills a selected row with the primary and then puts white on it, which is right
    /// for the blue it was written for and unreadable on a light primary. Black or white by contrast is
    /// what any primary can carry.
    /// </remarks>
    private static Style Ink(Func<Selector?, Selector> ofType, string part) =>
        On(
            selector => ofType(selector).Class(":selected").Template().Name(part),
            Brushed(ContentPresenter.ForegroundProperty, PrimaryInk)
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
    /// A setter whose value is one of the look's own resources.
    /// </summary>
    /// <remarks>
    /// The reference is resolved where the element is rather than here, so a colour is the one the
    /// element's own variant has. Resolving it here would fix it to the variant that was in force when
    /// the theme was applied, and a system that changed variant while the program ran would leave the
    /// overlay behind — half of the window following the system and half of it not. It is also what lets
    /// a colour changed at runtime reach every control that took it (Section 10).
    /// </remarks>
    private static Setter Brushed(AvaloniaProperty property, string resource) =>
        new(property, new DynamicResourceExtension(resource));

    /// <summary>A setter whose value is a colour this theme computes and never the user's to choose.</summary>
    private static Setter Fixed(AvaloniaProperty property, Color colour) =>
        new(property, new SolidColorBrush(colour));

    /// <summary>
    /// What is written on a surface filled with a colour.
    /// </summary>
    /// <remarks>
    /// Black or white, whichever can be read on it. It is the one thing about a chosen colour that
    /// cannot be read off the colour by eye, which is why it is worked out rather than picked, and why
    /// it is open to the tests as well as used here.
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
    /// A factor rather than a colour: the primary is the user's to choose, and a held state that was a
    /// second chosen colour would be one every other primary did without.
    /// </remarks>
    private static Color Darkened(Color colour, double factor) =>
        Color.FromRgb(
            (byte)Math.Round(colour.R * factor),
            (byte)Math.Round(colour.G * factor),
            (byte)Math.Round(colour.B * factor)
        );

    /// <summary>
    /// One colour stepped towards white by a fraction of the way there.
    /// </summary>
    /// <remarks>
    /// The mirror of <see cref="Darkened"/>, and worked out for the same reason: a surface under the
    /// pointer has to lift off the one it was, and how far is a property of the colour rather than of a
    /// second colour chosen beside it.
    /// </remarks>
    private static Color Lightened(Color colour, double amount) =>
        Color.FromRgb(
            (byte)Math.Round(colour.R + ((255 - colour.R) * amount)),
            (byte)Math.Round(colour.G + ((255 - colour.G) * amount)),
            (byte)Math.Round(colour.B + ((255 - colour.B) * amount))
        );

    /// <summary>
    /// How much one colour stands out from another, as the ratio a reader's legibility is held to.
    /// </summary>
    /// <remarks>
    /// The one number that answers "can this be read on that" without knowing which colour either is,
    /// which is what a chosen colour leaves to be worked out.
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
