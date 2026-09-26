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
/// The look: three grounds, rounded surfaces, one vivid primary, and soft shadows.
/// </summary>
/// <remarks>
/// It is the program's own and there is only one of it. What a run chooses is the variant it is drawn
/// in and the colours, read before the window is made (Section 10). Everything else here is stated
/// rather than configured, which is what makes the program look like one thing.
/// <para>
/// The design is the one the sibling project <c>gattipage</c> uses, and the tokens below are that
/// project's own: three grounds (<c>bg</c>, <c>bg-elevated</c>, <c>bg-sunken</c>), a neutral ramp in
/// three steps rather than by opacity (<c>fg</c>, <c>fg-muted</c>, <c>fg-faint</c>), two border
/// weights, 8/5-pixel radii, a soft two-layer shadow, and one vivid primary filled into the things
/// that are chosen. Surfaces are raised rather than flat, so a dock reads as a card on the ground
/// beneath it.
/// </para>
/// <para>
/// The colours are spent by role. <b>Primary</b> fills what is chosen — a chosen row is a wash of it,
/// a chosen tab and the one action of a dialog are filled with it — and it is written into the base
/// theme's own accent resources, so a selected row, a checked box and a selection of text are the
/// primary from one definition. <b>Accent</b> is the second colour, and it is spent on the marks that
/// must not be mistaken for a selection: the band a drag draws, the zone a dragged dock is aimed at,
/// and the hairline a splitter shows under the pointer. The design it comes from needs no such
/// colour, which is why it is the one thing here that is not that design's own.
/// </para>
/// <para>
/// Everything the look paints is drawn from a resource rather than baked into the setter that names
/// it, and that is what makes the colours editable while the program runs: a resource replaced in
/// place reaches every control that took it, where a colour written into a setter would be fixed for
/// the life of the window.
/// </para>
/// <para>
/// The shell marks the surfaces it owns with classes, which is how a rule here addresses a dock without
/// knowing what one is: <see cref="MainWindow.MenuBarClass"/> on the menu bar,
/// <see cref="DockArea.HeadersClass"/> on a region's header strip,
/// <see cref="DockArea.TabsClass"/> on the segmented control the tabs sit in,
/// <see cref="DockArea.TitleClass"/> on a dock's own tab — with <see cref="DockArea.SelectedClass"/> on
/// the one being shown — <see cref="DockArea.CloseClass"/> on the button that closes it,
/// <see cref="DockArea.SplitterClass"/> on the grab between regions, and
/// <see cref="DockArea.DropZoneClass"/> on where a dragged dock would land.
/// </para>
/// </remarks>
internal sealed class RorolalaTheme
{
    /// <summary>
    /// The look, in the colours the user chooses.
    /// </summary>
    /// <remarks>
    /// Two colours are configured, and a third may be named rather than derived: the ink written on a
    /// filled surface is worked out by contrast unless the file names one, because the design this
    /// comes from names its own (a dark green on a lime rather than a black) and a named value is a
    /// choice the program should keep.
    /// </remarks>
    /// <param name="primary">The colour what is chosen is drawn in.</param>
    /// <param name="accent">The colour the drag marks are drawn in.</param>
    /// <param name="primaryText">What is written on the primary, or nothing to work it out.</param>
    public RorolalaTheme(Color primary, Color accent, Color? primaryText)
    {
        _primary = primary;
        _accent = accent;
        _primaryText = primaryText ?? InkOn(primary);

        // What the design hovers a filled action with: its own colour mixed a little of the way towards
        // white, which lifts a lime without washing it out. It is derived before the palette is written
        // because the palette states it as a resource like every other colour.
        _primaryBright = Mix(_primary, Colors.White, 0.15);

        // The wash a chosen row is filled with. It is stated by the design as two alphas rather than
        // derived, because a wash has to stay a wash on both grounds and a factor would not.
        _selectionLight = WithAlpha(primary, 0x59);
        _selectionDark = WithAlpha(primary, 0x4D);

        // The ring a focused field wears. The design draws it as an outline two pixels out, which is
        // this: a shadow with a spread and no blur, drawn outside the border box.
        _ring = new BoxShadows(
            new BoxShadow
            {
                OffsetX = 0,
                OffsetY = 0,
                Blur = 0,
                Spread = 2,
                Color = WithAlpha(primary, 0x8C),
            }
        );

        _light = Variant(Grounds.Light, selection: _selectionLight);
        _dark = Variant(Grounds.Dark, selection: _selectionDark);

        _palette = new ResourceDictionary
        {
            // A scrollbar is a hairline of chrome rather than a control.
            ["ScrollBarThickness"] = 12.0,
            ["ScrollBarThumbThickness"] = 7.0,

            // The mark a warning is raised with. It is the one amber, fixed rather than chosen, and it
            // is picked to carry on either ground: what a warning is coloured is not a preference.
            [Warn] = Fill(Color.FromRgb(0xD4, 0xA0, 0x17)),

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
    private const double BodySize = 14.0;

    /// <summary>The height of a row of a list, a tree, or a field.</summary>
    private const double RowHeight = 30.0;

    /// <summary>
    /// The height of the menu bar, which is a fifth shorter than the other bands.
    /// </summary>
    /// <remarks>
    /// A menu bar is read and then left alone, where a strip carries the tabs and the commands that are
    /// worked in all session: the same height made the one thing that is only looked at as heavy as the
    /// things that are used.
    /// </remarks>
    private const double MenuBarHeight = 29.0;

    /// <summary>How rounded a card is.</summary>
    private static readonly CornerRadius Radius = new(8);

    /// <summary>How rounded a control is.</summary>
    private static readonly CornerRadius Small = new(5);

    /// <summary>How rounded a pill is: enough that nothing narrower can be.</summary>
    private static readonly CornerRadius Pill = new(999);

    /// <summary>The one edge a surface wears.</summary>
    private static readonly Thickness Edge = new(1);

    /// <summary>How long a surface answers the pointer with.</summary>
    private static readonly TimeSpan Fade = TimeSpan.FromMilliseconds(120);

    /// <summary>How long a drop zone takes to light up.</summary>
    private static readonly TimeSpan Light = TimeSpan.FromMilliseconds(120);

    /// <summary>
    /// The typeface the program is set in.
    /// </summary>
    /// <remarks>
    /// The platform's own face, as the design this comes from uses: it ships no font, so two machines
    /// showing the same two colours are the same program either way. A face the program carried would
    /// be one more thing to ship and one more thing to disagree about.
    /// </remarks>
    private static readonly FontFamily Face = FontFamily.Default;

    /// <summary>
    /// The typeface a column of data is set in.
    /// </summary>
    /// <remarks>
    /// A fallback list rather than one name, because no monospace is shipped and which one a system has
    /// is the system's business: the log is a table of aligned columns, and any monospace is better at
    /// that than the proportional face every other surface is set in.
    /// </remarks>
    private static readonly FontFamily Monospace = new(
        "JetBrains Mono, Cascadia Mono, Consolas, DejaVu Sans Mono, Liberation Mono, monospace"
    );

    // The look's own resources, the peers of the base theme's own names below. They are named rather
    // than written into setters so that a colour changed at runtime reaches what took it.
    private const string Bg = "rorolala.bg";
    private const string Elevated = "rorolala.bg.elevated";
    private const string Sunken = "rorolala.bg.sunken";
    private const string Foreground = "rorolala.fg";
    private const string Muted = "rorolala.fg.muted";
    private const string Faint = "rorolala.fg.faint";
    private const string BorderLine = "rorolala.border";
    private const string Strong = "rorolala.border.strong";
    private const string Primary = "rorolala.primary";
    private const string PrimaryBright = "rorolala.primary.bright";
    private const string PrimaryText = "rorolala.primary.text";
    private const string Accent = "rorolala.accent";
    private const string Selection = "rorolala.selection";
    private const string Warn = "rorolala.warn";
    private const string Shadow = "rorolala.shadow";
    private const string Ring = "rorolala.ring";

    /// <summary>
    /// The class the one action a surface exists for wears.
    /// </summary>
    /// <remarks>
    /// The design does not single out one action, but a dialog has one and the shell has to be able to
    /// say which: a class rather than a property the toolkit owns, so which button is the action is the
    /// shell's to say and the look's to draw.
    /// </remarks>
    private const string PrimaryAction = "primary";

    /// <summary>The colour what is chosen is drawn in.</summary>
    private readonly Color _primary;

    /// <summary>The primary lifted towards white, which the design hovers a filled action with.</summary>
    private readonly Color _primaryBright;

    /// <summary>The colour the drag marks are drawn in.</summary>
    private readonly Color _accent;

    /// <summary>What is written on a surface filled with the primary.</summary>
    private readonly Color _primaryText;

    /// <summary>The wash a chosen row is filled with, on the light ground.</summary>
    private readonly Color _selectionLight;

    /// <summary>The wash a chosen row is filled with, on the dark ground.</summary>
    private readonly Color _selectionDark;

    /// <summary>The ring a focused field wears.</summary>
    private readonly BoxShadows _ring;

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
    /// One variant's neutral ground, which is the design's own token set.
    /// </summary>
    /// <remarks>
    /// Three grounds rather than one: content sits on the ground, chrome and a control sit on the
    /// elevated one, and what is hovered, held or welled sits in the sunken one. The ramp of ink is
    /// three steps rather than an opacity, so that text is legible by construction rather than by
    /// arithmetic — and the two semantic pairs are the design's own, not the user's to choose, because
    /// what a failure is coloured is not a preference.
    /// </remarks>
    private readonly record struct Grounds(
        Color Bg,
        Color Elevated,
        Color Sunken,
        Color Fg,
        Color Muted,
        Color Faint,
        Color Border,
        Color Strong,
        Color Add,
        Color AddBg,
        Color Del,
        Color DelBg,
        Color Shadow,
        Color ShadowSoft
    )
    {
        /// <summary>The light ground.</summary>
        /// <remarks>
        /// The ramp is hue-free. It began as the design's own warm sage, tuned to its lime primary; kept
        /// that way under a primary the user chooses it reads as a green film over the whole window and
        /// fights whatever colour is set — so it carries no hue at all, and the two configured colours are
        /// the only colour in a window that has not drawn an added or a removed thing.
        /// </remarks>
        public static readonly Grounds Light = new(
            Bg: Color.FromRgb(0xF7, 0xF7, 0xF7),
            Elevated: Color.FromRgb(0xFF, 0xFF, 0xFF),
            Sunken: Color.FromRgb(0xEC, 0xEC, 0xEC),
            Fg: Color.FromRgb(0x1D, 0x1D, 0x1D),
            Muted: Color.FromRgb(0x5D, 0x5D, 0x5D),
            Faint: Color.FromRgb(0x8B, 0x8B, 0x8B),
            Border: Color.FromRgb(0xDC, 0xDC, 0xDC),
            Strong: Color.FromRgb(0xC2, 0xC2, 0xC2),
            Add: Color.FromRgb(0x1F, 0x8A, 0x3B),
            AddBg: Color.FromRgb(0xE2, 0xF6, 0xE6),
            Del: Color.FromRgb(0xC0, 0x36, 0x2C),
            DelBg: Color.FromRgb(0xFB, 0xE6, 0xE3),
            Shadow: Color.FromArgb(0x0F, 0x00, 0x00, 0x00),
            ShadowSoft: Color.FromArgb(0x0F, 0x00, 0x00, 0x00)
        );

        /// <summary>The dark ground.</summary>
        public static readonly Grounds Dark = new(
            Bg: Color.FromRgb(0x14, 0x14, 0x14),
            Elevated: Color.FromRgb(0x1C, 0x1C, 0x1C),
            Sunken: Color.FromRgb(0x0F, 0x0F, 0x0F),
            Fg: Color.FromRgb(0xE6, 0xE6, 0xE6),
            Muted: Color.FromRgb(0x9A, 0x9A, 0x9A),
            Faint: Color.FromRgb(0x6B, 0x6B, 0x6B),
            Border: Color.FromRgb(0x2C, 0x2C, 0x2C),
            Strong: Color.FromRgb(0x3C, 0x3C, 0x3C),
            Add: Color.FromRgb(0x5F, 0xD0, 0x7A),
            AddBg: Color.FromRgb(0x16, 0x30, 0x1D),
            Del: Color.FromRgb(0xFF, 0x7A, 0x6B),
            DelBg: Color.FromRgb(0x3A, 0x1D, 0x1A),
            Shadow: Color.FromArgb(0x66, 0x00, 0x00, 0x00),
            ShadowSoft: Color.FromArgb(0x59, 0x00, 0x00, 0x00)
        );

        /// <summary>The soft shadow the design raises a surface with.</summary>
        /// <remarks>
        /// Two shadows rather than one, as the design writes it: a hairline of contact and a wide soft
        /// one under it. The constructor takes the first and the rest, so the second is given as a list
        /// of one.
        /// </remarks>
        public BoxShadows Raised => new(
            new BoxShadow
            {
                OffsetX = 0,
                OffsetY = 1,
                Blur = 2,
                Spread = 0,
                Color = Shadow,
            },
            [
                new BoxShadow
                {
                    OffsetX = 0,
                    OffsetY = 8,
                    Blur = 24,
                    Spread = 0,
                    Color = ShadowSoft,
                },
            ]
        );
    }

    /// <summary>
    /// Writes this look's colours over an older look's palette, in place.
    /// </summary>
    /// <remarks>
    /// It replaces the entries of the dictionaries that are already attached rather than swapping the
    /// dictionaries for new ones, because a resource replaced in place is what raises the change that
    /// reaches every control that took it.
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
    /// <remarks>
    /// The look's own names and <em>the base theme's</em> are both written, because the base theme
    /// draws most of what a control does from its own resources: naming those is what turns a whole
    /// program's selection, hover, border and ink with two colours rather than a rule per control. The
    /// mapping is the design's: a control's surface is the elevated ground, what is hovered or held is
    /// the sunken one, the two border weights are two of the three, and a chosen row is the primary
    /// wash the base theme's accent family is filled with.
    /// </remarks>
    /// <param name="grounds">The variant's own neutrals.</param>
    /// <param name="selection">The wash a chosen row is filled with.</param>
    private ResourceDictionary Variant(Grounds grounds, Color selection) =>
        new()
        {
            // --- the base theme's own names, pointed at the design's colours ---------------------- //
            ["ThemeBackgroundColor"] = grounds.Elevated,
            ["ThemeBackgroundBrush"] = Fill(grounds.Elevated),
            ["ThemeBorderLowColor"] = grounds.Border,
            ["ThemeBorderLowBrush"] = Fill(grounds.Border),
            ["ThemeBorderMidColor"] = grounds.Border,
            ["ThemeBorderMidBrush"] = Fill(grounds.Border),
            ["ThemeBorderHighColor"] = grounds.Strong,
            ["ThemeBorderHighBrush"] = Fill(grounds.Strong),
            ["ThemeControlLowColor"] = grounds.Sunken,
            ["ThemeControlLowBrush"] = Fill(grounds.Sunken),
            ["ThemeControlMidColor"] = grounds.Elevated,
            ["ThemeControlMidBrush"] = Fill(grounds.Elevated),
            ["ThemeControlMidHighColor"] = grounds.Border,
            ["ThemeControlMidHighBrush"] = Fill(grounds.Border),
            ["ThemeControlHighColor"] = grounds.Sunken,
            ["ThemeControlHighBrush"] = Fill(grounds.Sunken),
            ["ThemeControlVeryHighColor"] = grounds.Strong,
            ["ThemeControlVeryHighBrush"] = Fill(grounds.Strong),
            ["ThemeControlHighlightLowColor"] = grounds.Sunken,
            ["ThemeControlHighlightLowBrush"] = Fill(grounds.Sunken),
            // A row under the pointer is the sunken ground, not the opaque grey the base theme ships:
            // it is the one place where a hover would otherwise be the wrong colour entirely.
            ["ThemeControlHighlightMidColor"] = grounds.Sunken,
            ["ThemeControlHighlightMidBrush"] = Fill(grounds.Sunken),
            ["ThemeControlHighlightHighColor"] = grounds.Strong,
            ["ThemeControlHighlightHighBrush"] = Fill(grounds.Strong),
            ["ThemeForegroundColor"] = grounds.Fg,
            ["ThemeForegroundBrush"] = Fill(grounds.Fg),
            ["ThemeForegroundLowColor"] = grounds.Faint,
            ["ThemeForegroundLowBrush"] = Fill(grounds.Faint),
            ["ErrorColor"] = grounds.Del,
            ["ErrorBrush"] = Fill(grounds.Del),
            ["ErrorLowColor"] = WithAlpha(grounds.Del, 0x10),
            ["ErrorLowBrush"] = Fill(WithAlpha(grounds.Del, 0x10)),
            ["HyperlinkVisitedColor"] = grounds.Faint,
            ["HyperlinkVisitedBrush"] = Fill(grounds.Faint),
            ["CaptionButtonForeground"] = grounds.Fg,
            ["CaptionButtonBackground"] = grounds.Elevated,
            ["CaptionButtonBorderBrush"] = grounds.Border,

            // A selection of text is the same wash a chosen row is: one idea, one colour.
            ["HighlightColor"] = selection,
            ["HighlightBrush"] = Fill(selection),
            ["HighlightColor2"] = selection,
            ["HighlightBrush2"] = Fill(selection),
            ["HighlightForegroundColor"] = grounds.Fg,
            ["HighlightForegroundBrush"] = Fill(grounds.Fg),

            // The primary family. The alphas are the base theme's own shape, and the fourth is the wash
            // the design fills a chosen row with rather than the base theme's own fifth.
            ["ThemeAccentColor"] = _primary,
            ["ThemeAccentColor2"] = WithAlpha(_primary, 0x99),
            ["ThemeAccentColor3"] = WithAlpha(_primary, 0x66),
            ["ThemeAccentColor4"] = selection,
            ["ThemeAccentBrush"] = Fill(_primary),
            ["ThemeAccentBrush2"] = Fill(WithAlpha(_primary, 0x99)),
            ["ThemeAccentBrush3"] = Fill(WithAlpha(_primary, 0x66)),
            ["ThemeAccentBrush4"] = Fill(selection),

            // --- the look's own names ------------------------------------------------------------- //
            [Bg] = Fill(grounds.Bg),
            [Elevated] = Fill(grounds.Elevated),
            [Sunken] = Fill(grounds.Sunken),
            [Foreground] = Fill(grounds.Fg),
            [Muted] = Fill(grounds.Muted),
            [Faint] = Fill(grounds.Faint),
            [BorderLine] = Fill(grounds.Border),
            [Strong] = Fill(grounds.Strong),
            [Primary] = Fill(_primary),
            [PrimaryBright] = Fill(_primaryBright),
            [PrimaryText] = Fill(_primaryText),
            [Accent] = Fill(_accent),
            [Selection] = Fill(selection),
            [Shadow] = grounds.Raised,
            [Ring] = _ring,
            ["rorolala.add"] = Fill(grounds.Add),
            ["rorolala.add.bg"] = Fill(grounds.AddBg),
            ["rorolala.del"] = Fill(grounds.Del),
            ["rorolala.del.bg"] = Fill(grounds.DelBg),
        };

    /// <summary>
    /// The accent-only marks, which do not differ by variant and so are stated once.
    /// </summary>
    /// <remarks>
    /// A hairline through the middle of a splitter is a gradient with hard stops rather than a colour,
    /// because there is nothing to put a line in: a splitter is one surface with no template part a
    /// theme may address, so the only way to draw one pixel of line inside four pixels of grip is to
    /// paint the grip clear and the middle of it not.
    /// </remarks>
    /// <param name="vertical">Whether the line runs down the splitter rather than across it.</param>
    private IBrush Hairline(bool vertical)
    {
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
    /// The design has one size for a heading, one for a body, one for a secondary line, and one
    /// <b>label</b>: eleven pixels, bold, letter-spaced, and set by the caller in capitals, which is
    /// the design's loudest small thing and the reason a section or a column of a table reads as a
    /// header rather than as more text.
    /// <para>
    /// They are classes on <see cref="TextBlock"/> rather than a rule per control because text is not a
    /// control: what is being said about it belongs to the text, and travels with it wherever it is put.
    /// </para>
    /// </remarks>
    private static Style[] Roles() =>
        [
            On(
                selector => selector.OfType<TextBlock>().Class("title"),
                new Setter(TextBlock.FontSizeProperty, 16.0),
                new Setter(TextBlock.FontWeightProperty, FontWeight.SemiBold)
            ),
            On(
                selector => selector.OfType<TextBlock>().Class("label"),
                new Setter(TextBlock.FontSizeProperty, 11.0),
                new Setter(TextBlock.FontWeightProperty, FontWeight.Bold),
                new Setter(TextBlock.LetterSpacingProperty, 0.9),
                Brushed(TextBlock.ForegroundProperty, Faint)
            ),
            On(
                selector => selector.OfType<TextBlock>().Class("caption"),
                new Setter(TextBlock.FontSizeProperty, 12.5)
            ),
            On(
                selector => selector.OfType<TextBlock>().Class("muted"),
                Brushed(TextBlock.ForegroundProperty, Muted)
            ),
            On(
                selector => selector.OfType<TextBlock>().Class("faint"),
                Brushed(TextBlock.ForegroundProperty, Faint)
            ),
            On(
                selector => selector.OfType<TextBlock>().Class("mono"),
                new Setter(TextBlock.FontFamilyProperty, Monospace),
                new Setter(TextBlock.FontSizeProperty, 12.5)
            ),
        ];

    /// <summary>
    /// Every control the kernel and the plugins build their content out of.
    /// </summary>
    /// <remarks>
    /// A control sits on the elevated ground with one border; under the pointer it drops to the sunken
    /// one and its border firms up, which is the whole of the interaction a control has beyond being
    /// chosen. Everything is rounded — a card at eight, a control at five — because the design is a
    /// raised one rather than a flat one.
    /// </remarks>
    private static Style[] Content() =>
        [
            // The window is the ground the cards sit on, where the base theme would have it be the same
            // colour as a card: the design is a stack of surfaces, and that only reads if there is a
            // ground beneath them.
            On(
                selector => selector.OfType<Window>(),
                Brushed(TemplatedControl.BackgroundProperty, Bg)
            ),

            On(
                selector => Pressable(selector),
                Brushed(TemplatedControl.BackgroundProperty, Elevated),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, BorderLine),
                new Setter(TemplatedControl.CornerRadiusProperty, Small),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(11, 5)),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector => Pressable(selector).Class(":pointerover"),
                Brushed(TemplatedControl.BackgroundProperty, Sunken),
                Brushed(TemplatedControl.BorderBrushProperty, Strong)
            ),

            // The one action a surface exists for: filled with the primary, in the ink the design names
            // for it, and set a weight heavier so that it reads as the thing to do.
            On(
                selector => Pressable(selector).Class(PrimaryAction),
                Brushed(TemplatedControl.BackgroundProperty, Primary),
                Brushed(TemplatedControl.ForegroundProperty, PrimaryText),
                new Setter(TemplatedControl.FontWeightProperty, FontWeight.SemiBold)
            ),
            On(
                selector => Pressable(selector).Class(PrimaryAction).Class(":pointerover"),
                Brushed(TemplatedControl.BackgroundProperty, PrimaryBright)
            ),

            // A tool is a square hit area and a ghost is the same idea with a word in it: both draw
            // nothing of their own until the pointer is on them. They are what the chrome is made of,
            // where a row of raised cards would read as a row of things to do rather than to press.
            On(
                selector => Pressable(selector).Class("tool"),
                new Setter(TemplatedControl.BackgroundProperty, Brushes.Transparent),
                new Setter(TemplatedControl.BorderBrushProperty, Brushes.Transparent),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(0)),
                new Setter(ContentControl.HorizontalContentAlignmentProperty, HorizontalAlignment.Center),
                new Setter(ContentControl.VerticalContentAlignmentProperty, VerticalAlignment.Center),
                new Setter(Layoutable.WidthProperty, 28.0),
                new Setter(Layoutable.HeightProperty, 28.0)
            ),
            On(
                selector => Pressable(selector).Class("ghost"),
                new Setter(TemplatedControl.BackgroundProperty, Brushes.Transparent),
                new Setter(TemplatedControl.BorderBrushProperty, Brushes.Transparent)
            ),
            On(
                selector => Pressable(selector).Class("tool").Class(":pointerover"),
                Brushed(TemplatedControl.BackgroundProperty, Sunken),
                Brushed(TemplatedControl.BorderBrushProperty, BorderLine)
            ),
            On(
                selector => Pressable(selector).Class("ghost").Class(":pointerover"),
                Brushed(TemplatedControl.BackgroundProperty, Sunken),
                Brushed(TemplatedControl.BorderBrushProperty, BorderLine)
            ),

            On(
                selector => selector.OfType<TextBox>(),
                Brushed(TemplatedControl.BackgroundProperty, Elevated),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, BorderLine),
                new Setter(TemplatedControl.CornerRadiusProperty, Small),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 4)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                // A field is a row tall and the ink in it is one line, so the two are not the same
                // height, and the base theme aligns the whole of a field's content by this: left as it
                // comes, the line stands at the top of the box. Left alone across, since that same
                // alignment sizes the field's content where it is set.
                new Setter(TextBox.VerticalContentAlignmentProperty, VerticalAlignment.Center),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<ComboBox>(),
                Brushed(TemplatedControl.BackgroundProperty, Elevated),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, BorderLine),
                new Setter(TemplatedControl.CornerRadiusProperty, Small),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 2)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<ComboBoxItem>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 4)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Small)
            ),
            On(
                selector => selector.OfType<ListBox>(),
                Brushed(TemplatedControl.BorderBrushProperty, BorderLine),
                new Setter(TemplatedControl.CornerRadiusProperty, Radius)
            ),
            On(
                selector => selector.OfType<ListBoxItem>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 4)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Small)
            ),
            On(
                selector => selector.OfType<TreeView>(),
                Brushed(TemplatedControl.BorderBrushProperty, BorderLine),
                new Setter(TemplatedControl.CornerRadiusProperty, Radius)
            ),
            On(
                selector => selector.OfType<TreeViewItem>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(2, 1)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Small)
            ),
            On(
                selector => selector.OfType<MenuItem>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Small),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 4)),
                // Centred, because the menu bar is taller than a menu item: left stretched, an item's
                // word sits at its top and the bar reads as if it were two rows high.
                new Setter(Layoutable.VerticalAlignmentProperty, VerticalAlignment.Center),
                new Setter(ContentControl.VerticalContentAlignmentProperty, VerticalAlignment.Center),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<ContextMenu>(),
                Brushed(TemplatedControl.BackgroundProperty, Elevated),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, BorderLine),
                new Setter(TemplatedControl.CornerRadiusProperty, Radius),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(4))
            ),
            On(
                selector => selector.OfType<MenuFlyoutPresenter>(),
                Brushed(TemplatedControl.BackgroundProperty, Elevated),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, BorderLine),
                new Setter(TemplatedControl.CornerRadiusProperty, Radius)
            ),
            On(
                selector => selector.OfType<FlyoutPresenter>(),
                Brushed(TemplatedControl.BackgroundProperty, Elevated),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, BorderLine),
                new Setter(TemplatedControl.CornerRadiusProperty, Radius)
            ),
            On(
                selector => selector.OfType<TabItem>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Small),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(12, 6))
            ),
            On(
                selector => selector.OfType<CheckBox>(),
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
                new Setter(TemplatedControl.CornerRadiusProperty, Small)
            ),
            On(
                selector => selector.OfType<ButtonSpinner>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Small)
            ),
            On(
                selector => selector.OfType<ProgressBar>(),
                new Setter(Layoutable.HeightProperty, 10.0),
                new Setter(TemplatedControl.CornerRadiusProperty, Pill)
            ),
            On(
                selector => selector.OfType<Slider>(),
                new Setter(Layoutable.MinHeightProperty, RowHeight)
            ),
            On(
                selector => selector.OfType<Expander>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Radius)
            ),
            On(
                selector => selector.OfType<GroupBox>(),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, BorderLine),
                new Setter(TemplatedControl.CornerRadiusProperty, Radius)
            ),
            On(
                selector => selector.OfType<SplitButton>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Small),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, BorderLine)
            ),
            On(
                selector => selector.OfType<DropDownButton>(),
                new Setter(TemplatedControl.CornerRadiusProperty, Small),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, BorderLine)
            ),
            On(
                selector => selector.OfType<Separator>(),
                new Setter(Layoutable.HeightProperty, 1.0),
                Brushed(TemplatedControl.BackgroundProperty, BorderLine)
            ),
            On(
                selector => selector.OfType<ToolTip>(),
                Brushed(TemplatedControl.BackgroundProperty, Elevated),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                Brushed(TemplatedControl.BorderBrushProperty, BorderLine),
                new Setter(TemplatedControl.CornerRadiusProperty, Small),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 4))
            ),
        ];

    /// <summary>
    /// The template parts the base theme colours itself.
    /// </summary>
    /// <remarks>
    /// The base theme draws most states on a control's template rather than on the control — a hovered
    /// row's fill, a focused field's edge, the tick in a box — so a rule for the control cannot reach
    /// them and the part has to be named. Every rule here carries an activator as well as the part
    /// name: Avalonia ranks an activated setter above a template binding and above a plain setter,
    /// which is what it takes to be heard over the base theme's styling of the same place.
    /// </remarks>
    private static Style[] Parts() =>
        [
            // The base theme paints a hovered or held button's own part, and firmed up its border to a
            // weight the design does not have; both are said again here.
            On(
                selector => Pressable(selector).Template().Name("PART_ContentPresenter"),
                new Setter(ContentPresenter.TransitionsProperty, Fading())
            ),
            On(
                selector =>
                    Pressable(selector).Class(":pointerover").Template().Name("PART_ContentPresenter"),
                Brushed(ContentPresenter.BorderBrushProperty, Strong)
            ),
            On(
                selector =>
                    Pressable(selector).Class(PrimaryAction).Class(":pointerover").Template().Name("PART_ContentPresenter"),
                Brushed(ContentPresenter.BorderBrushProperty, Primary)
            ),

            // A chosen box and a chosen radio are filled with the primary and marked in the ink the
            // design names for it.
            On(
                selector => selector.OfType<CheckBox>().Class(":checked").Template().Name("border"),
                Brushed(Border.BackgroundProperty, Primary),
                Brushed(Border.BorderBrushProperty, Primary)
            ),
            On(
                selector => selector.OfType<CheckBox>().Class(":checked").Template().Name("checkMark"),
                Brushed(Shape.FillProperty, PrimaryText)
            ),
            On(
                selector => selector.OfType<CheckBox>().Class(":indeterminate").Template().Name("border"),
                Brushed(Border.BackgroundProperty, Primary),
                Brushed(Border.BorderBrushProperty, Primary)
            ),
            On(
                selector =>
                    selector.OfType<CheckBox>().Class(":indeterminate").Template().Name("indeterminateMark"),
                Brushed(Shape.FillProperty, PrimaryText)
            ),
            On(
                selector => selector.OfType<RadioButton>().Class(":checked").Template().Name("border"),
                Brushed(Shape.FillProperty, Primary),
                Brushed(Shape.StrokeProperty, Primary)
            ),
            On(
                selector => selector.OfType<RadioButton>().Class(":checked").Template().Name("checkMark"),
                Brushed(Shape.FillProperty, PrimaryText)
            ),

            // A field that has focus keeps its one border and grows the ring the design draws: a shadow
            // with a spread and no blur, which is an outline two pixels out from the border box. The
            // border itself does not move, because a focus is a thing beside the field rather than a
            // change to it.
            On(
                selector => selector.OfType<TextBox>().Class(":focus").Template().Name("border"),
                new Setter(Border.BorderThicknessProperty, Edge),
                Brushed(Border.BorderBrushProperty, BorderLine),
                Brushed(Border.BoxShadowProperty, Ring)
            ),

            On(
                selector => selector.OfType<Slider>().Template().Name("TrackBackground"),
                new Setter(Border.BorderThicknessProperty, new Thickness(2)),
                Brushed(Border.BorderBrushProperty, BorderLine)
            ),

            // A menu and a tooltip are raised surfaces, and the shadow belongs to the part that draws
            // them rather than to the control, which has none of its own to set.
            On(
                selector => selector.OfType<MenuFlyoutPresenter>().Template().Name("LayoutRoot"),
                Brushed(Border.BoxShadowProperty, Shadow)
            ),
            On(
                selector => selector.OfType<FlyoutPresenter>().Template().Name("LayoutRoot"),
                Brushed(Border.BoxShadowProperty, Shadow)
            ),
            On(
                selector => selector.OfType<ToolTip>().Template().Name("PART_ContentPresenter"),
                Brushed(ContentPresenter.BoxShadowProperty, Shadow)
            ),

            // Every part that changes colour on a state fades into it.
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
    /// The surfaces the shell owns: the menu bar, the dock strips, and the grabs between regions.
    /// </summary>
    /// <remarks>
    /// Chrome is the elevated ground with one border under it, so the window reads as a stack of cards:
    /// the menu bar, each strip, and the content between them. A dock's tabs are a segmented control —
    /// one bordered rounded container, the tab being shown filled with the primary — which is the
    /// design's own answer to a tab strip and the reason there is no underline anywhere.
    /// </remarks>
    private Style[] Chrome() =>
        [
            On(
                selector => selector.OfType<Menu>().Class(MainWindow.MenuBarClass),
                Brushed(TemplatedControl.BackgroundProperty, Elevated),
                new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(0, 0, 0, 1)),
                Brushed(TemplatedControl.BorderBrushProperty, BorderLine),
                new Setter(Layoutable.MinHeightProperty, MenuBarHeight),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 0))
            ),
            On(
                selector => selector.OfType<Border>().Class(DockArea.HeadersClass),
                Brushed(Border.BackgroundProperty, Elevated),
                Brushed(Border.BorderBrushProperty, BorderLine),
                new Setter(Border.BorderThicknessProperty, new Thickness(0, 0, 0, 1))
            ),

            // The container the tabs sit in: one border, one radius, and the overflow clipped so that
            // the corners of the tab being shown are the container's.
            On(
                selector => selector.OfType<Border>().Class(DockArea.TabsClass),
                new Setter(Border.BorderThicknessProperty, Edge),
                Brushed(Border.BorderBrushProperty, BorderLine),
                new Setter(Border.CornerRadiusProperty, Small),
                new Setter(Border.ClipToBoundsProperty, true)
            ),
            On(
                selector => selector.OfType<Button>().Class(DockArea.TitleClass),
                new Setter(TemplatedControl.BackgroundProperty, Brushes.Transparent),
                new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(0)),
                new Setter(TemplatedControl.CornerRadiusProperty, new CornerRadius(0)),
                Brushed(TemplatedControl.ForegroundProperty, Muted),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<Button>().Class(DockArea.TitleClass).Class(":pointerover"),
                Brushed(TemplatedControl.BackgroundProperty, Sunken),
                Brushed(TemplatedControl.ForegroundProperty, Foreground)
            ),
            On(
                selector => selector.OfType<Button>().Class(DockArea.TitleClass).Class(DockArea.SelectedClass),
                Brushed(TemplatedControl.BackgroundProperty, Primary),
                Brushed(TemplatedControl.ForegroundProperty, PrimaryText),
                new Setter(TemplatedControl.FontWeightProperty, FontWeight.SemiBold)
            ),
            // A held tab keeps the fill it has, whether it is the one shown or not.
            On(
                selector => selector.OfType<Button>().Class(DockArea.TitleClass).Class(":pressed"),
                Brushed(TemplatedControl.BackgroundProperty, Sunken)
            ),
            On(
                selector =>
                    selector
                        .OfType<Button>()
                        .Class(DockArea.TitleClass)
                        .Class(DockArea.SelectedClass)
                        .Class(":pressed"),
                Brushed(TemplatedControl.BackgroundProperty, Primary)
            ),

            // The button that closes a region's shown dock: nothing until the pointer is on it, and then
            // the design's own red wash rather than a filled red, because nothing here is filled red.
            On(
                selector => selector.OfType<Button>().Class(DockArea.CloseClass),
                new Setter(TemplatedControl.BackgroundProperty, Brushes.Transparent),
                new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(0)),
                new Setter(TemplatedControl.CornerRadiusProperty, Small),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(0)),
                new Setter(Layoutable.WidthProperty, 26.0),
                new Setter(Layoutable.HeightProperty, 26.0),
                Brushed(TemplatedControl.ForegroundProperty, Faint),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector =>
                    selector
                        .OfType<Button>()
                        .Class(DockArea.CloseClass)
                        .Class(":pointerover")
                        .Template()
                        .Name("PART_ContentPresenter"),
                Brushed(ContentPresenter.BackgroundProperty, "rorolala.del.bg"),
                Brushed(ContentPresenter.ForegroundProperty, "rorolala.del")
            ),

            // A splitter draws nothing until the pointer is on it, and then the accent hairline through
            // its middle: the grab stays wide enough to hit, and a line that wide would be a bar.
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

            // Where a dragged dock could land. All of them are up while a drag is on, because they are
            // the question — which regions are there to land in — and the one being aimed at is drawn in
            // the accent, because it is the answer and must not read as a selection.
            On(
                selector => selector.OfType<Border>().Class(DockArea.DropZoneClass),
                new Setter(Border.BorderThicknessProperty, Edge),
                Brushed(Border.BorderBrushProperty, Strong),
                Brushed(Border.BackgroundProperty, Sunken),
                new Setter(Border.CornerRadiusProperty, Radius),
                new Setter(Border.TransitionsProperty, Fading(Light))
            ),
            On(
                selector =>
                    selector
                        .OfType<Border>()
                        .Class(DockArea.DropZoneClass)
                        .Class(DockArea.DropTargetClass),
                NewBrush(Border.BackgroundProperty, WithAlpha(_accent, 0x40)),
                Brushed(Border.BorderBrushProperty, Accent)
            ),
        ];

    /// <summary>
    /// The selector for a button that is a thing to press rather than part of the shell's chrome.
    /// </summary>
    /// <remarks>
    /// The dock's own buttons are excluded: a tab and a close are chrome, not a thing to press, and a
    /// rule that is always on cannot be relied on to beat one that turns on with a state. Written so
    /// that it does not match, the question does not arise.
    /// </remarks>
    /// <param name="selector">Where the rule starts.</param>
    private static Selector Pressable(Selector? selector) =>
        selector!
            .OfType<Button>()
            .Not(previous => previous.Class(DockArea.TitleClass))
            .Not(previous => previous.Class(DockArea.CloseClass));

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
    /// a colour changed at runtime reach every control that took it.
    /// </remarks>
    private static Setter Brushed(AvaloniaProperty property, string resource) =>
        new(property, new DynamicResourceExtension(resource));

    /// <summary>A setter whose value is a colour computed here rather than named as a resource.</summary>
    private static Setter NewBrush(AvaloniaProperty property, Color colour) =>
        new(property, new SolidColorBrush(colour));

    /// <summary>
    /// What is written on a surface filled with a colour.
    /// </summary>
    /// <remarks>
    /// Black or white, whichever can be read on it. The design names its own ink for its own primary; a
    /// primary the user chose has no such name, so it is worked out — which is why it is open to the
    /// tests as well as used here.
    /// </remarks>
    /// <param name="accent">The colour anything accented is drawn in.</param>
    /// <returns>The ink to write on it.</returns>
    internal static Color InkOn(Color accent) =>
        Contrasts(Colors.Black, accent) >= Contrasts(Colors.White, accent)
            ? Colors.Black
            : Colors.White;

    /// <summary>
    /// How much one colour stands out from another, as the ratio a reader's legibility is held to.
    /// </summary>
    /// <param name="one">One colour.</param>
    /// <param name="other">The other.</param>
    private static double Contrasts(Color one, Color other)
    {
        var (lighter, darker) = Luminance(one) > Luminance(other) ? (one, other) : (other, one);

        return (Luminance(lighter) + 0.05) / (Luminance(darker) + 0.05);
    }

    /// <summary>A colour's relative luminance, as the contrast ratio is defined over.</summary>
    /// <param name="colour">The colour.</param>
    private static double Luminance(Color colour) =>
        (0.2126 * Channel(colour.R)) + (0.7152 * Channel(colour.G)) + (0.0722 * Channel(colour.B));

    /// <summary>One channel, linearised from the value a display writes.</summary>
    /// <param name="value">The channel.</param>
    private static double Channel(byte value)
    {
        var encoded = value / 255.0;

        return encoded <= 0.03928 ? encoded / 12.92 : Math.Pow((encoded + 0.055) / 1.055, 2.4);
    }

    /// <summary>The fade a surface takes a new colour with.</summary>
    private static Transitions Fading() => Fading(Fade);

    /// <summary>The fade a surface arrives with, over the given time.</summary>
    /// <param name="duration">How long it takes.</param>
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
    /// <param name="colour">The colour.</param>
    private static IBrush Fill(Color colour) => new SolidColorBrush(colour);

    /// <summary>
    /// One colour mixed into another by a fraction of the way there.
    /// </summary>
    /// <remarks>
    /// The design writes its hover as a colour mixed with white rather than as a second colour, because
    /// what a hover is is "this colour, lifted" and that is a property of the colour in hand.
    /// </remarks>
    /// <param name="colour">The colour to start from.</param>
    /// <param name="towards">The colour to walk towards.</param>
    /// <param name="amount">How much of the way there, from nothing to all of it.</param>
    private static Color Mix(Color colour, Color towards, double amount)
    {
        // Written as two steps so that the rounding is of the whole channel rather than of each half.
        var other = 1 - amount;

        return Color.FromRgb(
            (byte)Math.Round((colour.R * other) + (towards.R * amount)),
            (byte)Math.Round((colour.G * other) + (towards.G * amount)),
            (byte)Math.Round((colour.B * other) + (towards.B * amount))
        );
    }

    /// <summary>The same colour, at another alpha.</summary>
    /// <param name="colour">The colour.</param>
    /// <param name="alpha">The alpha.</param>
    private static Color WithAlpha(Color colour, byte alpha) =>
        Color.FromArgb(alpha, colour.R, colour.G, colour.B);
}
