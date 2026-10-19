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
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// The Version Graph dock: the chain of the file clicked most recently in a directory listing.
/// </summary>
/// <remarks>
/// The drawing is the desktop's own geometry, not the command line's character grid: versions on one
/// vertical axis, the variants that produced them on another to their left, and the variants merged
/// in further left still. A file with no history is said in words rather than drawn as an empty
/// graph, and a directory click is not an answer here at all (see <see cref="ChosenEntry"/>). A choice
/// of several files is not a chain either, so what is said then is that the graph is hidden while the
/// row under it acts on the choice.
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
    /// <param name="statuses">What the work says about the file, for the edit it may be standing at.</param>
    /// <param name="sync">What a sync would carry, which a fetch is not made over.</param>
    /// <param name="fetch">The one fetch of the Vault's Layout, which the header's Refresh makes.</param>
    /// <param name="hub">What every command this plugin runs reports itself to, which a retrack joins.</param>
    public VersionGraphDock(
        IPluginHost host,
        StatusReadings statuses,
        SyncWatcher sync,
        VaultFetch fetch,
        ProgressHub hub
    )
    {
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
            hub
        );

        // The row under the graph is always there: the graph is about one file, and a reader who has
        // just changed it has nowhere else to say what the change was. What the row's button may do is
        // the work's answer, which is why the two share one dock rather than living apart.
        var stage = new Grid { Children = { graph } };
        var bar = new VersionGraphTrackBar(host, graph, statuses, hub);
        var toggle = UnjoinedToggle(host, graph);

        stage.Children.Add(toggle);

        var root = new Grid { RowDefinitions = new RowDefinitions("*,Auto") };

        root.Children.Add(stage);
        Grid.SetRow(bar, 1);
        root.Children.Add(bar);

        _graph = graph;
        _view = root;
    }

    /// <summary>
    /// The switch in the graph's corner: whether the variants the chain draws nowhere are shown.
    /// </summary>
    /// <remarks>
    /// It is drawn over the graph rather than in the dock's strip because it is about the drawing: a
    /// reader looking at the chain is looking there for what is beside it. What it shows is asked of
    /// the command line, since the graph's own reading reaches only what a link reaches.
    /// </remarks>
    /// <param name="host">The host, whose pictures the switch is drawn with.</param>
    /// <param name="graph">The graph it turns the unjoined variants on and off for.</param>
    /// <returns>The switch.</returns>
    private static ToggleButton UnjoinedToggle(IPluginHost host, VersionGraphView graph)
    {
        var toggle = new ToggleButton
        {
            Classes = { "tool" },
            HorizontalAlignment = HorizontalAlignment.Right,
            VerticalAlignment = VerticalAlignment.Top,
            Margin = new Thickness(0, 12, 12, 0),
            Padding = new Thickness(6),
        };

        Border? mark = null;

        if (host.Icons.Find(Icons.CallSplit) is { } picture)
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

        ToolTip.SetTip(
            toggle,
            RolaI18N.Get("rorolala_vcs.version_graph.show_unjoined")
        );

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

            graph.ShowUnjoined(on);
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
/// The graph itself: the chain laid out as geometry, and the pointer's zoom and drag.
/// </summary>
/// <remarks>
/// The unit states here are content units: every position is multiplied by the zoom as it is drawn,
/// so the shape holds at every scale. What is measured once per zoom and look — labels, bubbles,
/// capsule widths — is kept in <see cref="Metrics"/>; panning draws that layout again rather than
/// laying the text out again.
/// </remarks>
internal sealed partial class VersionGraphView : Control
{
    /// <summary>The least and the most the wheel can scale the drawing.</summary>
    private const double MinZoom = 0.25;

    /// <inheritdoc cref="MinZoom" />
    private const double MaxZoom = 3;

    /// <summary>What one wheel notch scales by.</summary>
    private const double ZoomStep = 1.15;

    /// <summary>The room the drawing keeps from the edge of the dock.</summary>
    private const double Inset = 16;

    /// <summary>How wide a version's bubble is allowed to become before its message wraps.</summary>
    private const double BubbleMaxWidth = 420;

    /// <summary>The room a bubble keeps inside itself.</summary>
    private const double BubblePaddingX = 10;

    /// <inheritdoc cref="BubblePaddingX" />
    private const double BubblePaddingY = 7;

    /// <summary>The room a creator's pill keeps around the name.</summary>
    private const double CreatorPillPaddingX = 7;

    /// <inheritdoc cref="CreatorPillPaddingX" />
    private const double CreatorPillPaddingY = 3;

    /// <summary>How tall a line of a bubble's message is.</summary>
    private const double BubbleLineHeight = 18;

    /// <summary>How tall the hash line is.</summary>
    private const double BubbleHashLineHeight = 14;

    /// <summary>How wide a variant's hover bubble is allowed to become.</summary>
    private const double TooltipMaxWidth = 280;

    /// <summary>The face the hashes and labels are set in.</summary>
    private static readonly FontFamily Mono = new(
        "JetBrains Mono, Cascadia Mono, Consolas, DejaVu Sans Mono, Liberation Mono, monospace"
    );

    /// <summary>The faces the drawing uses, made once so a render does not shape a typeface again.</summary>
    private static readonly Typeface MonoMedium = new(
        Mono,
        FontStyle.Normal,
        FontWeight.Medium
    );

    private static readonly Typeface FaceNormal = new(FontFamily.Default);
    private static readonly Typeface FaceMedium = new(
        FontFamily.Default,
        FontStyle.Normal,
        FontWeight.Medium
    );
    private static readonly Typeface FaceSemiBold = new(
        FontFamily.Default,
        FontStyle.Normal,
        FontWeight.SemiBold
    );

    /// <summary>The cursor a blank part of the graph wears: the view can be dragged from anywhere.</summary>
    private static readonly Cursor PanCursor = new(StandardCursorType.SizeAll);

    /// <summary>The cursor a node wears.</summary>
    private static readonly Cursor NodeCursor = new(StandardCursorType.Hand);

    /// <summary>Where a reading that failed is said.</summary>
    private readonly ILog _log;

    /// <summary>What says that the files may have changed, so that the chain is read again.</summary>
    private readonly IFileChanges _files;

    /// <summary>The pictures a node's menu is drawn with.</summary>
    private readonly IIconLibrary _icons;

    /// <summary>The colours the program is drawn in, answered afresh each time they are read.</summary>
    private readonly IPalette _palette;

    /// <summary>What the work says about the file on screen, for the edit it may be standing at.</summary>
    private readonly StatusReadings _statuses;

    /// <summary>The host's windows being come back to, which the graph is read again for.</summary>
    private readonly IRefocus _refocus;

    /// <summary>What a sync would carry, which is the run a fetch would wait for.</summary>
    private readonly SyncWatcher _sync;

    /// <summary>The one fetch of the Vault's Layout, which the header's Refresh makes.</summary>
    private readonly VaultFetch _fetch;

    /// <summary>Where a question is put to the user, and a run that failed is reported.</summary>
    private readonly IDialogs _dialogs;

    /// <summary>What every command this plugin runs reports itself to, which a retrack's card is begun in.</summary>
    private readonly ProgressHub _hub;

    /// <summary>The chain on screen, or nothing while a read is going or when there is none.</summary>
    private VersionGraphDrawing? _drawing;

    /// <summary>What is said in place of a drawing, or nothing while a read is going.</summary>
    private string? _emptyText;

    /// <summary>The file the current drawing is of, so a new file resets the pan.</summary>
    private string? _target;

    /// <summary>Whether the variants the chain draws nowhere are shown beside it.</summary>
    private bool _showUnjoined;

    /// <summary>How large the drawing is drawn.</summary>
    private double _zoom = 1;

    /// <summary>How far the drawing has been moved, in screen units.</summary>
    private double _panX = Inset;

    /// <inheritdoc cref="_panX" />
    private double _panY = Inset;

    /// <summary>The scale said in the corner.</summary>
    private string _zoomText = "100%";

    /// <summary>The node the pointer is on, when it is on one.</summary>
    private Hover? _hover;

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

    /// <summary>
    /// What waits for a change to stop being said before reading.
    /// </summary>
    /// <remarks>
    /// A change arrives more than once — the browser says so, whoever made it says so, and both a click
    /// and a touch follow one operation — and every reading walks the tree. A quiet moment turns a burst
    /// into the one read it means, and lets what the browser let go of be gone before the tree is read
    /// again, so the edit drawn is the one the files are in now rather than the one they were in before.
    /// </remarks>
    private readonly DispatcherTimer _settle = new() { Interval = TimeSpan.FromMilliseconds(140) };

    /// <summary>Which look is in force, raised by the theme changing what it publishes.</summary>
    private int _themeVersion;

    /// <summary>The colours in force, until the look changes.</summary>
    private Ink? _ink;

    /// <summary>The look <see cref="_ink"/> was read from.</summary>
    private int _inkVersion = -1;

    /// <summary>The layout at the current zoom and look, until either changes.</summary>
    private Metrics? _metrics;

    /// <summary>The drawing <see cref="_metrics"/> was made from.</summary>
    private VersionGraphDrawing? _metricsDrawing;

    /// <summary>The zoom <see cref="_metrics"/> was made at.</summary>
    private double _metricsZoom;

    /// <summary>The look <see cref="_metrics"/> was made for.</summary>
    private int _metricsTheme = -1;

    /// <summary>Makes the view.</summary>
    /// <param name="log">Where a reading that failed is said.</param>
    /// <param name="files">What says that the files may have changed.</param>
    /// <param name="icons">The pictures a node's menu is drawn with.</param>
    /// <param name="palette">The colours the program is drawn in.</param>
    /// <param name="statuses">What the work says about the file on screen.</param>
    /// <param name="refocus">The host's windows being come back to.</param>
    /// <param name="sync">What a sync would carry.</param>
    /// <param name="fetch">The one fetch of the Vault's Layout.</param>
    /// <param name="dialogs">Where a question is put to the user, and a failure reported.</param>
    /// <param name="hub">What every command this plugin runs reports itself to.</param>
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
        ProgressHub hub
    )
    {
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
        _settle.Tick += Settled;
        Focusable = true;
        ClipToBounds = true;
        Cursor = Cursor.Default;
        _emptyText = RolaI18N.Get("rorolala_vcs.version_graph.no_file");
    }

    /// <summary>
    /// Raised when the graph's answer about what may be tracked changes: the file, whether it is an
    /// edit, whether the chains have parted, or that a choice of several files has hidden the graph.
    /// </summary>
    /// <remarks>
    /// The row under the graph draws a message and one button, and what the button may do is read from
    /// the work rather than from the drawing; this is the view saying that its own reading of the work
    /// has changed, so that the row asks the work again rather than waiting for a click it would not
    /// be told about.
    /// </remarks>
    internal event EventHandler? TrackableChanged;

    /// <inheritdoc />
    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs e)
    {
        base.OnAttachedToVisualTree(e);

        // Listening while the view is on screen rather than for as long as it exists: the catalogue
        // outlives every dock, and a dock that was closed has no reason to be told what was clicked.
        ChosenEntry.Changed += Refresh;
        ChosenFiles.Changed += Refresh;
        _files.Changed += Refresh;
        _refocus.Regained += Refresh;
        ResourcesChanged += ResourcesChangedHandler;
        ActualThemeVariantChanged += ThemeVariantChangedHandler;
        Refresh();
    }

    /// <inheritdoc />
    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        ChosenEntry.Changed -= Refresh;
        ChosenFiles.Changed -= Refresh;
        _files.Changed -= Refresh;
        _refocus.Regained -= Refresh;
        ResourcesChanged -= ResourcesChangedHandler;
        ActualThemeVariantChanged -= ThemeVariantChangedHandler;

        // A read that was in flight is no longer this view's to draw.
        _generation++;
        _again = false;
        _settle.Stop();
        base.OnDetachedFromVisualTree(e);
    }

    /// <summary>What the look publishes changed, so the drawing is read again when next drawn.</summary>
    private void ResourcesChangedHandler(object? sender, ResourcesChangedEventArgs e) =>
        Retheme();

    /// <summary>The variant changed, which is a look change like any other.</summary>
    private void ThemeVariantChangedHandler(object? sender, EventArgs e) => Retheme();

    /// <summary>Drops what the old look measured and draws again.</summary>
    private void Retheme()
    {
        _themeVersion++;
        _ink = null;
        _metrics = null;
        InvalidateVisual();
    }

    /// <inheritdoc />
    protected override Size MeasureOverride(Size availableSize) =>
        new(
            double.IsInfinity(availableSize.Width) ? 640 : availableSize.Width,
            double.IsInfinity(availableSize.Height) ? 480 : availableSize.Height
        );

    /// <summary>
    /// Fetches the Vault's Layout under the file's work, then reads the graph again, which is what the dock's
    /// own Refresh asks for.
    /// </summary>
    /// <remarks>
    /// A graph's marked version and its edit are read from the copy the Vault's Layout is kept in, so a graph
    /// made before a fetch would say where the Vault stood when the copy was last brought over. A directory no
    /// Workspace holds has nothing to fetch; a sync is what brings the same copy, so its end is what reads the
    /// graph then, rather than a second exchange made over it.
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
    /// <remarks>
    /// One operation says so more than once — the browser, whoever made the change, and a click beside a
    /// touch — and every reading walks the tree and the index. Waiting for the saying to stop turns a
    /// burst into the one read it means, and lets the browser let go of its own readings first, so the
    /// edit drawn is the one the files are in now rather than the one before the change.
    /// </remarks>
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
    /// Says whether the variants the chain draws nowhere are to be shown beside it.
    /// </summary>
    /// <remarks>
    /// They are not part of the chain and are not read from the index here: the command line is asked
    /// for them, since a variant nothing points at is reached from no link in the chain. Turning the
    /// switch over is a reading again, which is what puts them in the drawing or takes them out.
    /// </remarks>
    /// <param name="show">Whether to show them.</param>
    internal void ShowUnjoined(bool show)
    {
        if (show == _showUnjoined)
        {
            return;
        }

        _showUnjoined = show;
        Refresh();
    }

    /// <summary>Reads what <see cref="Refresh"/> said to wait for.</summary>
    private void Settled(object? sender, EventArgs e)
    {
        _settle.Stop();
        Read();
    }

    /// <summary>The one file the graph is of, or nothing when there is none or when several are chosen.</summary>
    /// <remarks>
    /// The choice is what the user made most recently, and the last click is what the graph followed before
    /// a choice could hold several files: a choice of one is that file, a choice of none leaves the last
    /// clicked file standing, and a choice of several is no one chain at all.
    /// </remarks>
    /// <returns>The file whose chain the graph is of, or nothing.</returns>
    private static Entry? Subject() =>
        ChosenFiles.Current.Count switch
        {
            0 => ChosenEntry.Current,
            1 => ChosenFiles.Current[0],
            _ => null,
        };

    /// <summary>
    /// Reads the file clicked most recently, and draws what it has.
    /// </summary>
    /// <remarks>
    /// The reading is a walk of the index and is done off the window's thread, because a dock that
    /// stopped drawing while a version chain was read would stop for every click in a listing. One read
    /// is in flight at a time: a change said while one is going is read after it rather than beside it,
    /// since two reads of the same chain would answer the same and discard each other. The generation is
    /// what keeps a read that was overtaken from drawing after its view left the screen.
    /// </remarks>
    private void Read()
    {
        if (_reading)
        {
            _again = true;

            return;
        }

        var chosen = ChosenFiles.Current;
        var entry = chosen.Count switch
        {
            0 => ChosenEntry.Current,
            1 => chosen[0],
            _ => null,
        };
        var directory = entry is null ? null : Path.GetDirectoryName(entry.Path);
        var generation = ++_generation;

        // A choice of several files is not one chain: the drawing is hidden rather than made of whichever
        // file was clicked, and the row under it is what the choice is acted on through.
        if (chosen.Count > 1)
        {
            _target = null;
            ShowMessage(RolaI18N.Get("rorolala_vcs.version_graph.many_files"));

            return;
        }

        if (entry is null || directory is null)
        {
            _target = null;
            ShowMessage(RolaI18N.Get("rorolala_vcs.version_graph.no_file"));

            return;
        }

        // A different file's graph is not kept while that file is read, because it would be a wrong
        // answer for as long as the read takes. The same file's is kept until the newer reading is
        // ready, so a change of state does not blank the graph.
        if (!string.Equals(entry.Path, _target, StringComparison.Ordinal))
        {
            ShowMessage(null);
        }

        _reading = true;
        _readingFor = generation;

        // What the switch says is read here, on the window's thread, so a reading that was already
        // going does not change its mind halfway through the answer it was asked for.
        var showing = _showUnjoined;

        _ = Task.Run(() =>
        {
            RolaVersionGraph? graph = null;
            (string Words, bool Wrong)? editing = null;
            string? merging = null;

            try
            {
                graph = RolaVersionGraph.Locate(directory, entry.Path);
                editing = EditingOf(directory, entry.Path);
                merging = MergingOf(directory, entry.Path);
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                _log.Warn(
                    $"the version history of `{entry.Path}` could not be read: {error.Message}"
                );
            }

            IReadOnlyList<UnjoinedVariant> unjoined = [];

            if (showing && graph is not null)
            {
                var (done, said, found) = UnjoinedCommand.Run(directory, entry.Path);

                if (done)
                {
                    unjoined = found;
                }
                else
                {
                    _log.Warn(
                        $"the unjoined variants of `{entry.Path}` could not be read: {said}"
                    );
                }
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
                    // The view is no longer on screen: the reading is still this thread's to give back.
                    graph?.Dispose();
                }
                else if (graph is null)
                {
                    ShowMessage(
                        RolaI18N.Get("rorolala_vcs.version_graph.no_history")
                    );
                }
                else
                {
                    VersionGraphDrawing drawing;

                    using (graph)
                    {
                        drawing = VersionGraphDrawing.From(graph, editing, unjoined, merging);
                    }

                    var moved = !string.Equals(entry.Path, _target, StringComparison.Ordinal);

                    _target = entry.Path;
                    _drawing = drawing;
                    _emptyText = null;
                    _hover = null;
                    _metrics = null;
                    TrackableChanged?.Invoke(this, EventArgs.Empty);

                    if (moved)
                    {
                        _panX = Inset;
                        _panY = Inset;
                    }

                    UpdateCursor();
                    InvalidateVisual();
                }

                if (current && _again)
                {
                    _again = false;
                    Read();
                }
            });
        });
    }

    /// <summary>
    /// What the working file's unrecorded change is, or nothing when its content stands as recorded.
    /// </summary>
    /// <remarks>
    /// Only a change of content is an edit: a file the Layout does not name and one that moved are each
    /// about something other than being worked on, and each has a mark of its own elsewhere. A change
    /// that must not be recorded is still an edit, and is answered with its own reason so that the graph
    /// says what the listing does about the same file.
    /// <para>
    /// A merge waiting on the file does not hide a change made under it: the state answers the merge
    /// before the change, so the change is read from the reading's own list. What is drawn then is the
    /// edit, marked as work under way — the merge and the change are two things about one file and the
    /// graph shows both.
    /// </para>
    /// </remarks>
    /// <param name="directory">The directory holding the file, which is what a work is read for.</param>
    /// <param name="path">The file, as the program names it.</param>
    /// <returns>The state's words and whether it is a change that must not be recorded.</returns>
    private (string Words, bool Wrong)? EditingOf(string directory, string path)
    {
        if (_statuses.Reading(directory) is not { } status)
        {
            return null;
        }

        try
        {
            var state = status.StateOf(path);

            if (
                state is PathState.Merging
                && status.Modified.Any(named =>
                    StatusReadings.Names(status.Root, named, path)
                )
            )
            {
                return (RorolalaStates.Words(PathState.Modified), false);
            }

            return state switch
            {
                PathState.Modified => (
                    RorolalaStates.Words(PathState.Modified),
                    false
                ),
                PathState.Wrong => (RorolalaStates.Wrong(status, path), true),
                _ => null,
            };
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Warn($"how `{path}` stands could not be read: {error.Message}");

            return null;
        }
    }

    /// <summary>The variant waiting to be joined into the file at `path`, when one is.</summary>
    /// <remarks>
    /// It is read from the same status reading the edit is, so the menu and the drawing agree about
    /// whether a merge is in progress: the command refuses a join while one is, and a second join item
    /// would be an item that only fails.
    /// </remarks>
    /// <param name="directory">The directory holding the file, which is what a work is read for.</param>
    /// <param name="path">The file, as the program names it.</param>
    /// <returns>The pending variant's hash, or nothing when the file is not in a merge.</returns>
    private string? MergingOf(string directory, string path) =>
        MergeOf(directory, path)?.Merge.Variant;

    /// <summary>The merge waiting on the file at `path`, and the Workspace it was read in.</summary>
    /// <remarks>
    /// The Workspace's root is handed over with the record because the record names the variant file
    /// relative to it, and the two are read together: a root kept in a field would be the root of
    /// whichever file was read last.
    /// </remarks>
    /// <param name="directory">The directory holding the file, which is what a work is read for.</param>
    /// <param name="path">The file, as the program names it.</param>
    /// <returns>The merge and the Workspace's root, or nothing when the file is not in a merge.</returns>
    private (Merge Merge, string Root)? MergeOf(string directory, string path)
    {
        if (_statuses.Reading(directory) is not { } status)
        {
            return null;
        }

        try
        {
            foreach (var merge in status.Merging)
            {
                if (StatusReadings.Names(status.Root, merge.Target, path))
                {
                    return (merge, status.Root);
                }
            }
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Warn($"whether `{path}` is being merged could not be read: {error.Message}");
        }

        return null;
    }

    /// <summary>The path a place in a merge record names, when it lies inside the work.</summary>
    /// <remarks>
    /// The file is removed to end a merge, so a place that does not resolve under the Workspace is
    /// answered with nothing rather than with a path: the record is the command line's to hold, and a
    /// place that climbs out of the work is not a file this may delete.
    /// </remarks>
    /// <param name="root">Where the Workspace is rooted.</param>
    /// <param name="place">The place, relative to the Workspace's root.</param>
    /// <returns>The absolute path, or nothing when the place is not inside the Workspace.</returns>
    private static string? VariantPath(string root, string place)
    {
        var work = Path.GetFullPath(root);
        var disk = Path.GetFullPath(Path.Combine(work, place));
        var relative = Path.GetRelativePath(work, disk);

        return relative == ".."
            || relative.StartsWith(
                ".." + Path.DirectorySeparatorChar,
                StringComparison.Ordinal
            )
            || Path.IsPathRooted(relative)
            ? null
            : disk;
    }

    /// <summary>Says one thing in place of the graph, or nothing while a read is going.</summary>
    /// <param name="said">What to say, or nothing to say nothing.</param>
    private void ShowMessage(string? said)
    {
        _drawing = null;
        _emptyText = said;
        _hover = null;
        _metrics = null;
        UpdateCursor();
        InvalidateVisual();
        TrackableChanged?.Invoke(this, EventArgs.Empty);
    }

    /// <summary>Puts the drawing back at the top left, and at 100% when asked.</summary>
    /// <param name="resetZoom">Whether the scale is reset too.</param>
    private void ResetView(bool resetZoom)
    {
        if (resetZoom)
        {
            _zoom = 1;
            _zoomText = "100%";
            _metrics = null;
        }

        _panX = Inset;
        _panY = Inset;
        _hover = null;
        UpdateCursor();
        InvalidateVisual();
    }

    /// <summary>Draws the pointer as the surface it is over.</summary>
    private void UpdateCursor() =>
        Cursor = _drawing is null
            ? Cursor.Default
            : _hover is null
                ? PanCursor
                : NodeCursor;

    /// <inheritdoc />
    protected override void OnKeyDown(KeyEventArgs e)
    {
        if (e.Key == Key.F && e.KeyModifiers == KeyModifiers.None)
        {
            FocusCurrent();
            e.Handled = true;

            return;
        }

        base.OnKeyDown(e);
    }

    /// <summary>Puts the graph at 100% around the version it marks as the current one.</summary>
    /// <remarks>
    /// The marked version is the one the tracked Vault records, when a Vault is tracked; otherwise the
    /// current version is the highest one. Centring it is what makes <c>F</c> a way back to the thing the
    /// graph is about, however far the view has been panned or zoomed away from it.
    /// </remarks>
    private void FocusCurrent()
    {
        if (_drawing is null || _drawing.Versions.Count == 0)
        {
            return;
        }

        var version = _drawing.Versions.FirstOrDefault(one => one.Origin)
            ?? _drawing.Versions[0];

        _zoom = 1;
        _zoomText = "100%";
        _metrics = null;
        _hover = null;
        _panX = Bounds.Width / 2 - version.X;
        _panY = Bounds.Height / 2 - version.Y;

        var metrics = CurrentMetrics();

        ClampPan(metrics.MaxRight + Inset * _zoom, _drawing.Height * _zoom);
        UpdateCursor();
        InvalidateVisual();
    }

    /// <inheritdoc />
    protected override void OnPointerPressed(PointerPressedEventArgs e)
    {
        // The graph is what the keyboard's own commands are about once it is pressed, which is what
        // makes `F` mean this graph rather than whichever control had the focus before.
        Focus();

        var point = e.GetCurrentPoint(this);

        // A press with the right button asks what can be done with the node under it. It is not a
        // pan, and it is not the node's hover: a menu stays open after the pointer leaves.
        if (point.Properties.IsRightButtonPressed)
        {
            if (HitTest(e.GetPosition(this)) is { } hit)
            {
                ShowNodeMenu(hit);
                e.Handled = true;
            }

            return;
        }

        if (!point.Properties.IsLeftButtonPressed)
        {
            return;
        }

        // A double click puts the view back where it started, at 100%.
        if (e.ClickCount == 2)
        {
            ResetView(resetZoom: true);
            e.Handled = true;

            return;
        }

        if (_drawing is null)
        {
            return;
        }

        // A press on a node is not a pan: the nodes are the things hover answers.
        var at = e.GetPosition(this);

        if (HitTest(at) is not null)
        {
            return;
        }

        _dragging = true;
        _from = at;
        e.Pointer.Capture(this);
        e.Handled = true;
    }

    /// <inheritdoc />
    protected override void OnPointerMoved(PointerEventArgs e)
    {
        if (_dragging)
        {
            var at = e.GetPosition(this);

            _panX += at.X - _from.X;
            _panY += at.Y - _from.Y;
            _from = at;
            InvalidateVisual();
            e.Handled = true;

            return;
        }

        if (_drawing is null)
        {
            return;
        }

        var next = HitTest(e.GetPosition(this));

        if (SameHover(next, _hover))
        {
            return;
        }

        _hover = next;
        UpdateCursor();
        InvalidateVisual();
    }

    /// <inheritdoc />
    protected override void OnPointerReleased(PointerReleasedEventArgs e)
    {
        if (!_dragging)
        {
            return;
        }

        _dragging = false;
        e.Pointer.Capture(null);
        e.Handled = true;
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
            _zoom * Math.Pow(ZoomStep, e.Delta.Y),
            MinZoom,
            MaxZoom
        );

        if (Math.Abs(next - _zoom) < double.Epsilon)
        {
            return;
        }

        // The drawing is in content units, so the point under the pointer stays under it when the
        // pan compensates for the new scale.
        var contentX = (at.X - _panX) / _zoom;
        var contentY = (at.Y - _panY) / _zoom;

        _zoom = next;
        _panX = at.X - contentX * _zoom;
        _panY = at.Y - contentY * _zoom;
        _zoomText = $"{Math.Round(_zoom * 100, MidpointRounding.AwayFromZero)}%";
        _metrics = null;
        InvalidateVisual();
        e.Handled = true;
    }

    /// <summary>Offers what can be done with the node under the pointer.</summary>
    /// <remarks>
    /// The hash is always there to copy. A version adds the two retracks: the one that writes the version's
    /// content back over the file as well as moving the record, and the one that moves the record and leaves
    /// the file alone. A variant has neither, because a variant is not a position a record may be put at.
    /// <para>
    /// The edit and the merge are the two things about the working file that may be taken back. The
    /// <c>???</c> node offers dropping the change it stands for; a variant drawn as work under way —
    /// the <c>???</c> itself or the variant waiting to be joined — offers ending the merge, either by
    /// putting the file back as well or by leaving its content in place.
    /// </para>
    /// </remarks>
    /// <param name="node">The node that was right-clicked.</param>
    private void ShowNodeMenu(Hover node)
    {
        var hash =
            node.Version?.Hash
            ?? node.Upstream?.Hash
            ?? node.Variant?.Hash
            ?? string.Empty;
        var key = node.Variant is not null
            ? "rorolala_vcs.version_graph.copy_variant_hash"
            : "rorolala_vcs.version_graph.copy_version_hash";
        var copy = new Avalonia.Controls.MenuItem
        {
            Header = RolaI18N.Get(key),

            // A placeholder stands for something the index does not name, so there is no hash to
            // copy; an item that copied one would put an empty string on the clipboard.
            IsEnabled = hash.Length > 0,
        };
        var menu = new ContextMenu { Placement = PlacementMode.Pointer };

        if (_icons.Find(Icons.ContentCopy) is { } picture)
        {
            copy.Icon = Icons.Inked(picture);
        }

        copy.Click += (_, _) => CopyHash(hash);
        menu.Items.Add(copy);

        // An unjoined variant is one no version points at, and joining it into the file is what makes
        // it a merge waiting to be recorded. A file already in a merge has nowhere for a second one to
        // go, so the item is offered only while it is not.
        if (
            node.Variant is { Unjoined: true } unjoined
            && _target is { Length: > 0 } joining
            && Path.GetDirectoryName(joining) is { Length: > 0 } mergingAt
            && MergingOf(mergingAt, joining) is null
        )
        {
            menu.Items.Add(new Avalonia.Controls.Separator());

            var join = new Avalonia.Controls.MenuItem
            {
                Header = RolaI18N.Get("rorolala_vcs.version_graph.join_variant"),
            };

            if (_icons.Find(Icons.Merge) is { } joinPicture)
            {
                join.Icon = Icons.Inked(joinPicture);
            }

            join.Click += (_, _) => BeginJoin(unjoined.Hash);
            menu.Items.Add(join);
        }

        // A change that was never recorded may be dropped, which is what puts the file back to the
        // version the Layout names. The content is nowhere else, so the item only asks; nothing is
        // written until the question is answered.
        if (
            node.Variant is { State: not null }
            && _target is { Length: > 0 } edited
            && Path.GetDirectoryName(edited) is { Length: > 0 } editedAt
        )
        {
            menu.Items.Add(new Avalonia.Controls.Separator());
            menu.Items.Add(DeleteChangesItem(edited, editedAt));
        }

        // A merge waiting on the file is ended by taking its variant file away and then its record,
        // which is the two runs `align --delete` is built from. The plain item also puts the main file
        // back to what the Layout records; the other leaves the work as it stands, and both ask first.
        // The file is taken where the tree holds it, which a move made by hand may have taken away
        // from the record's place.
        if (
            node.Variant is { Merging: true }
            && _target is { Length: > 0 } merged
            && Path.GetDirectoryName(merged) is { Length: > 0 } mergedAt
            && MergeOf(mergedAt, merged) is { } pending
            && VariantPath(pending.Root, VariantFiles.Place(pending.Merge)) is { } actual
            && VariantPath(pending.Root, pending.Merge.Path) is { } recorded
        )
        {
            menu.Items.Add(new Avalonia.Controls.Separator());
            menu.Items.Add(CancelMergeItem(merged, mergedAt, actual, recorded, keep: false));
            menu.Items.Add(CancelMergeItem(merged, mergedAt, actual, recorded, keep: true));
        }

        // A retrack is about a version the track records, and the file the graph is of is where it is
        // written: a node the index has no hash for is a placeholder, and one with no file on screen
        // has nothing to move. A version only the Vault has may be gone to as well: the command brings
        // its chain here first, when it can, and then moves the record onto it.
        if (_target is not null)
        {
            if (node.Version is { Hash.Length: > 0 } version)
            {
                menu.Items.Add(new Avalonia.Controls.Separator());
                menu.Items.Add(
                    RetrackItem(version.Hash, version.Label, allowJump: false, onlyIndex: false)
                );
                menu.Items.Add(
                    RetrackItem(version.Hash, version.Label, allowJump: false, onlyIndex: true)
                );
            }
            else if (node.Upstream is { Hash.Length: > 0 } upstream)
            {
                var label = LabelOf(upstream);

                menu.Items.Add(new Avalonia.Controls.Separator());
                menu.Items.Add(
                    RetrackItem(upstream.Hash, label, allowJump: true, onlyIndex: false)
                );
                menu.Items.Add(
                    RetrackItem(upstream.Hash, label, allowJump: true, onlyIndex: true)
                );
            }
        }

        menu.Open(this);
    }

    /// <summary>What a retrack item names an upstream version by.</summary>
    /// <param name="version">The Vault's version.</param>
    /// <returns>Its version number, or the head of the hash for one the index does not hold.</returns>
    private static string LabelOf(VersionGraphDrawing.UpstreamNode version) =>
        version.Unknown
            ? version.Hash[..Math.Min(7, version.Hash.Length)]
            : version.Label;

    /// <summary>One of the two retrack items, which differ in whether the file on disk follows the record.</summary>
    /// <param name="hash">The version the record is to be moved to.</param>
    /// <param name="label">What the version is called in the question.</param>
    /// <param name="allowJump">Whether a version off the way back may be gone to.</param>
    /// <param name="onlyIndex">Whether the file on disk is left alone.</param>
    /// <returns>The item, ready to be added to a menu.</returns>
    private Avalonia.Controls.MenuItem RetrackItem(
        string hash,
        string label,
        bool allowJump,
        bool onlyIndex
    )
    {
        var item = new Avalonia.Controls.MenuItem
        {
            Header = RolaI18N.Get(
                onlyIndex
                    ? "rorolala_vcs.version_graph.retrack_only_index"
                    : "rorolala_vcs.version_graph.retrack"
            ),
        };

        if (_icons.Find(onlyIndex ? Icons.ArrowRightAlt : Icons.EditNote) is { } picture)
        {
            item.Icon = Icons.Inked(picture);
        }

        item.Click += (_, _) => AskRetrack(hash, label, allowJump, onlyIndex);

        return item;
    }

    /// <summary>The item that drops the file's unrecorded change.</summary>
    /// <param name="path">The file, as the program names it.</param>
    /// <param name="directory">The directory the runs of the item are made in.</param>
    /// <returns>The item, ready to be added to a menu.</returns>
    private Avalonia.Controls.MenuItem DeleteChangesItem(string path, string directory)
    {
        var item = new Avalonia.Controls.MenuItem
        {
            Header = RolaI18N.Get("rorolala_vcs.version_graph.delete_changes"),
        };

        if (_icons.Find(Icons.Refresh) is { } picture)
        {
            item.Icon = Icons.Inked(picture);
        }

        item.Click += (_, _) => AskDeleteChanges(path, directory);

        return item;
    }

    /// <summary>One of the two items that end a merge, which differ in what the file is left holding.</summary>
    /// <param name="path">The file the merge is waiting on, as the program names it.</param>
    /// <param name="directory">The directory the runs of the item are made in.</param>
    /// <param name="variant">Where the tree holds the variant file.</param>
    /// <param name="recorded">Where the record says the variant file lies.</param>
    /// <param name="keep">Whether the file's content is kept rather than put back.</param>
    /// <returns>The item, ready to be added to a menu.</returns>
    private Avalonia.Controls.MenuItem CancelMergeItem(
        string path,
        string directory,
        string variant,
        string recorded,
        bool keep
    )
    {
        var item = new Avalonia.Controls.MenuItem
        {
            Header = RolaI18N.Get(
                keep
                    ? "rorolala_vcs.version_graph.cancel_merge_keep"
                    : "rorolala_vcs.version_graph.cancel_merge"
            ),
        };

        if (_icons.Find(keep ? Icons.EditNote : Icons.CallSplit) is { } picture)
        {
            item.Icon = Icons.Inked(picture);
        }

        item.Click += (_, _) => AskCancelMerge(path, directory, variant, recorded, keep);

        return item;
    }

    /// <summary>Puts the dropping of an edit to the user, since nothing else holds its content.</summary>
    /// <param name="path">The file, as the program names it.</param>
    /// <param name="directory">The directory the run is made in.</param>
    private void AskDeleteChanges(string path, string directory) =>
        _dialogs.Show(
            new Dialog(
                RolaI18N.Get("rorolala_vcs.version_graph.delete_changes_confirm_title"),
                RolaI18N.Get(
                    "rorolala_vcs.version_graph.delete_changes_confirm_message",
                    Path.GetFileName(path)
                ),
                () => BeginDeleteChanges(path, directory)
            )
        );

    /// <summary>Puts the ending of a merge to the user, with what it leaves the file holding.</summary>
    /// <param name="path">The file the merge is waiting on, as the program names it.</param>
    /// <param name="directory">The directory the runs are made in.</param>
    /// <param name="variant">Where the tree holds the variant file.</param>
    /// <param name="recorded">Where the record says the variant file lies.</param>
    /// <param name="keep">Whether the file's content is kept rather than put back.</param>
    private void AskCancelMerge(
        string path,
        string directory,
        string variant,
        string recorded,
        bool keep
    ) =>
        _dialogs.Show(
            new Dialog(
                RolaI18N.Get("rorolala_vcs.version_graph.cancel_merge_confirm_title"),
                RolaI18N.Get(
                    keep
                        ? "rorolala_vcs.version_graph.cancel_merge_keep_confirm_message"
                        : "rorolala_vcs.version_graph.cancel_merge_confirm_message",
                    Path.GetFileName(path)
                ),
                () => BeginCancelMerge(path, directory, variant, recorded, keep)
            )
        );

    /// <summary>Puts the retrack to the user, asking first only when there is work on disk to lose.</summary>
    /// <remarks>
    /// The record alone moves without a question: nothing the user has is touched. Writing the version's
    /// content back over the file is confirmed only where the file holds a change that was never recorded:
    /// content that already stands as recorded is being replaced by another recorded content, which is what
    /// the reader asked for, and nothing is lost. An agreed question is what lets the run discard the
    /// changes it named — the command refuses to write over them without <c>--force</c>.
    /// </remarks>
    /// <param name="hash">The version the record is to be moved to.</param>
    /// <param name="label">What the version is called in the question.</param>
    /// <param name="allowJump">Whether a version off the way back may be gone to.</param>
    /// <param name="onlyIndex">Whether the file on disk is left alone.</param>
    private void AskRetrack(string hash, string label, bool allowJump, bool onlyIndex)
    {
        if (
            _target is not { Length: > 0 } path
            || Path.GetDirectoryName(path) is not { Length: > 0 } directory
        )
        {
            return;
        }

        if (onlyIndex || !Changed(path))
        {
            BeginRetrack(path, directory, hash, allowJump, onlyIndex, force: false);

            return;
        }

        _dialogs.Show(
            new Dialog(
                RolaI18N.Get("rorolala_vcs.version_graph.retrack_confirm_title"),
                RolaI18N.Get(
                    "rorolala_vcs.version_graph.retrack_confirm_message",
                    Path.GetFileName(path),
                    label
                ),
                () => BeginRetrack(path, directory, hash, allowJump, onlyIndex: false, force: true)
            )
        );
    }

    /// <summary>Whether the working file holds a change that was never recorded.</summary>
    /// <remarks>
    /// The judgement is the one the graph draws its `???` node from: a change of content, and the two states
    /// that answer why one must not be recorded. A file that stands as recorded — moved, perhaps, but holding
    /// the content the Layout agreed with — has nothing to lose to a retrack. A reading that cannot answer is
    /// answered as a change, so that the one thing not known is a question asked rather than a write made.
    /// </remarks>
    /// <param name="path">The file, as the program names it.</param>
    /// <returns>Whether the file's content is more than the record says.</returns>
    private bool Changed(string path)
    {
        if (
            Path.GetDirectoryName(path) is not { Length: > 0 } directory
            || _statuses.Reading(directory) is not { } status
        )
        {
            return true;
        }

        try
        {
            return status.StateOf(path) is PathState.Modified or PathState.Wrong;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Warn($"how `{path}` stands could not be read: {error.Message}");

            return true;
        }
    }

    /// <summary>Runs the restore that drops the file's unrecorded change, and reads again once it is over.</summary>
    /// <remarks>
    /// The run is the command line's and is made off the window's thread: the content of the version the
    /// file is put back to may not be in the local store, and the command reaches for a Vault when it is
    /// not. What it wrote is what every reading is made from, so the files are said to have changed and
    /// the graph reads again.
    /// </remarks>
    /// <param name="path">The file, as the program names it.</param>
    /// <param name="directory">The directory the run is made in.</param>
    private void BeginDeleteChanges(string path, string directory)
    {
        var run = _hub.Begin("rorolala_vcs.progress.restore");

        _ = Task.Run(() =>
        {
            var (done, said) = AlignCommand.Restore(directory, path);

            Dispatcher.UIThread.Post(() =>
            {
                _hub.End(run);
                _files.Touch();

                if (!done)
                {
                    FailureNotice.Show(
                        _dialogs,
                        _icons,
                        RolaI18N.Get("rorolala_vcs.version_graph.delete_changes_failed"),
                        said
                    );

                    return;
                }

                Refresh();
            });
        });
    }

    /// <summary>Ends the merge waiting on the file, putting the file back or leaving what it holds.</summary>
    /// <remarks>
    /// The variant file goes first, because a run that ends a merge finds it by being gone: what `align
    /// --delete` removes is the record, and the file is only how the variant was shown. The plain item
    /// puts the file back to the version the Layout names as well, which is what makes the two items
    /// differ; both are run off the window's thread, since a restore may reach for a Vault.
    /// </remarks>
    /// <param name="path">The file the merge is waiting on, as the program names it.</param>
    /// <param name="directory">The directory the runs are made in.</param>
    /// <param name="variant">Where the tree holds the variant file.</param>
    /// <param name="recorded">Where the record says the variant file lies.</param>
    /// <param name="keep">Whether the file's content is kept rather than put back.</param>
    private void BeginCancelMerge(
        string path,
        string directory,
        string variant,
        string recorded,
        bool keep
    )
    {
        // A file with nothing unrecorded has nothing to put back: a restore of it would be a run that
        // is only refused, and the merge is ended just the same by the run before it.
        var restore = !keep && EditingOf(directory, path) is not null;
        var run = _hub.Begin("rorolala_vcs.progress.unmerge");

        _ = Task.Run(() =>
        {
            var (done, said) = AlignCommand.Unmerge(directory, variant, recorded);

            if (done && restore)
            {
                (done, said) = AlignCommand.Restore(directory, path);
            }

            Dispatcher.UIThread.Post(() =>
            {
                _hub.End(run);
                _files.Touch();

                if (!done)
                {
                    FailureNotice.Show(
                        _dialogs,
                        _icons,
                        RolaI18N.Get("rorolala_vcs.version_graph.cancel_merge_failed"),
                        said
                    );

                    return;
                }

                Refresh();
            });
        });
    }

    /// <summary>Turns an unjoined variant into a merge waiting on the file.</summary>
    /// <remarks>
    /// The run is the command line's <c>checkin</c> in its variant mode, and it is made off the
    /// window's thread: the variant and its content may be asked of a Vault when they are not here.
    /// What it wrote is a waiting merge rather than a version, so every view of the work is told the
    /// files may have changed, and the graph reads again: the variant it names is the one drawn in the
    /// colour of work under way.
    /// </remarks>
    /// <param name="variant">The variant to join into the file on screen.</param>
    private void BeginJoin(string variant)
    {
        if (
            _target is not { Length: > 0 } path
            || Path.GetDirectoryName(path) is not { Length: > 0 } directory
        )
        {
            return;
        }

        var run = _hub.Begin("rorolala_vcs.progress.join");

        _ = Task.Run(() =>
        {
            var (done, said) = JoinCommand.Run(directory, path, variant);

            Dispatcher.UIThread.Post(() =>
            {
                _hub.End(run);
                _files.Touch();

                if (!done)
                {
                    FailureNotice.Show(
                        _dialogs,
                        _icons,
                        RolaI18N.Get("rorolala_vcs.version_graph.join_failed"),
                        said
                    );

                    return;
                }

                Refresh();
            });
        });
    }

    /// <summary>Runs the retrack, and reads the work again once it is over.</summary>
    /// <remarks>
    /// The run is the command line's and is made off the window's thread: it may reach for a Vault when the
    /// version it goes to is not held here, which takes as long as a network takes. What it wrote is what
    /// every reading is made from, so the files are said to have changed, and the Vault's Layout is fetched
    /// as well — the graph's marked version and its edit are read from the copy that fetch brings over.
    /// </remarks>
    /// <param name="path">The file the record is of.</param>
    /// <param name="directory">The directory the run is made in.</param>
    /// <param name="version">The version the record is to be moved to.</param>
    /// <param name="allowJump">Whether a version off the way back may be gone to.</param>
    /// <param name="onlyIndex">Whether the file on disk is left alone.</param>
    /// <param name="force">
    /// Whether unrecorded changes are to be discarded, which is what a reader agreed to in the question.
    /// </param>
    private void BeginRetrack(
        string path,
        string directory,
        string version,
        bool allowJump,
        bool onlyIndex,
        bool force
    )
    {
        var run = _hub.Begin("rorolala_vcs.progress.retrack");

        _ = Task.Run(() =>
        {
            var (done, said) = RetrackCommand.Run(
                directory,
                path,
                version,
                onlyIndex,
                allowJump,
                force
            );

            Dispatcher.UIThread.Post(() =>
            {
                _hub.End(run);
                _files.Touch();

                if (!done)
                {
                    FailureNotice.Show(
                        _dialogs,
                        _icons,
                        RolaI18N.Get("rorolala_vcs.version_graph.retrack_failed"),
                        said
                    );

                    return;
                }

                // The graph is read again for the record that moved, and the Vault's Layout is fetched so that
                // what is read is what the Vault holds now rather than the copy the last fetch brought.
                Refresh();

                // A sync writes the copy a fetch brings and is followed by one of its own, so a fetch made
                // beside it would be two exchanges for one file.
                if (!_sync.Running)
                {
                    _fetch.Fetch(directory, Refresh);
                }
            });
        });
    }

    /// <summary>Puts a hash on the clipboard.</summary>
    /// <param name="hash">The hash to copy.</param>
    private void CopyHash(string hash)
    {
        if (TopLevel.GetTopLevel(this)?.Clipboard is { } clipboard)
        {
            _ = Store(clipboard, hash);
        }
    }

    /// <summary>Writes a hash to a clipboard, saying so when the system refuses it.</summary>
    /// <param name="clipboard">The clipboard to write to.</param>
    /// <param name="hash">The hash to copy.</param>
    private async Task Store(IClipboard clipboard, string hash)
    {
        try
        {
            await clipboard.SetTextAsync(hash);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Warn($"the hash could not be copied: {error.Message}");
        }
    }

    /// <summary>Whether two hover answers are the same node.</summary>
    private static bool SameHover(Hover? left, Hover? right) =>
        ReferenceEquals(left?.Version, right?.Version)
        && ReferenceEquals(left?.Variant, right?.Variant)
        && ReferenceEquals(left?.Upstream, right?.Upstream);

    /// <summary>The node under a point, or nothing.</summary>
    private Hover? HitTest(Point at)
    {
        if (_drawing is null)
        {
            return null;
        }

        foreach (var version in _drawing.Versions)
        {
            var cx = _panX + version.X * _zoom;
            var cy = _panY + version.Y * _zoom;
            var radius = (VersionGraphDrawing.Radius + 5) * _zoom;
            var dx = at.X - cx;
            var dy = at.Y - cy;

            if (dx * dx + dy * dy <= radius * radius)
            {
                return new Hover(version, null, null);
            }
        }

        foreach (var upstream in _drawing.Upstream)
        {
            var cx = _panX + upstream.X * _zoom;
            var cy = _panY + upstream.Y * _zoom;
            var radius = (VersionGraphDrawing.Radius + 5) * _zoom;
            var dx = at.X - cx;
            var dy = at.Y - cy;

            if (dx * dx + dy * dy <= radius * radius)
            {
                return new Hover(null, null, upstream);
            }
        }

        var metrics = CurrentMetrics();

        foreach (var variant in _drawing.Variants)
        {
            var width = metrics.Variants[variant.Index].Width;
            var cx = _panX + variant.X * _zoom;
            var cy = _panY + variant.Y * _zoom;

            if (
                Math.Abs(at.X - cx) <= width / 2 + 3 * _zoom
                && Math.Abs(at.Y - cy)
                    <= (VersionGraphDrawing.CapsuleHeight / 2 + 3) * _zoom
            )
            {
                return new Hover(null, variant, null);
            }
        }

        return null;
    }

    /// <summary>
    /// Keeps the drawing inside the ground it is allowed to wander: five times its own bounds.
    /// </summary>
    /// <remarks>
    /// The drawing rests at the inset and may be moved two of its own widths or heights to either side of
    /// that, so where it can be found is five of them across and five down. It is a boundary rather than an
    /// anchor: the graph may be moved out of sight, and <c>F</c> is what brings it back.
    /// </remarks>
    /// <param name="width">How far to the right the drawing reaches, inset included.</param>
    /// <param name="height">How far down the drawing reaches.</param>
    private void ClampPan(double width, double height)
    {
        var slackX = 2 * Math.Max(width, 1);
        var slackY = 2 * Math.Max(height, 1);

        _panX = Math.Clamp(_panX, Inset - slackX, Inset + slackX);
        _panY = Math.Clamp(_panY, Inset - slackY, Inset + slackY);
    }

    /// <summary>A node the pointer is on.</summary>
    /// <param name="Version">The version it is on, when it is one of the work's own.</param>
    /// <param name="Variant">The variant it is on, when it is a variant.</param>
    /// <param name="Upstream">The Vault's version it is on, when it is one the work's chain does not hold.</param>
    private sealed record Hover(
        VersionGraphDrawing.VersionNode? Version,
        VersionGraphDrawing.VariantNode? Variant,
        VersionGraphDrawing.UpstreamNode? Upstream
    );
}
