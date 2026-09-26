using System.Globalization;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace FileSystemPlugin;

/// <summary>The Directory dock: one directory's entries, read at a zoom.</summary>
internal sealed class DirectoryDock : IDockView
{
    /// <summary>The control the dock shows.</summary>
    private readonly DirectoryControl _view;

    /// <summary>Makes a directory dock.</summary>
    /// <param name="host">The host, for logging what the directory cannot do.</param>
    /// <param name="shared">The answers every location shares.</param>
    /// <param name="browser">The one location, which this dock is a view of until it is taken out of step.</param>
    /// <param name="clip">What a copy or a cut has put within reach of a paste.</param>
    public DirectoryDock(IPluginHost host, Shared shared, Browser browser, Clip clip) =>
        _view = new DirectoryControl(host, shared, browser, clip);

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
/// their own, and so is the tree (Section 7.5). What is here is one directory, read at a size, and the size
/// is the one thing about this dock that is its own by default — the directory is every dock's, until this
/// one is taken out of step and given a location of its own.
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

    /// <summary>The dock key the choice about hidden entries is kept under.</summary>
    private const string HiddenKey = "hidden";

    /// <summary>The dock key the choice about following the whole is kept under.</summary>
    private const string SyncKey = "sync";

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

    /// <summary>The answers every location shares, which a dock out of step goes on reading.</summary>
    private readonly Shared _shared;

    /// <summary>The one location, which this dock is a view of until it is taken out of step.</summary>
    private readonly Browser _whole;

    /// <summary>The host, which the views are made over.</summary>
    private readonly IPluginHost _host;

    /// <summary>What a copy or a cut has put within reach of a paste.</summary>
    private readonly Clip _clip;

    /// <summary>What the views do, so that every view of the location behaves alike.</summary>
    private BrowserActions _actions;

    /// <summary>The zoom the entries are read at, as a percentage.</summary>
    private readonly Slider _zoom = new();

    /// <summary>Whether the entries the platform hides are shown.</summary>
    private readonly CheckBox _hidden = new();

    /// <summary>Whether this dock follows the whole, or looks at a directory of its own.</summary>
    private readonly CheckBox _sync = new();

    /// <summary>
    /// What the dock keeps beside the address, which is put at the far end of the toolbar.
    /// </summary>
    /// <remarks>
    /// Built once and handed to every toolbar this dock makes, because the toolbar is told which location to
    /// read rather than being made again (see <see cref="NavigationBar.Reading"/>), and a control that was
    /// handed over once cannot be handed over twice while it is still a child of the bar it left.
    /// </remarks>
    private readonly StackPanel _trailing = new()
    {
        Orientation = Orientation.Horizontal,
        Spacing = 12,
        VerticalAlignment = VerticalAlignment.Center,
    };

    /// <summary>The toolbar, kept so that being taken out of step hands it the other location.</summary>
    private readonly NavigationBar _bar;

    /// <summary>Whichever arrangement the zoom amounts to.</summary>
    private readonly ContentControl _content = new();

    /// <summary>The view being read, which is what a copy or a paste in this dock is about.</summary>
    private EntryView? _view;

    /// <summary>This dock's own location, while it is out of step, and nothing while it follows the whole.</summary>
    private Browser? _own;

    /// <summary>The location being listened to, so that one left behind is stopped being listened to.</summary>
    private Browser? _watched;

    /// <summary>What the dock kept, and where the next of it is kept.</summary>
    private IDockState? _state;

    /// <summary>The entries as this dock last drew them, so that a reread can be told from a step.</summary>
    private IReadOnlyList<Entry>? _drawn;

    /// <summary>
    /// What this dock is looking at now.
    /// </summary>
    /// <remarks>
    /// Its own location once it has been taken out of step, and the whole's until then — which is why nothing
    /// here reads the whole directly but this: a dock out of step that went on reading the whole would go on
    /// following it, and not following it is what being out of step means.
    /// </remarks>
    private Browser Location => _own ?? _whole;

