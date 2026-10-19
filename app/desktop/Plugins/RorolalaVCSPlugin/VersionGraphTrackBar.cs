using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// The row under the Version Graph: a message for the version an edit would make, and the button that
/// records it.
/// </summary>
/// <remarks>
/// The field is always there, because a reader who has just changed a file is already looking at the
/// graph and has nowhere else to say what the change was. What the button may do is the graph's answer:
/// it is pressable only while the file on screen has an unrecorded edit and something has been written,
/// since a run with neither is a run the command line would refuse.
/// <para>
/// A graph whose work and Vault have parted asks for the key the sync strip asks with: a track made
/// there is a version the Vault's chain does not carry, so the same acknowledgment that changes a sync's
/// meaning is what lets this run go ahead.
/// </para>
/// </remarks>
internal sealed class VersionGraphTrackBar : Border
{
    /// <summary>The room the field keeps inside itself, which is the whole of its frame.</summary>
    private const double Room = 16;

    /// <summary>The message the new version is to carry.</summary>
    private readonly TextBox _message;

    /// <summary>What records the message as a version.</summary>
    private readonly Button _track;

    /// <summary>The graph the row belongs to, which is what says whether there is anything to record.</summary>
    private readonly VersionGraphView _graph;

    /// <summary>Where the files are said to have changed, so that every view reads the work again.</summary>
    private readonly IFileChanges _files;

    /// <summary>The pictures a failure is reported with.</summary>
    private readonly IIconLibrary _icons;

    /// <summary>Where a question is put to the user, and a run that failed is reported.</summary>
    private readonly IDialogs _dialogs;

    /// <summary>What every command this plugin runs reports itself to.</summary>
    private readonly ProgressHub _hub;

    /// <summary>Whether a track is going, which is what holds the button until it ends.</summary>
    private bool _busy;

    /// <summary>Whether the key that acknowledges a parted chain is held.</summary>
    private bool _shifted;

    /// <summary>Makes the row.</summary>
    /// <param name="host">The host, which is what the button reaches to run and to report.</param>
    /// <param name="graph">The graph the message is a version of.</param>
    /// <param name="hub">What every command this plugin runs reports itself to.</param>
    public VersionGraphTrackBar(IPluginHost host, VersionGraphView graph, ProgressHub hub)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(graph);
        ArgumentNullException.ThrowIfNull(hub);

        _graph = graph;
        _files = host.Files;
        _icons = host.Icons;
        _dialogs = host.Dialogs;
        _hub = hub;

        _message = new TextBox
        {
            PlaceholderText = RolaI18N.Get("rorolala_vcs.version_graph.track_hint"),

            // One line is the whole of what a version's message is here: the command line's editor is
            // the place for a longer one, and this row is for the change the reader has just made.
            AcceptsReturn = false,
            TextWrapping = TextWrapping.NoWrap,

            // The field is the row rather than a control laid on it: its frame would be a second edge
            // inside the strip's own, so it draws none and keeps the room its frame would have taken.
            BorderThickness = new Thickness(0),
            Background = Brushes.Transparent,
            Padding = new Thickness(Room),
            VerticalContentAlignment = VerticalAlignment.Center,
        };

        _message.TextChanged += (_, _) => Paint();
        _message.KeyDown += (_, pressed) => Entered(pressed);

        _track = new Button
        {
            Content = RolaI18N.Get("rorolala_vcs.version_graph.track"),
            Classes = { "primary" },
            Padding = new Thickness(Room, 8),
            Margin = new Thickness(0, 8, Room, 8),
            VerticalAlignment = VerticalAlignment.Center,
        };

        _track.Click += (_, _) => Track();

