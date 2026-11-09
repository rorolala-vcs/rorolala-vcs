using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Input.Platform;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// The Version Graph dock: the variants of the work, their versions and the entries naming them.
/// </summary>
/// <remarks>
/// A card is a variant, a lane is the file whose chain it stands on, and a column is its version
/// depth; the drawing is the desktop's own geometry and every card is where the reading puts it, not
/// the command line's character grid. The graph stands on the entries chosen in a directory listing,
/// or on the whole work when nothing is chosen (see <see cref="VersionGraphView"/>), and the row under
/// it records the change the reader has just made.
/// </remarks>
internal sealed class VersionGraphDock : IDockView
{
    /// <summary>The control the dock shows.</summary>
    private readonly Control _view;

    /// <summary>The graph itself, which the header's Refresh reaches.</summary>
    private readonly VersionGraphView _graph;

    /// <summary>Makes the dock.</summary>
    /// <param name="host">
    /// The host, whose log says a reading that failed, whose files say when to read again, whose
    /// icons the node menu is drawn with, and whose windows being come back to is a reading too.
    /// </param>
    /// <param name="statuses">What the work says about the files, for the drafts it may stand at.</param>
    /// <param name="sync">What a sync would carry, which a fetch is not made over.</param>
    /// <param name="fetch">The one fetch of the Vault's Layout, which the header's Refresh makes.</param>
    /// <param name="hub">What every command this plugin runs reports itself to.</param>
    public VersionGraphDock(
        IPluginHost host,
        StatusReadings statuses,
        SyncWatcher sync,
        VaultFetch fetch,
        IProgressHub hub
    )
    {
        ArgumentNullException.ThrowIfNull(host);

        var graph = new VersionGraphView(
            host.Log,
            host.Files,
            host.Icons,
            host.Palette,
            statuses,
            host.Refocus,
            sync,
            fetch,
            host.Dialogs,
            hub,
            host.Windows
        );

        // The row under the graph is always there: a reader who has just changed a file is already
        // looking at the graph and has nowhere else to say what the change was.
        var stage = new Grid { Children = { graph } };
        var toggle = InfoToggle(host, graph);

        stage.Children.Add(toggle);

        // The table is drawn after the switch and over it, so that when it appears it covers the switch
        // rather than standing under it. It takes no pointer: the switch stays the thing the pointer
        // meets, and the table is gone by the time the pointer is over it.
        var table = graph.InfoTable;
        table.IsHitTestVisible = false;
        stage.Children.Add(table);

        var root = new Grid { RowDefinitions = new RowDefinitions("*,Auto") };
        var bar = new VersionGraphTrackBar(host, graph, statuses, hub);

        root.Children.Add(stage);
        Grid.SetRow(bar, 1);
        root.Children.Add(bar);

        _graph = graph;
        _view = root;
    }

    /// <summary>
    /// The switch in the graph's corner: whether hovering a thing says what it is.
    /// </summary>
    /// <remarks>
    /// It is drawn over the graph rather than in the dock's strip because it is about the drawing: a
    /// reader looking at the graph is looking there for what it is made of. The table it shows is laid
    /// over the graph's own corner so that the drawing is not given less room for it.
    /// </remarks>
    /// <param name="host">The host, whose pictures the switch is drawn with.</param>
    /// <param name="graph">The graph it turns the hover table on and off for.</param>
    /// <returns>The switch.</returns>
    private static ToggleButton InfoToggle(IPluginHost host, VersionGraphView graph)
    {
        var toggle = new ToggleButton
        {
            Classes = { "tool" },
            HorizontalAlignment = HorizontalAlignment.Right,
            VerticalAlignment = VerticalAlignment.Top,
            Margin = new Thickness(0, VersionGraphView.Corner, VersionGraphView.Corner, 0),
            Padding = new Thickness(6),
            IsChecked = true,
        };

        Border? mark = null;

        if (host.Icons.Find(Icons.Visibility) is { } picture)
        {
            mark = new Border
            {
                Width = 14,
                Height = 14,
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
                OpacityMask = new ImageBrush(picture) { Stretch = Stretch.Uniform },
            };

            RenderOptions.SetBitmapInterpolationMode(mark, BitmapInterpolationMode.HighQuality);
            mark[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Front);
            toggle.Content = mark;
        }

        toggle.Classes.Add("on");
        ToolTip.SetTip(toggle, RolaI18N.Get("rorolala_vcs.version_graph.show_info"));

        toggle.IsCheckedChanged += (_, _) =>
        {
            var on = toggle.IsChecked == true;

            if (on)
            {
                toggle.Classes.Add("on");
            }
            else
            {
                toggle.Classes.Remove("on");
            }

            if (mark is not null)
            {
                mark[!Border.BackgroundProperty] = new DynamicResourceExtension(
                    on ? PaletteKeys.PrimaryText : PaletteKeys.Front
                );
            }

            graph.ShowInfo(on);
        };

        return toggle;
    }

    /// <inheritdoc />
    public Control View => _view;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderItem> Header =>
        [
            new DockHeaderCommand(
                "rorolala_vcs.status.refresh",
                _graph.RefreshWithFetch,
                IconKey: Icons.Refresh
            ),
        ];
}

/// <summary>
/// The graph itself: the variant graph laid out as geometry, and the pointer's zoom, drag and menu.
/// </summary>
/// <remarks>
/// The reading is the native side's, taken whole off the window's thread; what the work says about the
/// files (which are being edited and which changes are wrong) is read beside it and becomes the draft
/// cards. The unit of the drawing is a content unit: <see cref="VariantGraphRenderer"/> applies the
/// zoom, so what is laid out and measured here is the same at every scale.
/// <para>
/// The choice decides what the drawing is made of, but taking the choice away does not throw the drawing
/// out: the same cards stay, their focused chains lose their emphasis, and the next file chosen is what
/// reads another graph. A click on the ground would otherwise replace a focused graph with every variant
/// of the work, which is a large change for a click that chose nothing.
/// </para>
/// </remarks>
internal sealed partial class VersionGraphView : Control
{
    /// <summary>The least and the most the wheel can scale the drawing.</summary>
    private const double MinZoom = 0.18;