    /// <summary>Makes the directory's control.</summary>
    /// <param name="host">The host, for logging what the directory cannot do.</param>
    /// <param name="shared">The answers every location shares.</param>
    /// <param name="whole">The one location, which this dock is a view of until it is taken out of step.</param>
    /// <param name="clip">What a copy or a cut has put within reach of a paste.</param>
    public DirectoryControl(IPluginHost host, Shared shared, Browser whole, Clip clip)
    {
        _host = host;
        _clip = clip;
        _shared = shared;
        _whole = whole;
        _actions = new BrowserActions(host, whole, clip);

        // Listening while it is on screen rather than for as long as it exists: a dock that was
        // closed is not a view of anything, and one that was dragged to another region is taken off
        // the tree and put back, which is a departure and a return for a dock that never closed.
        AttachedToVisualTree += (_, _) =>
        {
            Watch(Location);
            Draw();
        };
        DetachedFromVisualTree += (_, _) => Watch(null);

        // Tunnelled, so that the zoom is reached before whatever the pointer is over reads the wheel as a
        // scroll: a zoom that stopped working as soon as the entries were worth scrolling would be a zoom
        // that stopped working where it is wanted most.
        AddHandler(PointerWheelChangedEvent, Wheeled, RoutingStrategies.Tunnel);

        // And on the way down for the same reason: F5 is answered wherever in the dock the keyboard is, so that
        // a listing that happened to have the keyboard would not be the only place it worked.
        AddHandler(KeyDownEvent, Keyed, RoutingStrategies.Tunnel);

        FillZoom();
        FillHidden();
        FillSync();
        _trailing.Children.Add(_hidden);
        _trailing.Children.Add(_sync);

        // The toolbar is the dock's own rather than a dock beside it: the arrows and the address act on the
        // location being read, and they belong over the entries they act on (Section 7.5). What the dock keeps
        // beside them is handed in, so that the toolbar does not have to know what a dock remembers.
        _bar = new NavigationBar(_host, Location, _trailing) { Left = Seated };

        // What is left at the foot is the zoom alone, which is the one thing about reading a directory that has
        // nothing to do with where it is.
        var foot = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            HorizontalAlignment = HorizontalAlignment.Right,
            Margin = new Thickness(12, 0, 12, 8),
            Children = { _zoom },
        };

        var panel = new DockPanel { LastChildFill = true };
        DockPanel.SetDock(_bar, Dock.Top);
        DockPanel.SetDock(foot, Dock.Bottom);
        panel.Children.Add(_bar);
        panel.Children.Add(foot);
        panel.Children.Add(_content);

        Content = panel;