        // What is being held is read again at the press: a key pressed between the last key event and
        // the press is a key the row has not been told about yet, so what the press says wins.
        _track.AddHandler(
            InputElement.PointerPressedEvent,
            (_, pressed) =>
            {
                if (pressed is PointerPressedEventArgs at)
                {
                    Shift(at.KeyModifiers);
                }
            },
            RoutingStrategies.Tunnel
        );

        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,Auto"),
            Children = { _message },
        };

        Grid.SetColumn(_track, 1);
        row.Children.Add(_track);

        Child = row;

        // The strip is a part of the dock rather than a card on it: the line over it is the same line
        // the dock uses between two regions, and the ground is the sunken one, so the field reads as
        // sitting in the dock rather than as a second surface laid on it.
        BorderThickness = new Thickness(0, 1, 0, 0);
        this[!BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);
        this[!BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Dark);

        graph.TrackableChanged += (_, _) => Paint();
        AttachedToVisualTree += (_, _) => Wearing();
        DetachedFromVisualTree += (_, _) => Unwearing();

        Paint();
    }

    /// <summary>Runs the track the message and the graph say may be run.</summary>
    /// <remarks>
    /// A parted chain is gone back on only with the key held, which is read at the press as well as
    /// here: the button does nothing while the acknowledgment is missing, so a reader who did not know
    /// why sees the tooltip rather than a version made beside the Vault's.
    /// </remarks>
    private void Track()
    {
        if (_busy || !Ready())
        {
            return;
        }

        if (_graph.Diverged && !_shifted)
        {
            return;
        }

        if (_graph.Trackable is not { } target || _message.Text is not { } written)
        {
            return;
        }

        var message = written.Trim();

        if (message.Length == 0)
        {
            return;
        }

        _busy = true;
        Paint();

        var run = _hub.Begin("rorolala_vcs.progress.track");

        _ = Task.Run(() =>
        {
            var (done, said) = TrackCommand.Run(target.Directory, target.File, message);

            Avalonia.Threading.Dispatcher.UIThread.Post(() =>
            {
                _busy = false;
                _hub.End(run);

                if (!done)
                {
                    _dialogs.Present(
                        RolaI18N.Get("rorolala_vcs.version_graph.track_failed"),
                        () =>
                            ReportView.Of(
                                ReportLines.Parse(said),
                                key => _icons.Find(key),
                                ReportView.Ink
                            )
                    );

                    Paint();

                    return;
                }

                // What was written is now a version, so the field starts again empty and every view of
                // the work — the graph among them — is told that the files may have changed.
                _message.Text = string.Empty;
                _files.Touch();
                Paint();
            });
        });
    }

    /// <summary>Whether the button may be pressed: there is an edit, and something was written for it.</summary>
    /// <returns>Whether a run would have something to record.</returns>
    private bool Ready() =>
        !_busy
        && _graph.Trackable is not null
        && !string.IsNullOrWhiteSpace(_message.Text);

    /// <summary>Draws the button as the graph and what is written say it is.</summary>
    private void Paint()
    {
        var ready = Ready();
        var diverged = _graph.Diverged;
        var forced = diverged && _shifted;

        _track.IsEnabled = ready;

        var tip = RolaI18N.Get("rorolala_vcs.version_graph.track_tip");

        if (diverged)
        {
            tip = $"{tip} — {RolaI18N.Get("rorolala_vcs.version_graph.track_diverged")}";
        }

        ToolTip.SetTip(_track, tip);

        // The key being held is what turns the button into the colour the look spends on what must not
        // be done without saying so, as the sync strip does: what it acknowledges is a version made
        // beside the chain the Vault carries.
        if (forced)
        {
            _track[!TemplatedControl.BackgroundProperty] = new DynamicResourceExtension(
                PaletteKeys.Del
            );
        }
        else
        {
            _track.ClearValue(TemplatedControl.BackgroundProperty);
        }
    }

    /// <summary>Runs what the field holds when Enter is pressed.</summary>
    /// <param name="pressed">The key event.</param>
    private void Entered(KeyEventArgs pressed)
    {
        if (pressed.Key is not Key.Enter)
        {
            return;
        }

        Shift(pressed.KeyModifiers);
        Track();
        pressed.Handled = true;
    }

    /// <summary>Watches for the key that acknowledges a parted chain, wherever the pointer is.</summary>
    private void Wearing()
    {
        if (TopLevel.GetTopLevel(this) is not { } window)
        {
            return;
        }

        window.AddHandler(InputElement.KeyDownEvent, KeyHeld, RoutingStrategies.Tunnel);
        window.AddHandler(InputElement.KeyUpEvent, KeyLetGo, RoutingStrategies.Tunnel);
    }

    /// <summary>Stops watching, since a row taken out of a tree is not drawn by the key any more.</summary>
    private void Unwearing()
    {
        if (TopLevel.GetTopLevel(this) is not { } window)
        {
            return;
        }

        window.RemoveHandler(InputElement.KeyDownEvent, KeyHeld);
        window.RemoveHandler(InputElement.KeyUpEvent, KeyLetGo);
    }

    /// <summary>Draws the row for the key that went down.</summary>
    /// <param name="source">What raised it.</param>
    /// <param name="pressed">The event.</param>
    private void KeyHeld(object? source, KeyEventArgs pressed)
    {
        _ = source;
        Shift(pressed.KeyModifiers);
    }

    /// <summary>Draws it as it was once the key is up.</summary>
    /// <param name="source">What raised it.</param>
    /// <param name="released">The event.</param>
    private void KeyLetGo(object? source, KeyEventArgs released)
    {
        _ = source;
        Shift(released.KeyModifiers);
    }

    /// <summary>Redraws the button in the colour the modifiers say.</summary>
    /// <param name="modifiers">What is being held.</param>
    private void Shift(KeyModifiers modifiers)
    {
        var shifted = modifiers.HasFlag(KeyModifiers.Shift);

        if (shifted == _shifted)
        {
            return;
        }

        _shifted = shifted;
        Paint();
    }
}