    /// <inheritdoc cref="MinZoom" />
    private const double MaxZoom = 2.5;

    /// <summary>What one wheel notch scales by.</summary>
    private const double ZoomStep = 1.15;

    /// <summary>The room the drawing keeps from the edge of the dock.</summary>
    private const double Inset = 24;

    /// <summary>The room the switch in the corner keeps from the top and the right edge.</summary>
    internal const double Corner = 12;

    /// <summary>What a press on a card brings the drawing up to, when it is smaller.</summary>
    private const double FocusZoom = 1.3;

    /// <summary>How much larger than the drawing the view may be panned away from it.</summary>
    private const double PanSlack = 2;

    /// <summary>The overlay that draws the hover table over the switch in the corner.</summary>
    private InfoOverlay? _infoOverlay;

    /// <summary>Where a reading that failed is said.</summary>
    private readonly ILog _log;

    /// <summary>What says that the files may have changed, so that the graph is read again.</summary>
    private readonly IFileChanges _files;

    /// <summary>The pictures a node's menu is drawn with.</summary>
    private readonly IIconLibrary _icons;

    /// <summary>The colours the program is drawn in, answered afresh each time they are read.</summary>
    private readonly IPalette _palette;

    /// <summary>What the work says about the files, for the drafts it may stand at.</summary>
    private readonly StatusReadings _statuses;

    /// <summary>The host's windows being come back to, which the graph is read again for.</summary>
    private readonly IRefocus _refocus;

    /// <summary>What a sync would carry, which is the run a fetch would wait for.</summary>
    private readonly SyncWatcher _sync;

    /// <summary>The one fetch of the Vault's Layout, which the header's Refresh makes.</summary>
    private readonly VaultFetch _fetch;

    /// <summary>Where a question is put to the user, and a run that failed is reported.</summary>
    private readonly IDialogs _dialogs;

    /// <summary>What every command this plugin runs reports itself to.</summary>
    private readonly IProgressHub _hub;

    /// <summary>The host's windows, which a press on an entry asks to go to its file.</summary>
    private readonly IWindows _windows;

    /// <summary>Measures the graph's text in content units, so its pills are laid out once.</summary>
    private readonly GraphMeasure _measure = new();

    /// <summary>What waits for a change to stop being said before reading.</summary>
    /// <remarks>
    /// A change arrives more than once, the browser says so, whoever made it says so, and both a click
    /// and a touch follow one operation, and every reading walks the work. A quiet moment turns a burst
    /// into the one read it means, and lets what the browser let go of be gone before the work is read
    /// again.
    /// </remarks>
    private readonly DispatcherTimer _settle = new() { Interval = TimeSpan.FromMilliseconds(140) };

    /// <summary>The frame timer that eases a change of scale or place into view.</summary>
    private readonly DispatcherTimer _glide = new() { Interval = TimeSpan.FromMilliseconds(15) };

    /// <summary>How much of the way to the target a frame takes.</summary>
    private const double GlidePart = 0.22;

    /// <summary>The graph on screen, or nothing while a read is going or when there is none.</summary>
    private VariantGraphDrawing? _drawing;

    /// <summary>Draws the graph, made once with the words its edges are labelled with.</summary>
    private VariantGraphRenderer? _renderer;

    /// <summary>The words the drawing and the renderer are made with.</summary>
    private GraphVocabulary? _words;

    /// <summary>The colours in force, until the look changes.</summary>
    private GraphInk? _ink;

    /// <summary>What the colours were read from, so a changed one is noticed.</summary>
    private (int Theme, Color Ground, Color Primary, Color Front) _inkFor;

    /// <summary>Whether hovering a thing says what it is, which the switch in the corner turns.</summary>
    private bool _showInfo = true;

    /// <summary>
    /// Whether the drawing's focused chains are emphasised. Taking the choice away stops the emphasis but
    /// keeps the drawing: a graph that jumped to every variant when the pointer left a file would be one
    /// that changed more than the click did.
    /// </summary>
    private bool _showFocus;

    /// <summary>How many files the last choice held, so a choice that became empty is told from no choice.</summary>
    private int _chosenCount;

    /// <summary>
    /// Whether the graph is being kept after its choice was taken away. The next choice of a file ends it.
    /// </summary>
    private bool _retained;

    /// <summary>What is said in place of a drawing, or nothing while a read is going.</summary>
    private string? _emptyText;

    /// <summary>The file the current drawing stands on, when the choice names exactly one.</summary>
    private string? _target;

    /// <summary>Where the Workspace the drawing was read of is rooted, when it was read.</summary>
    private string? _root;

    /// <summary>What the drawing was read for, so a new subject is framed afresh.</summary>
    private string _subject = string.Empty;

    /// <summary>What subject the drawing was last framed for.</summary>
    private string _fitted = string.Empty;

    /// <summary>How large the drawing is drawn.</summary>
    private double _zoom = 1;

    /// <summary>How far the drawing has been moved, in screen units.</summary>
    private double _panX = Inset;

    /// <inheritdoc cref="_panX" />
    private double _panY = Inset;

    /// <summary>The scale the view is easing towards.</summary>
    private double _targetZoom = 1;

    /// <summary>How far the view is easing the drawing towards.</summary>
    private double _targetPanX = Inset;

    /// <inheritdoc cref="_targetPanX" />
    private double _targetPanY = Inset;

    /// <summary>Where the pointer went down on the empty ground, which is where a click is told from a drag.</summary>
    private Point _pressed;

    /// <summary>Whether the pointer has moved enough to be a drag rather than a click.</summary>
    private bool _moved;

    /// <summary>The scale said in the corner.</summary>
    private string _zoomText = "100%";

