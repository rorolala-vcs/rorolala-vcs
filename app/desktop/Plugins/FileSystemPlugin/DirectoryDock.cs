using System.Globalization;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>The Directory dock: one directory's entries, read at a zoom.</summary>
internal sealed class DirectoryDock : IDockView
{
    /// <summary>The control the dock shows.</summary>
    private readonly DirectoryControl _view;

    /// <summary>Makes a directory dock.</summary>
    /// <param name="host">The host, for logging what the directory cannot do.</param>
    /// <param name="browser">The one location, which this dock is a view of.</param>
    /// <param name="clip">What a copy or a cut has put within reach of a paste.</param>
    public DirectoryDock(IPluginHost host, Browser browser, Clip clip) =>
        _view = new DirectoryControl(host, browser, clip);

    /// <inheritdoc />
    public Control View => _view;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderCommand> HeaderCommands => [];

    /// <inheritdoc />
    public void Restored(IDockState state) => _view.Restored(state);
}

/// <summary>
/// The directory's control: the location's entries, as a list or a grid, at a zoom.
/// </summary>
/// <remarks>
/// Neither navigation nor the tree is here: back, forward, up, refresh and the address are a dock of
/// their own, and so is the tree (Section 7.5). What is here is one directory, read at a size, which is
/// the one thing about this dock that is its own — where it is looking is the same wherever else it is
/// looked at.
/// <para>
/// The arrangement is not a second choice beside the size: entries read too small to be tiles are read as
/// rows of names, and entries read large enough are read as tiles, so one scale decides both and there is
/// nothing to keep but the scale. A switch between two arrangements would let a dock be a grid of tiny
/// pictures, which is a grid nobody asked for.
/// </para>
/// </remarks>
internal sealed class DirectoryControl : UserControl
{
    /// <summary>The zoom the dock opens at, as a percentage of the size an icon is at its own.</summary>
    private const double Opening = 100.0;

    /// <summary>How little and how much a dock may be zoomed, so that the slider stops somewhere.</summary>
    private const double Least = 50.0;
    private const double Most = 200.0;

    /// <summary>How far one step of the slider, or one turn of the wheel, moves the zoom.</summary>
    private const double Step = 10.0;

    /// <summary>Above this zoom the entries are tiles; at it or below they are rows.</summary>
    private const double GridAbove = 60.0;

    /// <summary>The dock key the zoom is kept under.</summary>
    private const string ZoomKey = "zoom";

    /// <summary>
    /// The dock key the arrangement was kept under before a zoom decided it.
    /// </summary>
    /// <remarks>
    /// Read and never written. A dock that kept <c>grid</c> or <c>list</c> here is a dock whose user had
    /// chosen an arrangement, and opening at the zoom that arrangement amounts to is what keeps their choice
    /// rather than replacing it with the default. It is written no more because the zoom says the same thing
    /// and more, and two keys saying it is how they come to disagree.
    /// </remarks>
    private const string LayoutKey = "layout";

    /// <summary>What the arrangement used to be kept as, when there were two to choose from.</summary>
    private const string Grid = "grid";
    private const string List = "list";

    /// <summary>Where the browser is, which every dock looks at the same one of.</summary>
    private readonly Browser _browser;

    /// <summary>The host, which the views are made over.</summary>
    private readonly IPluginHost _host;

    /// <summary>What a copy or a cut has put within reach of a paste.</summary>
    private readonly Clip _clip;

    /// <summary>What the views do, so that every view of the location behaves alike.</summary>
    private readonly BrowserActions _actions;

    /// <summary>The zoom the entries are read at, as a percentage.</summary>
    private readonly Slider _zoom = new();

    /// <summary>Whichever arrangement the zoom amounts to.</summary>
    private readonly ContentControl _content = new();

    /// <summary>What the dock kept, and where the next of it is kept.</summary>
    private IDockState? _state;

    /// <summary>The entries as this dock last drew them, so that a reread can be told from a step.</summary>
    private IReadOnlyList<Entry>? _drawn;

