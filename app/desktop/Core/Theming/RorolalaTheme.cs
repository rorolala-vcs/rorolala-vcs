using Avalonia;
using Avalonia.Animation;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Controls.Chrome;
using Avalonia.Controls.Presenters;
using Avalonia.Controls.Primitives;
using Avalonia.Controls.Shapes;
using Avalonia.Controls.Templates;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Styling;
using RorolalaDesktop.CoreDocks;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;
using Path = Avalonia.Controls.Shapes.Path;

namespace RorolalaDesktop.Theming;

/// <summary>
/// The look: three grounds, rounded surfaces, one vivid primary, and soft shadows.
/// </summary>
/// <remarks>
/// It is the program's own and there is only one of it. What a run chooses is the variant it is drawn
/// in and the colours, read before the window is made. Everything else here is stated
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
/// <see cref="DockArea.TitleClass"/> on a dock's own tab — with <see cref="DockArea.SelectedClass"/> on
/// the one being shown and <see cref="DockArea.LeadingClass"/> on the first of them — and
/// <see cref="DockArea.CloseClass"/> on the close it carries, with
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
    /// <param name="style">Which of the three ways a window's own frame is drawn this window is drawn in.</param>
    public RorolalaTheme(Color primary, Color accent, Color? primaryText, WindowStyle style)
    {
        _primary = primary;
        _accent = accent;
        _primaryText = primaryText ?? InkOn(primary);
        _style = style;

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
            [Warn] = Fill(WarnColour),

            ThemeDictionaries =
            {
                [ThemeVariant.Light] = _light,
                [ThemeVariant.Dark] = _dark,
            },
        };

        var palette = new Style(selector => selector.OfType<Window>()) { Resources = _palette };

        Styles = [palette, .. Type(), .. Roles(), .. Content(), .. Parts(), .. Chrome(), .. Cards()];
    }

    /// <summary>
    /// The class a window's own button wears: minimize, maximize and close.
    /// </summary>
    /// <remarks>
    /// A window's buttons are chrome, not a thing to press, so they are drawn by their own rules rather
    /// than by the rule every button in the program shares — and being excluded from that rule is also
    /// what keeps a press on them from being styled as though it were an ordinary action.
    /// </remarks>
    public const string CaptionClass = "caption";

    /// <summary>
    /// The class the button that closes a window wears, beside <see cref="CaptionClass"/>.
    /// </summary>
    /// <remarks>
    /// Closing is the one of the three that takes work away rather than changing how it is shown, so it
    /// is the one drawn as the design draws a thing that removes something: the failure's red, as a wash
    /// under the pointer rather than as a fill, the way the button that closes a dock is drawn.
    /// </remarks>
    public const string CaptionCloseClass = "close";

    /// <summary>
    /// The class a window button's picture wears.
    /// </summary>
    /// <remarks>
    /// Nothing draws a picture through this class: a picture is read out of the icon library while the
    /// window's decorations are built, and what the look gives it is the ink of the button it was put in
    /// rather than a rule of its own. It is here so that a rule for it can be written where the buttons'
    /// own rules are, which is where anyone looking for one would look.
    /// </remarks>
    public const string CaptionGlyphClass = "caption-glyph";

    /// <summary>How wide one of a window's buttons is.</summary>
    /// <remarks>
    /// Wide enough to be hit without aiming and no wider, since three of them share a row with the menu
    /// and every pixel they take is a menu item's. It is stated as a width rather than left to the glyph
    /// so that the menu bar can reserve exactly this much on its own right, which is what keeps a long
    /// menu from being drawn under the buttons.
    /// </remarks>
    private const double CaptionButtonWidth = 44.0;

    /// <summary>How many of them there are, which is what the menu bar reserves room for.</summary>
    private const int CaptionButtonCount = 3;

    /// <summary>
    /// How wide one of the three lights of a macOS frame is.
    /// </summary>
    /// <remarks>
    /// A light is a mark rather than a button: it is as wide as it is tall, it is round, and the pointer is
    /// on it within a few pixels. This is macOS' own size for them, which is what makes the three read as
    /// one row of beads rather than as three buttons that happen to be small.
    /// </remarks>
    private const double MacLight = 12.0;

    /// <summary>How far one light stands from the next.</summary>
    private const double MacGap = 8.0;

    /// <summary>How far the first light stands from the window's own edge.</summary>
    private const double MacEdge = 12.0;

    /// <summary>What the three lights and their room take on the left of the row.</summary>
    private const double MacRoom =
        MacEdge + (MacLight * CaptionButtonCount) + (MacGap * (CaptionButtonCount - 1)) + 8;

    /// <summary>
    /// The class the menu bar carries, for a theme to address it by.
    /// </summary>
    /// <remarks>
    /// The shell marks the surfaces it owns so that a theme can style them without knowing what is in
    /// them. The name is fixed, since a plugin's theme writes it as a literal.
    /// </remarks>
    public const string MenuBarClass = "menu-bar";

    /// <summary>What the menu bar leaves free for the window's own buttons.</summary>
    /// <remarks>
    /// A menu bar that reached the buttons would draw its last menu under them: the row is shared by laying
    /// it out, and this is the part of it the menu may not have. Which side they are on is the style's — the
    /// three marks are on the right and the three lights on the left — and a window under a tiling window
    /// manager has none of them, so its menu bar has the whole of the row.
    /// </remarks>
    private Thickness MenuBarPadding =>
        _style switch
        {
            WindowStyle.Mac => new Thickness(MacRoom, 0, 8, 0),
            WindowStyle.TillingWm => new Thickness(8, 0, 8, 0),
            _ => new Thickness(8, 0, (CaptionButtonWidth * CaptionButtonCount) + 8, 0),
        };

    /// <summary>
    /// The class the row a window's menu and its own buttons share wears.
    /// </summary>
    /// <remarks>
    /// Marked rather than matched by the name the markup gives it, for the reason the menu bar is: the
    /// name a theme addresses a surface by is fixed, and a plugin's theme writes it as a
    /// literal.
    /// </remarks>
    public const string TitleBarClass = "title-bar";

    /// <summary>
    /// The class the name of the one dock a window holds wears, in that window's own row.
    /// </summary>
    /// <remarks>
    /// Marked for the same reason the row and the menu bar are, and read by more than the look: a press that
    /// lands on it begins a drag of the dock rather than a move of the window, and what tells the two apart
    /// is this class (<see cref="MainWindow.MoveOrMaximize"/>).
    /// </remarks>
    public const string WindowTitleClass = "window-title";

    /// <summary>The size every word is set at.</summary>
    private const double BodySize = 14.0;

    /// <summary>The height of a row of a list, a tree, or a field.</summary>
    private const double RowHeight = 30.0;

    /// <summary>
    /// The height a step of a tree is held at, and the room left under the line it draws.
    /// </summary>
    /// <remarks>
    /// A step draws a band of twenty-one pixels — an icon and a line of text with the row's own padding — and
    /// these two put three more under it: the row is twenty-four, and the three are left below the presenter
    /// rather than being the difference between the row and what it draws. That difference is what the base
    /// theme leaves, and it is dead: a row of thirty draws twenty-one and keeps nine, all of them under the
    /// band, so two steps of one directory stand nine pixels apart and a step and the first of its children
    /// stand nine too — except that the nine under a parent is *inside* the parent's own row, which puts its
    /// children three pixels closer to it than they are to each other. Stating the room below the line makes
    /// every pair of consecutive steps the same distance apart, whichever of them is the other's parent.
    /// </remarks>
    private const double TreeRowHeight = 24.0;

    /// <inheritdoc cref="TreeRowHeight" />
    private const double TreeRowRoom = 3.0;

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

    /// <summary>
    /// The sunken ground at no alpha: what a fill that hides until the pointer is on it fades in from.
    /// </summary>
    /// <remarks>
    /// A fade is between two colours, and "no colour" is <em>white</em> at no alpha: fading from it into a
    /// dark fill passes through a light grey — at the middle of the fade, a grey lighter than either end —
    /// which reads as a flash of light on the way in. Avalonia interpolates a colour' channels one by one and
    /// leaves the alpha to its own, so what it fades is the <em>colour</em> whether or not either end can be
    /// seen. A clear fill is therefore the colour it fades to at no alpha, and the only thing that changes
    /// over the fade is the alpha. See [`Variant`], where the two are made.
    /// </remarks>
    private const string SunkenClear = "rorolala.bg.sunken.clear";

    /// <inheritdoc cref="SunkenClear" />
    private const string BorderClear = "rorolala.border.clear";
    private const string Foreground = "rorolala.fg";
    private const string Muted = "rorolala.fg.muted";
    private const string Faint = "rorolala.fg.faint";
    private const string BorderLine = "rorolala.border";
    private const string Strong = "rorolala.border.strong";

    /// <summary>What a tab the pointer is on, and that is not the one being read, is filled with.</summary>
    private const string TabHover = "rorolala.tab.hover";

    /// <summary>The same fill at no alpha, which is what a tab that is not pointed at is filled with.</summary>
    /// <inheritdoc cref="SunkenClear" />
    private const string TabClear = "rorolala.tab.clear";
    private const string Primary = "rorolala.primary";
    private const string PrimaryBright = "rorolala.primary.bright";
    private const string PrimaryText = "rorolala.primary.text";
    private const string Accent = "rorolala.accent";
    private const string Selection = "rorolala.selection";

    /// <summary>
    /// The line between two docks, down a column splitter and across a row one.
    /// </summary>
    /// <remarks>
    /// Two names rather than one because the line is a gradient with hard stops and its axis is the
    /// splitter's: a brush drawn down a column splitter and the same brush drawn across a row one are two
    /// brushes. They are the border's own colour, so a divider and the edge of the surface it divides
    /// read as one drawing rather than two greys.
    /// </remarks>
    private const string SplitterDown = "rorolala.splitter";
    private const string SplitterAcross = "rorolala.splitter.across";

    /// <inheritdoc cref="SplitterDown" />
    private const string SplitterDownHover = "rorolala.splitter.hover";
    private const string SplitterAcrossHover = "rorolala.splitter.across.hover";

    private const string Warn = "rorolala.warn";

    /// <summary>
    /// The one amber, which is what a warning is drawn in on either ground.
    /// </summary>
    /// <remarks>
    /// Fixed rather than chosen: it is picked to carry on both the light ground and the dark one, and what a
    /// warning is coloured is not a preference. A named constant because its wash is the same colour laid thin.
    /// </remarks>
    private static readonly Color WarnColour = Color.FromRgb(0xD4, 0xA0, 0x17);

    /// <summary>
    /// The three lights of a macOS frame, in the order they are drawn from the window's edge.
    /// </summary>
    /// <remarks>
    /// Fixed rather than chosen, like the amber above, and for the reason that makes them worth having: a
    /// window asking to be drawn the way macOS draws one is asking for these three colours, and a red, a
    /// yellow and a green of the program's own choosing would be three beads that mean nothing.
    /// </remarks>
    private static readonly Color MacClose = Color.FromRgb(0xFF, 0x5F, 0x57);
    private static readonly Color MacMinimize = Color.FromRgb(0xFE, 0xBC, 0x2E);
    private static readonly Color MacMaximize = Color.FromRgb(0x28, 0xC8, 0x40);

    /// <summary>
    /// What the mark on a light is drawn in.
    /// </summary>
    /// <remarks>
    /// A dark ink laid thin, which is what macOS draws on all three of its lights: the fill is the colour
    /// that says which of them it is, and a mark in the window's own ink would be white on two of the three
    /// on the dark ground — where a mark that cannot be read is a bead with nothing to say.
    /// </remarks>
    private static readonly Color MacInk = Color.FromArgb(0x66, 0x00, 0x00, 0x00);
    private const string Shadow = "rorolala.shadow";
    private const string Ring = "rorolala.ring";

    /// <summary>The colour a failure is drawn in.</summary>
    internal const string DelInk = "rorolala.del";

    /// <summary>The wash a failure is drawn on.</summary>
    internal const string DelBackground = "rorolala.del.bg";

    /// <summary>The wash a plugin card takes when the pointer is on it, or a drag is over it.</summary>
    internal const string AccentWash = "rorolala.accent.wash";

    /// <summary>The wash a plugin card takes while it is the one being dragged: the accent, lifted.</summary>
    internal const string AccentBrightWash = "rorolala.accent.bright.wash";

    /// <summary>The wash a plugin card takes when what it depends on leaves it unloadable.</summary>
    internal const string DelWash = "rorolala.del.wash";

    /// <summary>
    /// The class the words for a fault wear, which the tip on a card's mark shows.
    /// </summary>
    /// <remarks>
    /// A class rather than a colour written where the text is made, so that the red is the look's and
    /// follows a variant or a recolour like every other colour the program paints.
    /// </remarks>
    private const string FaultText = "plugin-fault";

    /// <summary>
    /// The class the one action a surface exists for wears.
    /// </summary>
    /// <remarks>
    /// The design does not single out one action, but a dialog has one and the shell has to be able to
    /// say which: a class rather than a property the toolkit owns, so which button is the action is the
    /// shell's to say and the look's to draw.
    /// </remarks>
    private const string PrimaryAction = "primary";

    /// <summary>
    /// The class a tool wears while it is on.
    /// </summary>
    /// <remarks>
    /// A tool's state is the program's rather than the toolkit's — a toolbar's switch says whether the program
    /// is showing what the rules hide, which is no control's own fact — so it is a class the program puts on
    /// rather than the toolkit's checked state: the look draws it, and what it means stays where the meaning is
    /// kept.
    /// </remarks>
    private const string OnClass = "on";

    /// <summary>
    /// The class a menu item wears when it is one of the forced actions.
    /// </summary>
    /// <remarks>
    /// What an action does past the checks it would otherwise make is worth saying twice — in its words and in
    /// its colour — because it is the one kind of item a user should reach on purpose rather than by pointing
    /// at whatever is under the pointer.
    /// </remarks>
    private const string Forced = "force";

    /// <summary>The colour what is chosen is drawn in.</summary>
    private readonly Color _primary;

    /// <summary>The primary lifted towards white, which the design hovers a filled action with.</summary>
    private readonly Color _primaryBright;

    /// <summary>The colour the drag marks are drawn in.</summary>
    private readonly Color _accent;

    /// <summary>What is written on a surface filled with the primary.</summary>
    private readonly Color _primaryText;

    /// <summary>Which of the three ways a window's own frame is drawn this look draws.</summary>
    private readonly WindowStyle _style;

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
            [SunkenClear] = Fill(WithAlpha(grounds.Sunken, 0)),
            [BorderClear] = Fill(WithAlpha(grounds.Border, 0)),
            [Foreground] = Fill(grounds.Fg),
            [Muted] = Fill(grounds.Muted),
            [Faint] = Fill(grounds.Faint),
            [BorderLine] = Fill(grounds.Border),
            [Strong] = Fill(grounds.Strong),

            // What a tab that is not the one being read is filled with when the pointer is on it, and the same
            // colour at no alpha for the rest of the time. The two are one pair rather than a fill and
            // "nothing", because a tab at rest is not filled with nothing — it is filled with the strip's own
            // ground, which is what a clear fill has to be if the fade is to change the alpha alone
            // (see `SunkenClear`).
            [TabHover] = Fill(TabFill(grounds)),
            [TabClear] = Fill(WithAlpha(TabFill(grounds), 0)),

            [Primary] = Fill(_primary),
            [PrimaryBright] = Fill(_primaryBright),
            [PrimaryText] = Fill(_primaryText),
            [Accent] = Fill(_accent),
            [AccentWash] = Fill(WithAlpha(_accent, 0x2E)),
            [AccentBrightWash] = Fill(WithAlpha(Mix(_accent, Colors.White, 0.18), 0x59)),

            // The line between two docks, and the same line under the pointer. Both are made here rather
            // than where the splitter is styled, because a hairline is a gradient with hard stops and a
            // gradient cannot be told to take a colour from a resource: made in the look they are entries
            // of it, so a variant switched or a colour recoloured draws the line it names now.
            [SplitterDown] = Hairline(vertical: true, grounds.Border),
            [SplitterAcross] = Hairline(vertical: false, grounds.Border),
            [SplitterDownHover] = Hairline(vertical: true, _accent),
            [SplitterAcrossHover] = Hairline(vertical: false, _accent),

            [Selection] = Fill(selection),
            [Shadow] = grounds.Raised,
            [Ring] = _ring,
            ["rorolala.add"] = Fill(grounds.Add),
            ["rorolala.add.bg"] = Fill(grounds.AddBg),
            // The wash a warning stands on, which is its own amber laid thin rather than a second colour: what
            // a warning is coloured is not a preference, and neither is the ground it is read against.
            ["rorolala.warn.bg"] = Fill(WithAlpha(WarnColour, 30)),
            [DelInk] = Fill(grounds.Del),
            [DelBackground] = Fill(grounds.DelBg),
            [DelWash] = Fill(WithAlpha(grounds.Del, 0x2E)),
        };

    /// <summary>
    /// What a tab the pointer is on is filled with, which is also what a tab at rest is filled with at no
    /// alpha.
    /// </summary>
    /// <remarks>
    /// Halfway back to the ground the lit tab is drawn on, because a tab that is already the sunken ground
    /// has no sunken state left to take.
    /// </remarks>
    /// <param name="grounds">The variant's own neutrals.</param>
    /// <returns>The colour.</returns>
    private static Color TabFill(Grounds grounds) => Mix(grounds.Sunken, grounds.Elevated, 0.5);

    /// <summary>
    /// One pixel of line through the middle of a splitter, in the colour it is given.
    /// </summary>
    /// <remarks>
    /// A gradient with hard stops rather than a colour, because there is nothing to put a line in: a
    /// splitter is one surface with no template part a theme may address, so painting the grip clear and
    /// the middle of it not is the only way to draw one pixel of line inside the grip's own width. The
    /// stops are placed from that width rather than from the pixel, so the line is a line whatever the
    /// grip is widened to.
    /// <para>
    /// The colour is a parameter rather than the accent the two hover brushes happen to use, because the
    /// line drawn at rest is the border's own and that one differs by variant — which is also why these
    /// are made here, where the variant's neutrals are, rather than where a splitter is styled.
    /// </para>
    /// </remarks>
    /// <param name="vertical">Whether the line runs down the splitter rather than across it.</param>
    /// <param name="colour">What the line is drawn in.</param>
    private static IBrush Hairline(bool vertical, Color colour)
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
                new GradientStop(colour, 0.5 - half),
                new GradientStop(colour, 0.5 + half),
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
                Brushed(TemplatedControl.BackgroundProperty, SunkenClear),
                Brushed(TemplatedControl.BorderBrushProperty, BorderClear),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(0)),
                new Setter(ContentControl.HorizontalContentAlignmentProperty, HorizontalAlignment.Center),
                new Setter(ContentControl.VerticalContentAlignmentProperty, VerticalAlignment.Center),
                new Setter(Layoutable.WidthProperty, 28.0),
                new Setter(Layoutable.HeightProperty, 28.0)
            ),
            On(
                selector => Pressable(selector).Class("ghost"),
                Brushed(TemplatedControl.BackgroundProperty, SunkenClear),
                Brushed(TemplatedControl.BorderBrushProperty, BorderClear)
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

            // A tool that is on is filled with the primary, the way the one action a surface exists for is:
            // what a toolbar's tool shows is a state, and a state takes the colour the program says a thing
            // chosen is. Its own hover is named as well, because a fill and a hover are two answers to one
            // property and the later rule is the one that is read: without it, a tool that is on would go back
            // to the sunken hover whenever the pointer found it.
            On(
                selector => Pressable(selector).Class("tool").Class(OnClass),
                Brushed(TemplatedControl.BackgroundProperty, Primary),
                Brushed(TemplatedControl.ForegroundProperty, PrimaryText)
            ),
            On(
                selector => Pressable(selector).Class("tool").Class(OnClass).Class(":pointerover"),
                Brushed(TemplatedControl.BackgroundProperty, PrimaryBright)
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
            // A list is a pane rather than a card, for the reason a tree is: what it is drawn in is the
            // ground of whatever holds it, and a card of its own inside a dock — with an edge and a corner
            // of its own, boxed in by the dock's own edges — reads as a surface laid on the dock rather than
            // as the part of the window the entries are read in. Nothing is drawn around it here, which is
            // said because the base theme does draw something: its border would be that card.
            On(
                selector => selector.OfType<ListBox>(),
                new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(0))
            ),
            On(
                selector => selector.OfType<ListBoxItem>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(8, 4)),
                new Setter(Layoutable.MinHeightProperty, RowHeight),
                new Setter(TemplatedControl.CornerRadiusProperty, Small)
            ),
            // A tree is a pane rather than a card: the one edge it is read by is the ground the dock that
            // holds it draws, and no corner is rounded off. It used to be drawn with a radius, which read as a
            // surface laid on the dock rather than as the part of the window the steps are read in.
            //
            // Nothing is drawn around it here, which is stated only because the base theme draws something: its
            // border would draw that one edge a second time, beside the ground's own and twice as thick, and
            // would box in the three sides that are the window's edge and belong to no surface of this program.
            On(
                selector => selector.OfType<TreeView>(),
                new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(0))
            ),
            On(
                selector => selector.OfType<TreeViewItem>(),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(2, 1)),
                // A step of a tree is a row of its own height rather than a listing's, and the room between two
                // steps is stated below rather than being the difference between that height and what a step
                // draws. See the two constants named here for why each is what it is.
                new Setter(Layoutable.MinHeightProperty, TreeRowHeight)
            ),
            // The room under a step, which is also the room between a step and whatever it holds: the presenter
            // is what the row's own line is drawn in, and the steps under it are laid out directly below it, so
            // the room left here is what the first of them stands off by. Left to the difference between the
            // row's height and the band it draws, that room lands under the row's cell instead of under the
            // line — which is where nothing is ever drawn — and a step reads as being further from the line of
            // its parent than from the step beside it.
            On(
                selector => selector.OfType<TreeViewItem>().Template().Name("PART_HeaderPresenter"),
                new Setter(Layoutable.MarginProperty, new Thickness(0, 0, 0, TreeRowRoom))
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
            // A forced action is drawn as what it is: the red the program draws a failure in, because what it
            // does is past the checks every other action makes. Its picture is the mark of an item's picture
            // read from the item's own ink, so it is drawn in the same red without being told.
            On(
                selector => selector.OfType<MenuItem>().Class(Forced),
                Brushed(TemplatedControl.ForegroundProperty, DelInk)
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

            // A tab and the cross it carries are buttons this look draws itself, so the rule above leaves
            // them out — and what it leaves out is exactly what has to be said for them, since a hover there
            // would otherwise firm the part's border with the base theme's brush, at once, while the fill
            // behind it fades: the two arriving at different times is what a flicker is. Both are said again
            // here, against the part, because a button's template is what paints its own background, border
            // and the fill a held one is given.
            //
            // What is deliberately *not* said for them is the part's own transition. The look gives one to
            // every other button; here it is left out, because a button that is already fading its own
            // background hands the part a new brush on each frame of that fade — so a transition on the part
            // has nothing to animate but a value that is itself moving, is begun again by every frame of the
            // first, and arrives nowhere. A fade that never arrives is a highlight that flickers.
            On(
                selector =>
                    selector
                        .OfType<Button>()
                        .Class(DockArea.TitleClass)
                        .Class(":pointerover")
                        .Template()
                        .Name("PART_ContentPresenter"),
                Brushed(ContentPresenter.BorderBrushProperty, BorderLine)
            ),
            On(
                selector =>
                    selector.OfType<Button>().Class(DockArea.TitleClass).Class(":pressed").Template().Name("PART_ContentPresenter"),
                Brushed(ContentPresenter.BackgroundProperty, Strong)
            ),
            On(
                selector =>
                    selector.OfType<Button>().Class(DockArea.CloseClass).Class(":pressed").Template().Name("PART_ContentPresenter"),
                new Setter(ContentPresenter.BackgroundProperty, Brushes.Transparent)
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
    /// The surfaces the shell owns: the row the menu and the window's buttons share, the dock strips, and
    /// the grabs between regions.
    /// </summary>
    /// <remarks>
    /// Chrome is the elevated ground with one border under it, so the window reads as a stack of cards:
    /// the row at the top, each strip, and the content between them. A dock's tabs are a segmented
    /// control — one bordered rounded container, the tab being shown filled with the primary — which is
    /// the design's own answer to a tab strip and the reason there is no underline anywhere.
    /// </remarks>
    private Style[] Chrome() =>
        [
            // The row a window's menu and its own buttons share: the elevated ground with the one line along
            // its foot that separates it from the dock area, which stands on the window's own ground. The menu
            // bar inside the row is transparent on purpose: a bar painted with its own fill would take every
            // pixel of itself out of the toolkit's hands, and the press that moves a window would then work
            // everywhere in the row except the empty space of the bar, which is where it is aimed. What is set
            // here is what a press cannot do without — the room a menu needs and the room on the right the
            // buttons are in.
            On(
                selector => selector.OfType<Border>().Class(TitleBarClass),
                Brushed(Border.BackgroundProperty, Elevated),
                new Setter(Border.BorderThicknessProperty, new Thickness(0, 0, 0, 1)),
                Brushed(Border.BorderBrushProperty, BorderLine),
                new Setter(Layoutable.MinHeightProperty, MenuBarHeight)
            ),
            On(
                selector => selector.OfType<Menu>().Class(MenuBarClass),
                new Setter(TemplatedControl.PaddingProperty, MenuBarPadding)
            ),

            // The name of the one dock a window holds, which is drawn in the same row and is what a drag of
            // that dock is taken from. It is drawn as a surface that is there rather than as a word on the
            // row's own ground, because what it is for is being taken hold of; what it holds still is the
            // row's ink, since a name is a name and not an action.
            On(
                selector => selector.OfType<TextBlock>().Class(WindowTitleClass),
                Brushed(TextBlock.BackgroundProperty, SunkenClear),
                Brushed(TextBlock.ForegroundProperty, Muted),
                new Setter(TextBlock.PaddingProperty, new Thickness(8, 3)),
                new Setter(Layoutable.VerticalAlignmentProperty, VerticalAlignment.Center)
            ),
            On(
                selector => selector.OfType<TextBlock>().Class(WindowTitleClass).Class(":pointerover"),
                Brushed(TextBlock.BackgroundProperty, Sunken)
            ),

            // A window's own buttons, which are the one thing about this row the three styles disagree on.
            .. Captions(),

            // The strip a region's headers sit in: the sunken ground, so that the tab being read — which is
            // drawn on the elevated one — is the one thing in the row that lifts out of it.
            //
            // The line along its foot is what separates the row from the dock below, and it is the row's own
            // rather than each tab's: one line under everything is one line, where a foot drawn by each tab
            // would be a line broken wherever two tabs meet. The tab being read covers it — see the rule for
            // one that is chosen — so the line stops where that tab starts and the two surfaces join there.
            On(
                selector => selector.OfType<Border>().Class(DockArea.HeadersClass),
                Brushed(Border.BackgroundProperty, Sunken),
                Brushed(Border.BorderBrushProperty, BorderLine),
                new Setter(Border.BorderThicknessProperty, new Thickness(0, 0, 0, 1))
            ),

            // A tab is a piece of the strip rather than a control laid in it: no margin, as tall as the
            // strip, and square, because the tabs *are* the strip. What a tab is, is its fill — the one
            // being read is drawn on the elevated ground and lifts out of the sunken one the strip is, which
            // is what makes a row of them read as one strip with the current one lit. A tab that is not the
            // one being read draws nothing of its own, so that the strip it is part of shows through and the
            // divider beside it is what marks where it ends.
            //
            // The one edge drawn is the one between two tabs: each tab carries it on its left, so a shared
            // edge is drawn once rather than twice. What ends the row carries one on its right as well,
            // because that is the edge between the tabs and the room the strip keeps beside them — and the
            // tab that is both ends of a strip of one is closed on its right and open on its left, where the
            // strip's own edge already is. Nothing is drawn on the top or the bottom, which are the strip's.
            On(
                selector => selector.OfType<Button>().Class(DockArea.TitleClass),
                Brushed(TemplatedControl.BackgroundProperty, TabClear),
                new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(1, 0, 0, 0)),
                Brushed(TemplatedControl.BorderBrushProperty, BorderLine),
                new Setter(TemplatedControl.CornerRadiusProperty, new CornerRadius(0)),
                Brushed(TemplatedControl.ForegroundProperty, Muted),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector =>
                    selector.OfType<Button>().Class(DockArea.TitleClass).Class(DockArea.LeadingClass),
                new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(0))
            ),
            On(
                selector =>
                    selector.OfType<Button>().Class(DockArea.TitleClass).Class(DockArea.TrailingClass),
                new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(1, 0, 1, 0))
            ),
            On(
                selector =>
                    selector
                        .OfType<Button>()
                        .Class(DockArea.TitleClass)
                        .Class(DockArea.LeadingClass)
                        .Class(DockArea.TrailingClass),
                new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(0, 0, 1, 0))
            ),
            On(
                selector => selector.OfType<Button>().Class(DockArea.TitleClass).Class(":pointerover"),
                Brushed(TemplatedControl.BackgroundProperty, TabHover),
                Brushed(TemplatedControl.ForegroundProperty, Foreground)
            ),
            On(
                selector => selector.OfType<Button>().Class(DockArea.TitleClass).Class(DockArea.SelectedClass),
                Brushed(TemplatedControl.BackgroundProperty, Elevated),
                Brushed(TemplatedControl.ForegroundProperty, Foreground),
                new Setter(TemplatedControl.FontWeightProperty, FontWeight.SemiBold),
                // Down by the line the strip draws along its foot, so that this tab is drawn over it: a tab
                // that is read is joined to what it is a tab of, and the line is the row's rather than the
                // tab's, so covering it is what leaves the join (see the strip's own rule). One pixel of
                // margin is the whole of it: the row is one pixel shorter than the strip, and this is the
                // pixel the line is drawn in.
                new Setter(Layoutable.MarginProperty, new Thickness(0, 0, 0, -1))
            ),
            // A held tab is the one thing darker than the sunken ground it already sits on, whether or not
            // it is the one being read: a press that changed nothing would read as a press that went nowhere.
            On(
                selector => selector.OfType<Button>().Class(DockArea.TitleClass).Class(":pressed"),
                Brushed(TemplatedControl.BackgroundProperty, Strong)
            ),
            On(
                selector =>
                    selector
                        .OfType<Button>()
                        .Class(DockArea.TitleClass)
                        .Class(DockArea.SelectedClass)
                        .Class(":pressed"),
                Brushed(TemplatedControl.BackgroundProperty, Elevated)
            ),

            // The close a tab carries: nothing of its own — no ground behind it, no border, no ink, so that it
            // is written in the tab's and reads as the end of the title rather than as a button that found its
            // way into one. Its room is the tab's to give (see `DockArea.Make`), which is why nothing about how
            // wide or how tall it is is said here: it is a strip of the tab's own right side, and a strip is
            // what the tab is shaped around rather than a box dropped into it.
            //
            // It is drawn while the pointer is on the tab rather than always, because a row of crosses is a row
            // of things to avoid pressing, and one that arrives under the pointer is one the eye finds when it
            // is wanted. Being on the cross *is* being on the tab — the toolkit marks every ancestor of what it
            // is over, and not only what it hit — so nothing has to hold it drawn under a pointer that has come
            // to rest on it.
            On(
                selector => selector.OfType<Button>().Class(DockArea.CloseClass),
                new Setter(TemplatedControl.BackgroundProperty, Brushes.Transparent),
                new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(0)),
                new Setter(TemplatedControl.CornerRadiusProperty, new CornerRadius(0)),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(0)),
                new Setter(Visual.OpacityProperty, 0.0),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector =>
                    selector
                        .OfType<Button>()
                        .Class(DockArea.TitleClass)
                        .Class(":pointerover")
                        .Descendant()
                        .OfType<Button>()
                        .Class(DockArea.CloseClass),
                new Setter(Visual.OpacityProperty, 1.0)
            ),

            // A splitter is the line between two docks: one row of the drawing, drawn at rest rather than
            // only under the pointer, because the space between two docks is a divider and a gap that showed
            // only the ground read as two docks that did not meet. What it is drawn in is the border's own
            // colour, so every divider is the same one wherever two docks meet.
            //
            // The grab is wider than the line — see `DockArea.SplitterSize` — and the line is drawn through
            // the middle of it by a gradient rather than by a border, because a splitter is one surface with
            // no template part a theme may address: painting the grip clear and the middle of it not is the
            // only way to put one pixel of line inside five pixels of grip.
            On(
                selector =>
                    selector
                        .OfType<GridSplitter>()
                        .Class(DockArea.SplitterClass)
                        .Class(DockArea.SplitterColumnsClass),
                Brushed(TemplatedControl.BackgroundProperty, SplitterDown),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector =>
                    selector
                        .OfType<GridSplitter>()
                        .Class(DockArea.SplitterClass)
                        .Class(DockArea.SplitterRowsClass),
                Brushed(TemplatedControl.BackgroundProperty, SplitterAcross),
                new Setter(TemplatedControl.TransitionsProperty, Fading())
            ),
            On(
                selector =>
                    selector
                        .OfType<GridSplitter>()
                        .Class(DockArea.SplitterClass)
                        .Class(DockArea.SplitterColumnsClass)
                        .Class(":pointerover"),
                Brushed(TemplatedControl.BackgroundProperty, SplitterDownHover)
            ),
            On(
                selector =>
                    selector
                        .OfType<GridSplitter>()
                        .Class(DockArea.SplitterClass)
                        .Class(DockArea.SplitterRowsClass)
                        .Class(":pointerover"),
                Brushed(TemplatedControl.BackgroundProperty, SplitterAcrossHover)
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
    /// How a window's own buttons are drawn, which is where the three styles part company.
    /// </summary>
    /// <remarks>
    /// One button is a mark on the right on Windows, a light on the left on macOS, and nothing at all under
    /// a tiling window manager — which keeps its window by a frame of its own, so buttons drawn here would
    /// be a second frame inside the first.
    /// <para>
    /// The marks are drawn the way the design draws a tool: nothing of their own until the pointer is on
    /// them, the sunken ground while it is, and the failure's red wash for the one that closes. A light is
    /// filled instead, because the colour is what says which of the three it is, and what it does is the
    /// mark macOS draws on it while the pointer is over it.
    /// </para>
    /// </remarks>
    private Style[] Captions() =>
        _style switch
        {
            WindowStyle.Mac =>
            [
                On(
                    selector => selector.OfType<Button>().Class(CaptionClass),
                    new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(0)),
                    new Setter(TemplatedControl.CornerRadiusProperty, Pill),
                    new Setter(TemplatedControl.PaddingProperty, new Thickness(0)),
                    new Setter(Layoutable.WidthProperty, MacLight),
                    new Setter(Layoutable.HeightProperty, MacLight),
                    new Setter(TemplatedControl.TransitionsProperty, Fading())
                ),
                On(
                    selector => selector.OfType<Border>().Class(CaptionGlyphClass),
                    new Setter(Visual.OpacityProperty, 0.0)
                ),
                On(
                    selector =>
                        selector
                            .OfType<Button>()
                            .Class(CaptionClass)
                            .Class(":pointerover")
                            .Descendant()
                            .OfType<Border>()
                            .Class(CaptionGlyphClass),
                    new Setter(Visual.OpacityProperty, 1.0)
                ),
            ],
            WindowStyle.TillingWm => [],
            _ =>
            [
                On(
                    selector => selector.OfType<Button>().Class(CaptionClass),
                    Brushed(TemplatedControl.BackgroundProperty, SunkenClear),
                    new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(0)),
                    new Setter(TemplatedControl.CornerRadiusProperty, new CornerRadius(0)),
                    new Setter(TemplatedControl.PaddingProperty, new Thickness(0)),
                    new Setter(Layoutable.WidthProperty, CaptionButtonWidth),
                    Brushed(TemplatedControl.ForegroundProperty, Muted),
                    new Setter(TemplatedControl.TransitionsProperty, Fading())
                ),
                On(
                    selector => selector.OfType<Button>().Class(CaptionClass).Class(":pointerover"),
                    Brushed(TemplatedControl.BackgroundProperty, Sunken),
                    Brushed(TemplatedControl.ForegroundProperty, Foreground)
                ),
                On(
                    selector =>
                        selector.OfType<Button>().Class(CaptionClass).Class(":pressed"),
                    Brushed(TemplatedControl.BackgroundProperty, Strong)
                ),
                On(
                    selector =>
                        selector.OfType<Button>().Class(CaptionClass).Class(CaptionCloseClass).Class(":pointerover"),
                    Brushed(TemplatedControl.BackgroundProperty, DelBackground),
                    Brushed(TemplatedControl.ForegroundProperty, DelInk)
                ),
            ],
        };

    /// <summary>
    /// The cards the plugin manager arranges: one raised surface per plugin, and its wash.
    /// </summary>
    /// <remarks>
    /// A card is the same raised surface as every other control, with one addition: a child border
    /// filled with a translucent colour, which is what says "the pointer is on this" and "this is
    /// the one being dragged" and "this cannot load" without the card becoming a different colour
    /// from the ones beside it. The wash is a child rather than the card's own background because a
    /// background cannot be laid over a ground — it replaces it — and a tint that replaced the
    /// surface would read as a different control rather than as the same one under the pointer.
    /// <para>
    /// The card's own error state is stated last, so that it beats the pointer and the drag: an
    /// error a hover could hide would be an error the user loses the moment they reach for it.
    /// </para>
    /// </remarks>
    private static Style[] Cards() =>
        [
            On(
                selector => selector.OfType<Border>().Class(PluginCard.CardClass),
                Brushed(Border.BackgroundProperty, Elevated),
                new Setter(Border.BorderThicknessProperty, Edge),
                Brushed(Border.BorderBrushProperty, BorderLine),
                new Setter(Border.CornerRadiusProperty, Radius),
                new Setter(Border.TransitionsProperty, Fading())
            ),
            On(
                selector => selector.OfType<Border>().Class(PluginCard.CardClass).Class(":pointerover"),
                Brushed(Border.BorderBrushProperty, Accent)
            ),
            On(
                selector =>
                    selector
                        .OfType<Border>()
                        .Class(PluginCard.CardClass)
                        .Class(PluginCard.DraggingClass),
                Brushed(Border.BorderBrushProperty, Accent)
            ),
            On(
                selector =>
                    selector.OfType<Border>().Class(PluginCard.CardClass).Class(PluginCard.ErrorClass),
                Brushed(Border.BorderBrushProperty, DelInk)
            ),

            // The mark a faulted card carries beside its switch: the card's red, at the size of a
            // word rather than of a line. It is a button only so that it can carry the words for the
            // fault as a tip; pressing it does nothing. Its red is stated again on the presenter,
            // because the base theme's hover and press rules reach that part rather than the button
            // and would otherwise grey the mark the moment the pointer came near it.
            On(
                selector => selector.OfType<Button>().Class(PluginCard.ErrorBadgeClass),
                Brushed(TemplatedControl.BackgroundProperty, DelBackground),
                Brushed(TemplatedControl.BorderBrushProperty, DelInk),
                Brushed(TemplatedControl.ForegroundProperty, DelInk),
                new Setter(TemplatedControl.BorderThicknessProperty, Edge),
                new Setter(TemplatedControl.CornerRadiusProperty, Small),
                new Setter(TemplatedControl.PaddingProperty, new Thickness(7, 2)),
                new Setter(TemplatedControl.FontSizeProperty, 11.0),
                new Setter(TemplatedControl.FontWeightProperty, FontWeight.Bold)
            ),
            On(
                selector =>
                    selector
                        .OfType<Button>()
                        .Class(PluginCard.ErrorBadgeClass)
                        .Template()
                        .Name("PART_ContentPresenter"),
                Brushed(ContentPresenter.BackgroundProperty, DelBackground),
                Brushed(ContentPresenter.BorderBrushProperty, DelInk),
                Brushed(ContentPresenter.ForegroundProperty, DelInk)
            ),

            // The wash: its shape, and the resting state it starts in. Which of the three colours it
            // takes is decided while the program runs, because it turns on a hover, a drag, and a
            // fault the look cannot see; the resources it chooses between are named above, so a
            // recolour reaches a wash that is up as readily as one that is not.
            On(
                selector => selector.OfType<Border>().Class(PluginCard.WashClass),
                Brushed(Border.BackgroundProperty, AccentWash),
                new Setter(Border.CornerRadiusProperty, new CornerRadius(7)),
                new Setter(Visual.OpacityProperty, 0.0),
                new Setter(Visual.TransitionsProperty, Fading())
            ),

            // The words for a fault, which a hover on a card's mark shows, in the same red the card's
            // edge takes.
            On(
                selector => selector.OfType<TextBlock>().Class(FaultText),
                Brushed(TextBlock.ForegroundProperty, DelInk)
            ),
        ];

    /// <summary>
    /// The selector for a button that is a thing to press rather than part of the shell's chrome.
    /// </summary>
    /// <remarks>
    /// The shell's own buttons are excluded: a dock's tab, a dock's close, and a window's minimize,
    /// maximize and close are chrome, not a thing to press, and a rule that is always on cannot be relied
    /// on to beat one that turns on with a state. Written so that it does not match, the question does
    /// not arise.
    /// </remarks>
    /// <param name="selector">Where the rule starts.</param>
    private static Selector Pressable(Selector? selector) =>
        selector!
            .OfType<Button>()
            .Not(previous => previous.Class(DockArea.TitleClass))
            .Not(previous => previous.Class(DockArea.CloseClass))
            .Not(previous => previous.Class(CaptionClass));

    /// <summary>
    /// The name of the one dock a window of its own holds, drawn in that window's own row.
    /// </summary>
    /// <remarks>
    /// A window of one dock draws no strip of tabs, so the name its tab would have carried has nowhere to be
    /// but the row — which is what the desktop's title bar would have said, and the one part of the window
    /// that is not the dock itself. It is the handle the dock is dragged back to the main window by
    /// (<see cref="MainWindow"/>), which is why it is a control of its own rather than a word laid on the
    /// row.
    /// <para>
    /// It stands where the frame's own buttons are not, and clear of the room they take: the row is shared by
    /// laying it out, and a name drawn under a button would be a name that cannot be pressed. Which side that
    /// is is the one thing the three styles disagree about, so it is read here rather than said by the
    /// window.
    /// </para>
    /// </remarks>
    /// <param name="style">Which of the three ways a window's frame is drawn this window is drawn in.</param>
    /// <param name="text">What the dock is called.</param>
    /// <returns>The name, for the window to put in its row and to take a drag from.</returns>
    public static TextBlock Caption(WindowStyle style, string text)
    {
        var (side, margin) = style switch
        {
            // The three lights are at the window's own left corner, so the name starts after them.
            WindowStyle.Mac => (HorizontalAlignment.Left, new Thickness(MacRoom, 0, 8, 0)),

            // A tiling window manager draws no buttons of its own, so the row's right end is free of them.
            WindowStyle.TillingWm => (HorizontalAlignment.Right, new Thickness(8, 0, 12, 0)),

            _ => (
                HorizontalAlignment.Right,
                new Thickness(8, 0, (CaptionButtonWidth * CaptionButtonCount) + 12, 0)
            ),
        };

        return new TextBlock
        {
            Text = text,
            Classes = { WindowTitleClass },
            HorizontalAlignment = side,
            Margin = margin,

            // The pointer says what the name is for before it is pressed: a hand over a word that begins a
            // drag is told so, and a word that could only be read would be a drag nobody finds.
            Cursor = new Cursor(StandardCursorType.SizeAll),

            // Cut off rather than drawn over the menu it shares the row with: a dock with a long name is a
            // name that would otherwise reach the menus, and what a name is worth is less than what a menu
            // is.
            TextTrimming = TextTrimming.CharacterEllipsis,
        };
    }

    /// <summary>
    /// Whether what a press landed on is the name of the dock a window of its own holds.
    /// </summary>
    /// <remarks>
    /// Walked rather than compared, because what the pointer hits is a part of the name's own template
    /// rather than the name — the same reason what landed on a menu is walked to the item.
    /// </remarks>
    /// <param name="pressed">What the press landed on.</param>
    /// <returns>Whether it is that name.</returns>
    public static bool TitleUnder(object? pressed)
    {
        for (var at = pressed as StyledElement; at is not null; at = at.Parent)
        {
            if (at is TextBlock title && title.Classes.Contains(WindowTitleClass))
            {
                return true;
            }
        }

        return false;
    }

    /// <summary>
    /// The frame a window draws for itself, over the one the desktop would have drawn.
    /// </summary>
    /// <remarks>
    /// The desktop's title bar is not wanted and neither is a second one: what this states is a frame the
    /// window's own row shows through, carrying the two things the toolkit is still needed for — the area a
    /// press on the row moves the window by, and the window's own buttons. Everything the frame is drawn
    /// with is the window's own content, so the row and the frame are one thing.
    /// <para>
    /// It is made for the window it is put on rather than named as a resource, because an implicit theme
    /// is found by type and every window of the program would then be given it: a plugin's question and a
    /// notice are cards of their own with a title bar of the desktop's, which is what a card should have.
    /// </para>
    /// <para>
    /// The buttons come back with the theme because they are made while it is, and the window that puts the
    /// theme on is the only thing that can say what they are named and what they are drawn as.
    /// </para>
    /// </remarks>
    /// <param name="style">Which of the three ways a window's frame is drawn this window is drawn in.</param>
    /// <param name="row">
    /// The height the window draws its own row at, which is the room the frame keeps for it. It is the
    /// window's rather than one number for every window, because the row and what the toolkit reserves for a
    /// title bar have to be the same height or the frame's buttons are drawn where the row is not.
    /// </param>
    /// <returns>The theme to put on one window, and the buttons it draws.</returns>
    public static Frame DecorationsTheme(WindowStyle style, double row)
    {
        var scope = new NameScope();
        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            HorizontalAlignment = style == WindowStyle.Mac ? HorizontalAlignment.Left : HorizontalAlignment.Right,

            // At the top, because what this is laid out in is the whole of the window's client area rather
            // than the row: the frame draws its buttons over everything, and a row centred in that would put
            // them in the middle of the window. The height is one row's, so where they are is where the row is.
            VerticalAlignment = VerticalAlignment.Top,
            Height = row,

            // The lights stand off the window's own edge and off one another, where the three marks are a
            // row of one control each: room left between two of them is room the menu bar would otherwise be
            // drawn in.
            Spacing = style == WindowStyle.Mac ? MacGap : 0,
            Margin = style == WindowStyle.Mac ? new Thickness(MacEdge, 0, 0, 0) : default,
        };

        // A tiling window manager's window has none: the manager outside keeps the window, and buttons drawn
        // here would be a frame inside the frame it already has.
        Button? minimize = null;
        Button? maximize = null;
        Button? close = null;

        if (style != WindowStyle.TillingWm)
        {
            // Made in the order they are drawn, which is the style's own: Windows puts its three marks in the
            // order they read — minimize, maximize, close — and macOS puts the one that closes the window at
            // the window's own corner, then the light that puts it away, then the one that makes it fill the
            // screen.
            if (style == WindowStyle.Mac)
            {
                close = Caption(scope, buttons, "PART_CloseButton", WindowDecorationsElementRole.CloseButton, style, MacClose, closes: true);
                minimize = Caption(scope, buttons, "PART_MinimizeButton", WindowDecorationsElementRole.MinimizeButton, style, MacMinimize);
                maximize = Caption(scope, buttons, "PART_MaximizeButton", WindowDecorationsElementRole.MaximizeButton, style, MacMaximize);
            }
            else
            {
                minimize = Caption(scope, buttons, "PART_MinimizeButton", WindowDecorationsElementRole.MinimizeButton, style, MacMinimize);
                maximize = Caption(scope, buttons, "PART_MaximizeButton", WindowDecorationsElementRole.MaximizeButton, style, MacMaximize);
                close = Caption(scope, buttons, "PART_CloseButton", WindowDecorationsElementRole.CloseButton, style, MacClose, closes: true);
            }
        }

        var theme = new ControlTheme(typeof(WindowDrawnDecorations));

        // The height the toolkit believes the bar is, which is the height the row is drawn at and the
        // height the window's own hint is given. The frame and the shadow are none: the one-pixel frame
        // is the desktop's own where the desktop draws edges, and on a display server that leaves the
        // frame to the program a line around the whole window would be the design's only one.
        theme.Setters.Add(new Setter(WindowDrawnDecorations.DefaultTitleBarHeightProperty, row));
        theme.Setters.Add(new Setter(WindowDrawnDecorations.DefaultFrameThicknessProperty, default(Thickness)));
        theme.Setters.Add(new Setter(WindowDrawnDecorations.DefaultShadowThicknessProperty, default(Thickness)));
        theme.Setters.Add(
            new Setter(
                WindowDrawnDecorations.TemplateProperty,
                new DecorationsTemplate(new TemplateResult<WindowDrawnDecorationsContent>(
                    new WindowDrawnDecorationsContent
                    {
                        Underlay = new Panel { Children = { DragArea(row) } },

                        // The buttons are in the overlay rather than in the window's own content, because
                        // that is what puts them above the menu: the menu is the window's, and a row whose
                        // buttons could be covered by it would be covered by it the moment a plugin added
                        // one menu too many.
                        Overlay = new Panel { Children = { buttons } },
                    },
                    scope))
            )
        );

        return new Frame(theme, minimize, maximize, close);
    }

    /// <summary>
    /// One of the window's own buttons.
    /// </summary>
    /// <remarks>
    /// What each does is the toolkit's, from the role it carries, and it is the toolkit that reads them out
    /// of the template's names and wires them up. What is left here is what a window button is made of: a
    /// part the toolkit looks for, a role, and the class the look draws it by — and, where the frame is
    /// drawn the way macOS draws one, the light's own colour, which is the one thing about it the look
    /// cannot say in a rule, three buttons of one class being three different colours.
    /// </remarks>
    /// <param name="scope">The names the toolkit reads the parts out of.</param>
    /// <param name="row">The row to add it to.</param>
    /// <param name="name">The part name the toolkit looks for.</param>
    /// <param name="role">What the toolkit makes a press on it do.</param>
    /// <param name="style">Which way the window's frame is being drawn.</param>
    /// <param name="light">The colour of the light, where the frame is drawn the way macOS draws one.</param>
    /// <param name="closes">Whether it is the one that closes the window.</param>
    /// <returns>The button, which the window names and draws.</returns>
    private static Button Caption(
        NameScope scope,
        StackPanel row,
        string name,
        WindowDecorationsElementRole role,
        WindowStyle style,
        Color light,
        bool closes = false
    )
    {
        var button = new Button
        {
            Name = name,
            Classes = { CaptionClass },
            VerticalAlignment = style == WindowStyle.Mac
                ? VerticalAlignment.Center
                : VerticalAlignment.Stretch,
            [WindowDecorationProperties.ElementRoleProperty] = role,
        };

        if (closes)
        {
            button.Classes.Add(CaptionCloseClass);
        }

        if (style == WindowStyle.Mac)
        {
            button.Width = MacLight;
            button.Height = MacLight;
            button.CornerRadius = Pill;
            button.Background = new SolidColorBrush(light);
            button.Foreground = new SolidColorBrush(MacInk);
        }

        scope.Register(name, button);
        row.Children.Add(button);

        return button;
    }

    /// <summary>The area a press on the row moves the window by.</summary>
    /// <remarks>
    /// Transparent and under everything: it is what a press finds where no menu and no button claimed it.
    /// The role is what a display server that draws its own decorations reads to move the window — the
    /// desktop moves it through the window's own code instead, which is why the role is here and the call
    /// is in the window's.
    /// </remarks>
    /// <param name="row">How tall the window's own row is, which is the band a press there moves it by.</param>
    private static Control DragArea(double row) =>
        new Panel
        {
            Background = Brushes.Transparent,
            Height = row,
            VerticalAlignment = VerticalAlignment.Top,
            [WindowDecorationProperties.ElementRoleProperty] = WindowDecorationsElementRole.TitleBar,
        };

    /// <summary>
    /// The parts of a window's own frame: the theme that states it, and the buttons it draws.
    /// </summary>
    /// <remarks>
    /// They travel together because they are made together, and the window that puts the theme on is the
    /// only thing that can say what the buttons are named and what they are drawn as.
    /// <para>
    /// A button is nothing where the frame is drawn for a tiling window manager, which has none of them: what
    /// the window does with a button it was not given is nothing, which is why they are asked for by name
    /// rather than assumed.
    /// </para>
    /// </remarks>
    /// <param name="Theme">The theme to put on the window.</param>
    /// <param name="Minimize">The button that minimizes it, or nothing where the frame has none.</param>
    /// <param name="Maximize">The button that maximizes it, or puts it back, or nothing.</param>
    /// <param name="Close">The button that closes it, or nothing.</param>
    public readonly record struct Frame(
        ControlTheme Theme,
        Button? Minimize,
        Button? Maximize,
        Button? Close
    );

    /// <summary>
    /// The two halves of a window's own frame: the area a press moves it by, and its buttons.
    /// </summary>
    /// <remarks>
    /// The parts are built rather than written as markup because what is wanted here is two panels and
    /// three buttons, and a file for that would be a second place the shape of the window is written down.
    /// Nothing in them binds to the decorations they belong to, since a built template is given no templated
    /// parent to bind against: the height the buttons are is the row's height, and that is the one number
    /// the window, the look and the markup all name (<see cref="MainWindow.TitleBarHeight"/>).
    /// </remarks>
    /// <param name="content">What the template hands the toolkit, built once while the window was made.</param>
    private sealed class DecorationsTemplate(TemplateResult<WindowDrawnDecorationsContent> content)
        : IWindowDrawnDecorationsTemplate
    {
        /// <inheritdoc />
        public TemplateResult<WindowDrawnDecorationsContent> Build() => content;

        /// <summary>
        /// The interface a template is held as, which is what the toolkit reads it through.
        /// </summary>
        /// <remarks>
        /// Written out because one type cannot answer one method with two return types: the toolkit's
        /// template interface is the one that carries the content and the names, and what a caller of the
        /// other would get is that same content.
        /// </remarks>
        object? ITemplate.Build() => content.Result;
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