    /// <summary>The thing the pointer is on, when it is on one.</summary>
    private Hover? _hover;

    /// <summary>Where the pointer is, which is where a bubble stands.</summary>
    private Point _pointer;

    /// <summary>Whether a drag that moves the drawing is on.</summary>
    private bool _dragging;

    /// <summary>Where the pointer went down, while a drag is on.</summary>
    private Point _from;

    /// <summary>Which read is current, so a slower earlier read cannot draw over a newer answer.</summary>
    private int _generation;

    /// <summary>Whether a read is in flight, so a burst of changes starts one read rather than several.</summary>
    private bool _reading;

    /// <summary>The generation of the read <see cref="_reading"/> is about.</summary>
    private int _readingFor;

    /// <summary>Whether a change was said while a read was in flight, so the newest answer is read after it.</summary>
    private bool _again;

    /// <summary>Which look is in force, raised by the theme changing what it publishes.</summary>
    private int _themeVersion;

    /// <summary>Makes the view.</summary>
    /// <param name="log">Where a reading that failed is said.</param>
    /// <param name="files">What says that the files may have changed.</param>
    /// <param name="icons">The pictures a node's menu is drawn with.</param>
    /// <param name="palette">The colours the program is drawn in.</param>
    /// <param name="statuses">What the work says about the files.</param>
    /// <param name="refocus">The host's windows being come back to.</param>
    /// <param name="sync">What a sync would carry.</param>
    /// <param name="fetch">The one fetch of the Vault's Layout.</param>
    /// <param name="dialogs">Where a question is put to the user, and a failure reported.</param>
    /// <param name="hub">What every command this plugin runs reports itself to.</param>
    /// <param name="windows">The host's windows, which a press on an entry asks to show its file.</param>
    public VersionGraphView(
        ILog log,
        IFileChanges files,
        IIconLibrary icons,
        IPalette palette,
        StatusReadings statuses,
        IRefocus refocus,
        SyncWatcher sync,
        VaultFetch fetch,
        IDialogs dialogs,
        IProgressHub hub,
        IWindows windows
    )
    {
        ArgumentNullException.ThrowIfNull(log);
        ArgumentNullException.ThrowIfNull(files);
        ArgumentNullException.ThrowIfNull(icons);
        ArgumentNullException.ThrowIfNull(palette);
        ArgumentNullException.ThrowIfNull(statuses);
        ArgumentNullException.ThrowIfNull(refocus);
        ArgumentNullException.ThrowIfNull(sync);
        ArgumentNullException.ThrowIfNull(fetch);
        ArgumentNullException.ThrowIfNull(dialogs);
        ArgumentNullException.ThrowIfNull(hub);
        ArgumentNullException.ThrowIfNull(windows);

        _log = log;
        _files = files;
        _icons = icons;
        _palette = palette;
        _statuses = statuses;
        _refocus = refocus;
        _sync = sync;
        _fetch = fetch;
        _dialogs = dialogs;
        _hub = hub;
        _windows = windows;
        _settle.Tick += Settled;
        _glide.Tick += Glided;

        // The first drawing may be read before the dock has a size to frame it in, and a dock that is
        // resized before the next reading would keep a frame that no longer holds the work.
        SizeChanged += (_, _) =>
        {
            if (_drawing is not null && !string.Equals(_subject, _fitted, StringComparison.Ordinal))
            {
                Fit();
                InvalidateVisual();
            }
        };

        Focusable = true;
        ClipToBounds = true;
        Cursor = Cursor.Default;
        _emptyText = RolaI18N.Get("rorolala_vcs.version_graph.empty");
    }

    /// <summary>
    /// Raised when the graph's answer about what may be tracked changes: the files, whether any is an
    /// edit, whether the chains have parted, or that the work cannot be read.
    /// </summary>
    /// <remarks>
    /// The row under the graph draws a message and one button, and what the button may do is read from
    /// the work rather than from the drawing; this is the view saying that its own reading of the work
    /// has changed, so that the row asks the work again rather than waiting for a click it would not be
    /// told about.
    /// </remarks>
    internal event EventHandler? TrackableChanged;

    /// <summary>The words the drawing is made with.</summary>
    private GraphVocabulary Words =>
        _words ??= new GraphVocabulary(
            DraftLabel: RolaI18N.Get("rorolala_vcs.version_graph.draft_label"),
            DraftMessage: RolaI18N.Get("rorolala_vcs.version_graph.draft_message"),
            DraftWrongMessage: RolaI18N.Get("rorolala_vcs.version_graph.draft_wrong_message"),
            DraftUnownedMessage: RolaI18N.Get("rorolala_vcs.version_graph.draft_unowned_message"),
            DraftBadge: RolaI18N.Get("rorolala_vcs.version_graph.draft_badge"),
            DraftWrongBadge: RolaI18N.Get("rorolala_vcs.version_graph.draft_wrong_badge"),
            DraftUnownedBadge: RolaI18N.Get("rorolala_vcs.version_graph.draft_unowned_badge"),
            PendingInto: RolaI18N.Get("rorolala_vcs.version_graph.pending_into"),
            NoEntries: RolaI18N.Get("rorolala_vcs.version_graph.no_entries"),
            UnknownBase: RolaI18N.Get("rorolala_vcs.version_graph.unknown_base")
        );

    /// <summary>The renderer, made the first time a drawing is.</summary>
    private VariantGraphRenderer Renderer => _renderer ??= new VariantGraphRenderer(Words);