    /// <summary>Makes the directory's control.</summary>
    /// <param name="host">The host, for logging what the directory cannot do.</param>
    /// <param name="browser">The one location, which this dock is a view of.</param>
    /// <param name="clip">What a copy or a cut has put within reach of a paste.</param>
    public DirectoryControl(IPluginHost host, Browser browser, Clip clip)
    {
        _host = host;
        _clip = clip;
        _browser = browser;
        _actions = new BrowserActions(host, browser, clip);

        // Listening while it is on screen rather than for as long as it exists: a dock that was
        // closed is not a view of anything, and one that was dragged to another region is taken off
        // the tree and put back, which is a departure and a return for a dock that never closed.
        AttachedToVisualTree += (_, _) =>
        {
            _browser.Changed += Update;
            Draw();
        };
        DetachedFromVisualTree += (_, _) => _browser.Changed -= Update;

        // Tunnelled, so that the zoom is reached before whatever the pointer is over reads the wheel as a
        // scroll: a zoom that stopped working as soon as the entries were worth scrolling would be a zoom
        // that stopped working where it is wanted most.
        AddHandler(PointerWheelChangedEvent, Wheeled, RoutingStrategies.Tunnel);

        FillZoom();

        // Over the entries rather than under them: it reads as what the pane is being read at, and it is out
        // of the way in the corner rather than across the top, where the entries begin.
        var bar = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            HorizontalAlignment = HorizontalAlignment.Right,
            Margin = new Thickness(4),
        };
        bar.Children.Add(_zoom);

        var panel = new DockPanel { LastChildFill = true };
        DockPanel.SetDock(bar, Dock.Bottom);
        panel.Children.Add(bar);
        panel.Children.Add(_content);

        Content = panel;

        Draw();
    }

    /// <summary>
    /// Takes what this dock was, which is the zoom it was left at.
    /// </summary>
    /// <remarks>
    /// Read before the dock is drawn, so that a directory that was left as tiles comes back as tiles rather
    /// than being drawn as rows and rearranging under the first frame. The host makes the view and then tells
    /// it what it was, which is why there is anything to do here at all.
    /// </remarks>
    /// <param name="state">What the dock kept, and where the next of it is kept.</param>
    public void Restored(IDockState state)
    {
        _state = state;

        if (Kept(state) is { } zoom)
        {
            _zoom.Value = zoom;
        }

        Draw();
    }

    /// <summary>Sets the slider up, and what moving it does.</summary>
    private void FillZoom()
    {
        _zoom.Minimum = Least;
        _zoom.Maximum = Most;
        _zoom.Value = Opening;
        _zoom.Width = 140;
        _zoom.TickPlacement = TickPlacement.None;
        _zoom.VerticalAlignment = VerticalAlignment.Center;

        // Snapped, so that a zoom is one of a handful of sizes rather than wherever a drag let go: an icon is
        // read from the desktop once per size it is asked for, and a slider that never stopped between two
        // sizes would ask for one nearly every time it moved.
        _zoom.IsSnapToTickEnabled = true;
        _zoom.TickFrequency = Step;

        _zoom.ValueChanged += (_, _) =>
        {
            // Written rather than saved: the dock keeps what it was and the host writes the layout, which is
            // the same division as everything else about where a dock is.
            _state?.Write(ZoomKey, _zoom.Value.ToString(CultureInfo.InvariantCulture));
            Draw();
        };
    }

    /// <summary>
    /// Takes a turn of the wheel with control held as a step of zoom.
    /// </summary>
    /// <remarks>
    /// A turn without control is left alone for whatever is under the pointer to read, which is what makes
    /// the wheel scroll the entries and only zoom them when it is asked to.
    /// </remarks>
    private void Wheeled(object? sender, PointerWheelEventArgs e)
    {
        if (!e.KeyModifiers.HasFlag(KeyModifiers.Control))
        {
            return;
        }

        _zoom.Value = Math.Clamp(_zoom.Value + Math.Sign(e.Delta.Y) * Step, Least, Most);
        e.Handled = true;
    }

    /// <summary>Draws the entries again when they are not the ones it drew.</summary>
    /// <remarks>
    /// Every read of a directory is a new list, so one list being another is what a step, a refresh
    /// and a change of base all look like from here.
    /// </remarks>
    private void Update()
    {
        if (!ReferenceEquals(_drawn, _browser.Shown))
        {
            Draw();
        }
    }

    /// <summary>Draws what is there, at the zoom this dock reads it.</summary>
    private void Draw()
    {
        _content.Content = _zoom.Value > GridAbove
            ? new GridBrowser(_host, _browser, _actions, _clip, Icons.SizeAt(_zoom.Value))
            : new ListBrowser(_host, _browser, _actions, _clip);

        _drawn = _browser.Shown;
    }

    /// <summary>
    /// The zoom this dock was left at, written down either as a zoom or, from before there were zooms, as
    /// the arrangement it used to be read in.
    /// </summary>
    /// <param name="state">What the dock kept.</param>
    /// <returns>The zoom, or nothing when it kept none.</returns>
    private static double? Kept(IDockState state) =>
        double.TryParse(state.Read(ZoomKey), NumberStyles.Float, CultureInfo.InvariantCulture, out var zoom)
            ? Math.Clamp(zoom, Least, Most)
            : state.Read(LayoutKey) switch
            {
                Grid => Opening,
                List => Least,
                _ => null,
            };
}