        Draw();
    }

    /// <summary>
    /// Takes what this dock was: the zoom it was left at, whether hidden entries were shown, and whether it
    /// followed the whole.
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

        // Read before the dock is drawn, for the same reason the zoom is: a dock left showing what is hidden
        // should come back showing it rather than drawing the listing without and redoing it.
        if (bool.TryParse(state.Read(HiddenKey), out var hidden))
        {
            _hidden.IsChecked = hidden;
        }

        // And for the same reason again: a dock left out of step comes back out of step, at a directory of its
        // own that nothing keeps — the location is not written into the layout (Section 7.3) — rather than
        // being drawn in step and switching under the first frame.
        if (bool.TryParse(state.Read(SyncKey), out var inStep))
        {
            _sync.IsChecked = inStep;
        }

        Draw();
    }

    /// <summary>Sets the toggle up, and what turning it does.</summary>
    /// <remarks>
    /// The choice is the dock's to keep and the shared answers are the plugin's to hold: the listing follows
    /// the base and the hiding wherever it is, and a dock that kept them to itself would list something the
    /// next dock did not.
    /// </remarks>
    private void FillHidden()
    {
        _hidden.Content = RolaI18N.Get("rorolala_file_system.hidden");
        _hidden.VerticalAlignment = VerticalAlignment.Center;

        _hidden.IsCheckedChanged += (_, _) =>
        {
            var shown = _hidden.IsChecked == true;

            // Written rather than saved, as the zoom is: the dock keeps what it was and the host writes the
            // layout, which is the same division as everything else about where a dock is.
            _state?.Write(HiddenKey, shown.ToString(CultureInfo.InvariantCulture));
            _shared.ShowHidden = shown;
        };
    }

    /// <summary>
    /// Sets the sync toggle up, and what turning it does.
    /// </summary>
    /// <remarks>
    /// It is the one choice about where a dock is looking that is the dock's own, which is why it sits with
    /// what the dock keeps rather than in the toolbar's tools: a direction is everybody's, and being in step
    /// with it is this dock's answer.
    /// </remarks>
    private void FillSync()
    {
        _sync.Content = RolaI18N.Get("rorolala_file_system.sync");
        _sync.VerticalAlignment = VerticalAlignment.Center;

        // In step until it is said otherwise, which is what a dock opened fresh is: the whole is where a run
        // starts, and a dock that opened somewhere of its own would be a second place to find on the first
        // frame.
        _sync.IsChecked = true;

        _sync.IsCheckedChanged += (_, _) => InStep(_sync.IsChecked == true);
    }

    /// <summary>
    /// Follows the whole again, or looks at a directory of this dock's own.
    /// </summary>
    /// <remarks>
    /// Leaving starts from where the whole is looking, because that is where this dock is looking when it
    /// leaves: a dock that appeared somewhere else entirely would be a second place to find rather than a
    /// place this one went to.
    /// <para>
    /// Coming back throws that location away, which is why a location is let go of rather than dropped
    /// (<see cref="Browser.Dispose"/>): what it holds of the plugin's outlives it.
    /// </para>
    /// </remarks>
    /// <param name="inStep">Whether this dock is to follow the whole.</param>
    private void InStep(bool inStep)
    {
        // Written rather than saved, as the zoom and the hiding are: the dock keeps what it was and the host
        // writes the layout.
        _state?.Write(SyncKey, inStep.ToString(CultureInfo.InvariantCulture));

        var was = Location;

        Watch(null);
        _own?.Dispose();
        _own = inStep ? null : new Browser(_shared, was.Current);
        _actions = new BrowserActions(_host, Location, _clip);

        _bar.Reading(Location);
        Watch(Location);

        // Only when it is another location: coming back in step while already in step is not a place to go,
        // and drawing again would only throw the listing away for the one it already has.
        if (!ReferenceEquals(was, Location))
        {
            Draw();
        }
    }

    /// <summary>
    /// Listens to a location, so that what it holds is drawn again when it changes, or listens to none.
    /// </summary>
    /// <remarks>
    /// One at a time, and while the dock is on screen: a dock out of step is a view of its own location and not
    /// of the whole's, so listening to both would redraw it for a change it is no longer following. What the
    /// toolbar listens to is its own affair and moves with it (see <see cref="NavigationBar.Reading"/>).
    /// </remarks>
    /// <param name="location">What to listen to, or nothing to stop listening.</param>
    private void Watch(Browser? location)
    {
        if (ReferenceEquals(_watched, location))
        {
            return;
        }

        if (_watched is not null)
        {
            _watched.Changed -= Update;
        }

        _watched = location;

        if (_watched is not null)
        {
            _watched.Changed += Update;
        }
    }

    /// <summary>Sets the slider up, and what moving it does.</summary>
    private void FillZoom()
    {
        _zoom.Minimum = Least;
        _zoom.Maximum = Most;
        _zoom.Value = Opening;
        _zoom.Width = 144;
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
    /// Takes the keyboard back into the dock, which is where a dock's own keys are answered from.
    /// </summary>
    /// <remarks>
    /// The listing and not the dock itself: the keys are taken at the top of the dock and handed to the view being
    /// read, so a view that is not focused is a view that never receives one. It is what the address asks for when
    /// an edit ends, so that typing a path and pressing Return leaves the keyboard where the keys are
    /// (Section 7.7).
    /// </remarks>
    private void Seated() => _view?.Listen();

    /// <summary>
    /// Reads the keys a dock answers for the whole of itself.
    /// </summary>
    /// <remarks>
    /// Read again on <c>F5</c>, which is the dock's own key, and the three the clipboard answers, which are
    /// taken here rather than by the listing so that they are answered wherever the keyboard is in the dock —
    /// after a zoom was dragged as much as after an entry was clicked (Section 7.7).
    /// </remarks>
    /// <param name="sender">The dock.</param>
    /// <param name="e">The key.</param>
    private void Keyed(object? sender, KeyEventArgs e)
    {
        if (Keys.Again(e, _whole, _host.Log))
        {
            return;
        }

        if (_view is { } view)
        {
            _ = Keys.Clipboard(e, new Clipboard(view.Copy, view.Cut, view.Paste), _host.Log);
        }
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
        // The toggle follows the shared answer as well as leading it: what is shown is one answer for every
        // dock, so one opened after another was toggled shows what that one shows rather than its own last word
        // on it.
        if (_hidden.IsChecked != _shared.ShowHidden)
        {
            _hidden.IsChecked = _shared.ShowHidden;
        }

        if (!ReferenceEquals(_drawn, Location.Shown))
        {
            Draw();
        }
    }

    /// <summary>Draws what is there, at the zoom this dock reads it.</summary>
    private void Draw()
    {
        _view = _zoom.Value > GridAbove
            ? new GridBrowser(_host, Location, _actions, _clip, Icons.SizeAt(_zoom.Value))
            : new ListBrowser(_host, Location, _actions, _clip);

        _content.Content = _view;

        _drawn = Location.Shown;
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