    /// <inheritdoc />
    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs e)
    {
        base.OnAttachedToVisualTree(e);

        // Listening while the view is on screen rather than for as long as it exists: the catalogue
        // outlives every dock, and a dock that was closed has no reason to be told what was clicked.
        _chosenCount = ChosenFiles.Current.Count;
        ChosenEntry.Changed += EntryChanged;
        ChosenFiles.Changed += ChoiceChanged;
        BaseDirectory.Changed += Refresh;
        _files.Changed += Refresh;

        // And when a reading this graph draws from lands: the status readings are read off the window's
        // thread, so the first drawing of a file can find them not yet there, and this is what draws it
        // again.
        _files.Redrawn += Refresh;
        _refocus.Regained += Refresh;
        ResourcesChanged += ResourcesChangedHandler;
        ActualThemeVariantChanged += ThemeVariantChangedHandler;
        Refresh();
    }

    /// <inheritdoc />
    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        ChosenEntry.Changed -= EntryChanged;
        ChosenFiles.Changed -= ChoiceChanged;
        BaseDirectory.Changed -= Refresh;
        _files.Changed -= Refresh;
        _files.Redrawn -= Refresh;
        _refocus.Regained -= Refresh;
        ResourcesChanged -= ResourcesChangedHandler;
        ActualThemeVariantChanged -= ThemeVariantChangedHandler;

        // A read that was in flight is no longer this view's to draw.
        _generation++;
        _again = false;
        _settle.Stop();
        _glide.Stop();
        base.OnDetachedFromVisualTree(e);
    }

    /// <summary>What the look publishes changed, so the drawing is read again when next drawn.</summary>
    private void ResourcesChangedHandler(object? sender, ResourcesChangedEventArgs e) => Retheme();

    /// <summary>The variant changed, which is a look change like any other.</summary>
    private void ThemeVariantChangedHandler(object? sender, EventArgs e) => Retheme();

    /// <summary>Drops what the old look shaped and draws again.</summary>
    private void Retheme()
    {
        _themeVersion++;
        _ink = null;
        _renderer?.Invalidate();
        InvalidateVisual();
    }

    /// <inheritdoc />
    protected override Size MeasureOverride(Size availableSize) =>
        new(
            double.IsInfinity(availableSize.Width) ? 640 : availableSize.Width,
            double.IsInfinity(availableSize.Height) ? 480 : availableSize.Height
        );

    /// <summary>
    /// Fetches the Vault's Layout under the work, then reads the graph again, which is what the dock's
    /// own Refresh asks for.
    /// </summary>
    /// <remarks>
    /// A graph's marked version and its drafts are read from the copy the Vault's Layout is kept in, so
    /// a graph made before a fetch would say where the Vault stood when the copy was last brought over.
    /// A directory no Workspace holds has nothing to fetch; a sync is what brings the same copy, so its
    /// end is what reads the graph then, rather than a second exchange made over it.
    /// </remarks>
    public void RefreshWithFetch()
    {
        if (_sync.Running)
        {
            return;
        }

        if (
            Subject() is { } entry
            && Path.GetDirectoryName(entry.Path) is { Length: > 0 } at
            && WorkspaceNotice.Holds(at, _log)
        )
        {
            _fetch.Fetch(at, Refresh);

            return;
        }

        Refresh();
    }

    /// <summary>
    /// Says that what the graph shows may have changed, and leaves the reading to a quiet moment.
    /// </summary>
    internal void Refresh()
    {
        // A plugin may say that a command of its own changed the files from the thread the command ran
        // on, so the words the graph keeps are only touched on the window's thread.
        if (!Dispatcher.UIThread.CheckAccess())
        {
            Dispatcher.UIThread.Post(Refresh);

            return;
        }

        _settle.Stop();
        _settle.Start();
    }

    /// <summary>
    /// Says that the file clicked last changed, which is a choice of a file again.
    /// </summary>
    private void EntryChanged()
    {
        _retained = false;
        Refresh();
    }

    /// <summary>
    /// Says that a choice was made or taken away, and keeps the drawing when it became empty.
    /// </summary>
    /// <remarks>
    /// A file clicked in a listing is also <see cref="ChosenEntry"/>'s answer, and an empty choice leaves
    /// that answer standing: the graph used to fall back to the last clicked file and then read it again.
    /// What an empty choice means here is that the emphasis goes and the drawing stays, until a file is
    /// chosen again.
    /// </remarks>
    private void ChoiceChanged()
    {
        var count = ChosenFiles.Current.Count;

        if (count == 0 && _chosenCount > 0)
        {
            _retained = true;
            HoldDrawing();
        }
        else if (count > 0)
        {
            _retained = false;
        }

        _chosenCount = count;
        Refresh();
    }

    /// <summary>Reads what <see cref="Refresh"/> said to wait for.</summary>
    private void Settled(object? sender, EventArgs e)
    {
        _settle.Stop();
        Read();
    }

    /// <summary>Takes the choice's emphasis away while leaving the drawing where it is.</summary>
    /// <remarks>
    /// The drawings of neighbouring files share most of their cards, so replacing one with the whole work
    /// the moment a click lands on the ground would move almost everything for a click that chose nothing.
    /// The cards stay, the focused chains stop being picked out, and the next chosen file is what reads a
    /// new graph.
    /// </remarks>
    private void HoldDrawing()
    {
        _showFocus = false;
        _hover = null;
        _again = false;

        if (_reading)
        {
            // The read was for a choice that is gone, and its answer must not arrive over the drawing that
            // is being left alone.
            _reading = false;
            _generation++;
        }

        UpdateCursor();
        RedrawInfo();
        InvalidateVisual();
    }

    /// <summary>The one file the graph would act on, or nothing when none or several are chosen.</summary>
    /// <remarks>
    /// The choice is what the user made most recently: a choice of one is that file, a choice of none
    /// leaves the file clicked last standing, and a choice of several is no one file at all.
    /// </remarks>
    /// <returns>The file whose actions the graph offers, or nothing.</returns>
    private static Entry? Subject() =>
        ChosenFiles.Current.Count switch
        {
            0 => ChosenEntry.Current,
            1 => ChosenFiles.Current[0],
            _ => null,
        };

    /// <summary>The files the drawing is centred on: the chosen ones under the browsed directory, or none.</summary>
    /// <remarks>
    /// The graph stands on the work being browsed, so a file clicked in a directory left behind is not
    /// a focus for the directory now open: what is left is the whole of the work being looked at.
    /// </remarks>
    /// <returns>The entries, in the order the listing shows them.</returns>
    private static IReadOnlyList<Entry> Considered()
    {
        var chosen = ChosenFiles.Current.Count > 0
            ? ChosenFiles.Current
            : ChosenEntry.Current is { } one
                ? [one]
                : [];

        if (BaseDirectory.Current is not { Length: > 0 } browsed)
        {
            return chosen;
        }

        return [.. chosen.Where(entry => Under(browsed, entry.Path))];
    }

    /// <summary>Whether a file lies under a directory.</summary>
    /// <param name="directory">The directory.</param>
    /// <param name="file">The file, as the program names it.</param>
    /// <returns>Whether it is the directory or something below it.</returns>
    private static bool Under(string directory, string file)
    {
        var at = Path.GetFullPath(directory);
        var path = Path.GetFullPath(file);
        var comparison = OperatingSystem.IsWindows()
            ? StringComparison.OrdinalIgnoreCase
            : StringComparison.Ordinal;

        return path.StartsWith(at + Path.DirectorySeparatorChar, comparison)
            || string.Equals(path, at, comparison);
    }

    /// <summary>The directory the work is read from: the one being browsed, or the chosen file's.</summary>
    /// <param name="entries">The entries the drawing is centred on.</param>
    /// <returns>The directory, or nothing when nothing says one.</returns>
    private static string? DirectoryOf(IReadOnlyList<Entry> entries) =>
        BaseDirectory.Current
        ?? (entries.Count > 0 && Path.GetDirectoryName(entries[0].Path) is { Length: > 0 } at
            ? at
            : null);

    /// <summary>
    /// Reads the work's variant graph, and the drafts the work says its files hold.
    /// </summary>
    /// <remarks>
    /// The reading is a walk of the index and of the Layouts, and is done off the window's thread,
    /// because a dock that stopped drawing while a graph was read would stop for every click in a
    /// listing. One read is in flight at a time: a change said while one is going is read after it
    /// rather than beside it, since two reads of the same work would answer the same and discard each
    /// other. The generation is what keeps a read that was overtaken from drawing after its view left
    /// the screen.
    /// </remarks>
    private void Read()
    {
        if (_reading)
        {
            _again = true;

            return;
        }

        var entries = Considered();

        // Taking the choice away is not a reason to read the whole work: the graph that was on screen is
        // still the graph the reader was looking at, and it stays while the emphasis goes. A read that was
        // already in flight is overtaken here, so it cannot put another graph up after the choice is gone.
        if (
            _drawing is not null
            && (
                (_retained && ChosenFiles.Current.Count == 0)
                || (entries.Count == 0 && ChosenFiles.Current.Count == 0)
            )
        )
        {
            HoldDrawing();

            return;
        }

        var directory = DirectoryOf(entries);
        var generation = ++_generation;

        if (directory is null)
        {
            _target = null;
            _root = null;
            ShowMessage(RolaI18N.Get("rorolala_vcs.version_graph.empty"));

            return;
        }

        _reading = true;
        _readingFor = generation;

        var files = entries.Select(entry => entry.Path).ToArray();

        _ = Task.Run(() =>
        {
            RolaVariantGraph? graph = null;
            IReadOnlyList<DraftFact> drafts = [];
            var emphasized = false;
            string? root = null;

            try
            {
                var status = _statuses.Reading(directory);
                root = status?.Root;
                var focus = FocusOf(files, root);

                emphasized = focus.Count > 0;
                graph = RolaVariantGraph.Locate(
                    directory,
                    new VariantGraphRequest { Focus = focus.Count > 0 ? focus : null }
                );

                if (graph is not null && status is not null && root is not null)
                {
                    drafts = DraftsOf(graph, status, root);
                }
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                _log.Warn($"the variant graph of `{directory}` could not be read: {error.Message}");
            }

            Dispatcher.UIThread.Post(() =>
            {
                var current = _readingFor == generation;

                if (current)
                {
                    _reading = false;
                }

                if (generation != _generation)
                {
                    // The view is no longer on screen: the reading is this thread's to give back and
                    // the drawing's to discard.
                    return;
                }

                if (graph is null || graph.Nodes.Count == 0)
                {
                    _target = files.Length == 1 ? files[0] : null;
                    _root = root;
                    _showFocus = false;
                    ShowMessage(RolaI18N.Get("rorolala_vcs.version_graph.empty"));
                }
                else
                {
                    var subject = $"{directory}|{emphasized}|{string.Join('|', files)}";
                    var moved = !string.Equals(subject, _subject, StringComparison.Ordinal);

                    _drawing = VariantGraphDrawing.From(graph, drafts, Words, _measure, emphasized);
                    _renderer?.Invalidate();
                    _emptyText = null;
                    _showFocus = emphasized;
                    _hover = null;
                    RedrawInfo();
                    _target = files.Length == 1 ? files[0] : null;
                    _root = root;
                    _subject = subject;

                    if (moved || _fitted.Length == 0)
                    {
                        Fit();
                    }

                    UpdateCursor();
                    InvalidateVisual();
                    TrackableChanged?.Invoke(this, EventArgs.Empty);
                }

                if (current && _again)
                {
                    _again = false;
                    Read();
                }
            });
        });
    }

    /// <summary>The entries the reading is centred on, as Layout-relative paths.</summary>
    /// <param name="files">The files the choice names, as the program names them.</param>
    /// <param name="root">Where the Workspace is rooted, or nothing when it was not read yet.</param>
    /// <returns>The focus the native side is asked for; empty stands on the whole work.</returns>
    private static List<VariantGraphFocus> FocusOf(IReadOnlyList<string> files, string? root)
    {
        var focus = new List<VariantGraphFocus>(files.Count);

        if (root is null)
        {
            return focus;
        }

        foreach (var file in files)
        {
            var relative = Path.GetRelativePath(root, file).Replace('\\', '/');

            if (relative is "." or { Length: 0 } || relative.StartsWith("..", StringComparison.Ordinal))
            {
                continue;
            }

            focus.Add(new VariantGraphFocus("local", relative));
        }

        return focus;
    }

    /// <summary>What the work says about the local entries of a reading: the drafts it holds.</summary>
    /// <remarks>
    /// A draft is not in the index, so it is never read from there: a local entry whose file holds a
    /// change is one, and the state the reading gives says which of the two kinds it is, a track that
    /// continues the file's record, or a change that would fork it. A Vault's newer version of the same
    /// path is carried beside the draft so the card can say which version the change should stand on.
    /// </remarks>
    /// <param name="graph">The reading.</param>
    /// <param name="status">What the work says about its files.</param>
    /// <param name="root">Where the Workspace is rooted.</param>
    /// <returns>The drafts, one per changed local entry.</returns>
    private static List<DraftFact> DraftsOf(RolaVariantGraph graph, RolaStatus status, string root)
    {
        var numbers = new Dictionary<string, ulong>(StringComparer.Ordinal);
        var variants = new Dictionary<string, string>(StringComparer.Ordinal);

        foreach (var version in graph.Versions)
        {
            if (version.IsRoot)
            {
                continue;
            }

            variants[version.Hash] = version.Variant;

            if (version.Number is { } number)
            {
                numbers[version.Variant] = number;
            }
        }

        var byPath = graph
            .Entries.Where(entry => entry.Version is not null)
            .GroupBy(entry => VariantGraphDrawing.Normalize(entry.Path), StringComparer.Ordinal)
            .ToDictionary(group => group.Key, group => group.ToArray(), StringComparer.Ordinal);

        var drafts = new List<DraftFact>();

        foreach (var entry in graph.Entries)
        {
            if (
                entry.Side != "local"
                || entry.Version is not { } recorded
                || entry.Deprecated
                || !byPath.TryGetValue(VariantGraphDrawing.Normalize(entry.Path), out var beside)
            )
            {
                continue;
            }

            var absolute = Path.Combine(root, entry.Path);
            PathState state;

            try
            {
                state = status.StateOf(absolute);
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                continue;
            }

            var edited =
                state is PathState.Modified or PathState.Untagged
                || (
                    state is PathState.Merging
                    && status.Modified.Any(named => StatusReadings.Names(status.Root, named, absolute))
                );

            if (!edited && state is not PathState.Wrong)
            {
                continue;
            }

            var unowned =
                state is PathState.Wrong
                && status.Unowned.Any(named => StatusReadings.Names(status.Root, named, absolute));
            var wrong = state is PathState.Wrong;
            var should = ShouldBe(
                beside,
                recorded,
                variants,
                numbers
            );

            drafts.Add(
                new DraftFact(
                    VariantGraphDrawing.Normalize(entry.Path),
                    recorded,
                    Wrong: wrong,
                    Unowned: unowned,
                    ShouldBeVersionHash: should
                )
            );
        }

        return drafts;
    }

    /// <summary>The newer version a draft should stand on, when a Vault's copy names one.</summary>
    /// <param name="beside">The entries at the same path, on every side.</param>
    /// <param name="recorded">The version the local entry records.</param>
    /// <param name="variants">The variant each version fixes.</param>
    /// <param name="numbers">The number each variant has.</param>
    /// <returns>The version, or nothing when no Vault's copy is ahead of the record.</returns>
    private static string? ShouldBe(
        IReadOnlyList<VariantGraphEntry> beside,
        string recorded,
        IReadOnlyDictionary<string, string> variants,
        IReadOnlyDictionary<string, ulong> numbers
    )
    {
        var mine = variants.TryGetValue(recorded, out var local)
            && numbers.TryGetValue(local, out var number)
                ? number
                : (ulong?)null;

        if (mine is null)
        {
            return null;
        }

        string? should = null;
        var best = mine.Value;

        foreach (var entry in beside)
        {
            if (
                entry.Side == "local"
                || entry.Version is not { } version
                || !variants.TryGetValue(version, out var variant)
                || !numbers.TryGetValue(variant, out var other)
                || other <= best
            )
            {
                continue;
            }

            best = other;
            should = version;
        }

        return should;
    }

    /// <summary>
    /// Says whether hovering a thing says what it is.
    /// </summary>
    /// <param name="show">Whether the hover table is shown.</param>
    internal void ShowInfo(bool show)
    {
        if (show == _showInfo)
        {
            return;
        }

        _showInfo = show;
        RedrawInfo();
        InvalidateVisual();
    }

    /// <summary>Says that the table over the switch has to be drawn again.</summary>
    /// <remarks>
    /// The table is a control of its own so that it can be drawn above the switch, and a control is
    /// only drawn when it is told something it shows has changed: the graph's own redraw does not
    /// reach it.
    /// </remarks>
    private void RedrawInfo() => _infoOverlay?.InvalidateVisual();

    /// <summary>Says one thing in place of the graph, or nothing while a read is going.</summary>
    /// <param name="said">What to say, or nothing to say nothing.</param>
    private void ShowMessage(string? said)
    {
        _drawing = null;
        _emptyText = said;
        _hover = null;
        _glide.Stop();
        UpdateCursor();
        RedrawInfo();
        InvalidateVisual();
        TrackableChanged?.Invoke(this, EventArgs.Empty);
    }

    /// <summary>Puts the whole drawing in sight.</summary>
    private void Fit()
    {
        if (_drawing is not { } drawing || Bounds.Width <= 0 || Bounds.Height <= 0)
        {
            return;
        }

        var width = Math.Max(1, drawing.Right - drawing.Left);
        var height = Math.Max(1, drawing.Bottom - drawing.Top);
        var scale = Math.Min(
            (Bounds.Width - (Inset * 2)) / width,
            (Bounds.Height - (Inset * 2)) / height
        );
        var zoom = Math.Clamp(scale, MinZoom, 1.4);

        GlideTo(
            zoom,
            (Bounds.Width / 2) - (((drawing.Left + drawing.Right) / 2) * zoom),
            (Bounds.Height / 2) - (((drawing.Top + drawing.Bottom) / 2) * zoom)
        );
        _fitted = _subject;
    }

    /// <summary>Puts the drawing back at 100%, at its top left.</summary>
    private void ResetView()
    {
        _hover = null;
        UpdateCursor();
        RedrawInfo();
        GlideTo(1, Inset, Inset);
    }

    /// <summary>Eases the view towards a scale and a place rather than snapping to them.</summary>
    /// <remarks>
    /// The drawing is read while it moves, so a jump is a redraw the eye cannot follow: a wheel notch,
    /// a framing and a node brought to the middle all set a target, and the frames between are what
    /// make the reading continuous.
    /// </remarks>
    /// <param name="zoom">The scale to reach.</param>
    /// <param name="panX">Where the drawing's left is to stand.</param>
    /// <param name="panY">Where its top is to stand.</param>
    private void GlideTo(double zoom, double panX, double panY)
    {
        _targetZoom = Math.Clamp(zoom, MinZoom, MaxZoom);
        _targetPanX = panX;
        _targetPanY = panY;
        ClampTarget();

        if (!_glide.IsEnabled)
        {
            _glide.Start();
        }

        InvalidateVisual();
    }

    /// <summary>Stops a glide where the view stands, so a drag is the pointer's and nothing else's.</summary>
    private void StopGlide()
    {
        _glide.Stop();
        _targetZoom = _zoom;
        _targetPanX = _panX;
        _targetPanY = _panY;
    }

    /// <summary>Takes one frame of the glide towards the target.</summary>
    /// <param name="sender">What raised the frame.</param>
    /// <param name="e">The frame's arguments.</param>
    private void Glided(object? sender, EventArgs e)
    {
        var zoom = _targetZoom - _zoom;
        var panX = _targetPanX - _panX;
        var panY = _targetPanY - _panY;

        if (Math.Abs(zoom) < 0.0005 && Math.Abs(panX) < 0.4 && Math.Abs(panY) < 0.4)
        {
            _zoom = _targetZoom;
            _panX = _targetPanX;
            _panY = _targetPanY;
            _glide.Stop();
            ClampPan();
            UpdateZoomText();
            InvalidateVisual();

            return;
        }

        _zoom += zoom * GlidePart;
        _panX += panX * GlidePart;
        _panY += panY * GlidePart;
        ClampPan();
        UpdateZoomText();
        InvalidateVisual();
    }

    /// <summary>Says the scale the drawing stands at now, as the corner reads it.</summary>
    private void UpdateZoomText() =>
        _zoomText = $"{Math.Round(_zoom * 100, MidpointRounding.AwayFromZero)}%";

    /// <summary>Keeps the place a glide is heading for inside the ground it may wander.</summary>
    private void ClampTarget()
    {
        if (_drawing is not { } drawing)
        {
            return;
        }

        var width = Math.Max(1, (drawing.Right - drawing.Left) * _targetZoom);
        var height = Math.Max(1, (drawing.Bottom - drawing.Top) * _targetZoom);
        var slackX = Math.Max(width, Bounds.Width) * PanSlack;
        var slackY = Math.Max(height, Bounds.Height) * PanSlack;

        _targetPanX = Math.Clamp(_targetPanX, Inset - slackX, Inset + slackX);
        _targetPanY = Math.Clamp(_targetPanY, Inset - slackY, Inset + slackY);
    }

    /// <summary>Keeps the drawing inside the ground it is allowed to wander.</summary>
    /// <remarks>
    /// The drawing rests where it was put and may be moved two of its own sizes to either side, so
    /// where it can be found is five of them across and five down. It is a boundary rather than an
    /// anchor: the graph may be moved out of sight, and <c>F</c> is what brings it back.
    /// </remarks>
    private void ClampPan()
    {
        if (_drawing is not { } drawing)
        {
            return;
        }

        var width = Math.Max(1, (drawing.Right - drawing.Left) * _zoom);
        var height = Math.Max(1, (drawing.Bottom - drawing.Top) * _zoom);
        var slackX = Math.Max(width, Bounds.Width) * PanSlack;
        var slackY = Math.Max(height, Bounds.Height) * PanSlack;

        _panX = Math.Clamp(_panX, Inset - slackX, Inset + slackX);
        _panY = Math.Clamp(_panY, Inset - slackY, Inset + slackY);
    }

    /// <summary>Draws the pointer as the surface it is over.</summary>
    private void UpdateCursor() =>
        Cursor = _drawing is null
            ? Cursor.Default
            : _hover is null
                ? new Cursor(StandardCursorType.SizeAll)
                : new Cursor(StandardCursorType.Hand);

    /// <inheritdoc />
    protected override void OnKeyDown(KeyEventArgs e)
    {
        if (e.Key == Key.F && e.KeyModifiers == KeyModifiers.None)
        {
            Fit();
            InvalidateVisual();
            e.Handled = true;

            return;
        }

        if (e.Key == Key.Escape && _hover is not null)
        {
            _hover = null;
            UpdateCursor();
            RedrawInfo();
            InvalidateVisual();
            e.Handled = true;

            return;
        }

        base.OnKeyDown(e);
    }

    /// <inheritdoc />
    protected override void OnPointerPressed(PointerPressedEventArgs e)
    {
        // The graph is what the keyboard's own commands are about once it is pressed.
        Focus();

        var point = e.GetCurrentPoint(this);
        var at = e.GetPosition(this);

        if (point.Properties.IsRightButtonPressed)
        {
            if (HitTest(at) is { } menuTarget)
            {
                ShowMenu(menuTarget);
                e.Handled = true;
            }

            return;
        }

        if (!point.Properties.IsLeftButtonPressed)
        {
            return;
        }

        if (e.ClickCount == 2)
        {
            ResetView();
            e.Handled = true;

            return;
        }

        if (_drawing is null)
        {
            return;
        }

        var hit = HitTest(at);

        if (hit is not null)
        {
            // A press on a thing is not a pan: the cards are what the pointer reads, and a pill naming
            // a local entry is also the way to the file it names. A press on the card itself reads it:
            // the node is brought to the middle, and the drawing is enlarged only when it reads small.
            if (hit.Part is HoverPart.EntryPill or HoverPart.FilePill)
            {
                RevealOnClick(hit);
            }
            else if (hit.Card is { } card)
            {
                FocusCard(card);
            }

            e.Handled = true;

            return;
        }

        StopGlide();
        _dragging = true;
        _moved = false;
        _pressed = at;
        _from = at;
        e.Pointer.Capture(this);
        e.Handled = true;
    }

    /// <summary>Brings one node into the middle of the view, and enlarges a drawing that reads small.</summary>
    /// <remarks>
    /// The enlargement stops at <see cref="FocusZoom"/>: a press is a way to read one node at a size that
    /// is comfortable, not a command to undo a scale the reader chose. A drawing already larger than that
    /// keeps its scale and only moves the node into the middle.
    /// </remarks>
    /// <param name="card">The node to bring.</param>
    private void FocusCard(GraphCard card)
    {
        var zoom = _zoom < FocusZoom ? FocusZoom : _zoom;

        GlideTo(
            zoom,
            (Bounds.Width / 2) - (card.X * zoom),
            (Bounds.Height / 2) - (card.Y * zoom)
        );
    }

    /// <summary>Takes the choice away, which is a click on the ground with no file chosen any more.</summary>
    private void CancelSelection()
    {
        _hover = null;
        _showFocus = false;
        UpdateCursor();
        ChosenFiles.Clear();
        ChosenEntry.Clear();
        RedrawInfo();
        InvalidateVisual();
    }

    /// <summary>Asks the browser to show the file a pressed pill names, when it names one of the work's.</summary>
    /// <param name="hover">What was pressed.</param>
    private void RevealOnClick(Hover hover)
    {
        if (hover.Card is not { } card || _root is not { Length: > 0 } root)
        {
            return;
        }

        string? path = hover.Part switch
        {
            HoverPart.EntryPill when hover.Chip?.Entry is { Side: "local" } entry
                => Path.Combine(root, entry.Path),
            HoverPart.FilePill when card.Draft is { } draft => Path.Combine(root, draft.Path),
            _ => null,
        };

        if (path is null)
        {
            return;
        }

        _windows.Reveal(
            new DockRequest(
                global::FileSystemPlugin.FileSystemPlugin.DirectoryDockNameId,
                path
            )
        );
    }

    /// <inheritdoc />
    protected override void OnPointerMoved(PointerEventArgs e)
    {
        _pointer = e.GetPosition(this);

        if (_dragging)
        {
            var movedBy = Math.Abs(_pointer.X - _pressed.X) + Math.Abs(_pointer.Y - _pressed.Y);
            _moved |= movedBy > 4;
            _panX += _pointer.X - _from.X;
            _panY += _pointer.Y - _from.Y;
            _from = _pointer;
            _targetPanX = _panX;
            _targetPanY = _panY;
            ClampPan();
            InvalidateVisual();
            e.Handled = true;

            return;
        }

        if (_drawing is null)
        {
            return;
        }

        var next = HitTest(_pointer);

        if (SameHover(next, _hover))
        {
            return;
        }

        _hover = next;
        UpdateCursor();
        RedrawInfo();
        InvalidateVisual();
    }

    /// <inheritdoc />
    protected override void OnPointerReleased(PointerReleasedEventArgs e)
    {
        if (!_dragging)
        {
            return;
        }

        var click = !_moved;

        _dragging = false;
        _moved = false;
        e.Pointer.Capture(null);
        e.Handled = true;

        if (click)
        {
            CancelSelection();
        }
    }

    /// <inheritdoc />
    protected override void OnPointerCaptureLost(PointerCaptureLostEventArgs e) =>
        _dragging = false;

    /// <inheritdoc />
    protected override void OnPointerExited(PointerEventArgs e)
    {
        if (_hover is null)
        {
            return;
        }

        _hover = null;
        UpdateCursor();
        RedrawInfo();
        InvalidateVisual();
    }

    /// <inheritdoc />
    protected override void OnPointerWheelChanged(PointerWheelEventArgs e)
    {
        if (_drawing is null)
        {
            return;
        }

        var at = e.GetPosition(this);
        var next = Math.Clamp(
            _targetZoom * Math.Pow(ZoomStep, e.Delta.Y),
            MinZoom,
            MaxZoom
        );

        if (Math.Abs(next - _targetZoom) < double.Epsilon)
        {
            return;
        }

        // The drawing is in content units, so the point under the pointer stays under it when the
        // pan compensates for the new scale. The target is read rather than the frame in hand, so a
        // wheel turned while a glide is on adds to where the view is going.
        var contentX = (at.X - _targetPanX) / _targetZoom;
        var contentY = (at.Y - _targetPanY) / _targetZoom;

        GlideTo(next, at.X - (contentX * next), at.Y - (contentY * next));
        e.Handled = true;
    }

    /// <summary>Whether two hover answers are the same thing.</summary>
    /// <param name="left">One answer.</param>
    /// <param name="right">The other.</param>
    /// <returns>Whether they name the same card, part and pill.</returns>
    private static bool SameHover(Hover? left, Hover? right) =>
        left?.Card?.Index == right?.Card?.Index
        && left?.Part == right?.Part
        && ReferenceEquals(left?.Chip, right?.Chip)
        && left?.GhostIndex == right?.GhostIndex;
}
