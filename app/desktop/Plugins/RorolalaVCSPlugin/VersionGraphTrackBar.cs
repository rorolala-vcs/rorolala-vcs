using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Threading;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// The row under the Version Graph: a message for the version an edit would make, and the button that
/// records it.
/// </summary>
/// <remarks>
/// The field is always there, because a reader who has just changed a file is already looking at the
/// graph and has nowhere else to say what the change was. What the button may do is the work's answer:
/// it is pressable only while the files chosen have unrecorded edits and something has been written,
/// since a run with neither is a run the command line would refuse.
/// <para>
/// The choice may be several files. What the graph hides then, this row is what that choice is acted
/// on through: every chosen file whose content changed is recorded in one run, and the entries that
/// need no recording — the unchanged, the lost, the ones waiting to be merged — are left out rather
/// than sent to a command that would skip them anyway.
/// </para>
/// <para>
/// A file whose work and Vault have parted is a version the Vault's chain does not carry, so tracking
/// one is confirmed first: the question names the files, and nothing is written until it is answered.
/// </para>
/// </remarks>
internal sealed class VersionGraphTrackBar : Border
{
    /// <summary>The room the field keeps inside itself, which is the whole of its frame.</summary>
    private const double Room = 16;

    /// <summary>The message the new versions are to carry.</summary>
    private readonly TextBox _message;

    /// <summary>What records the message as versions.</summary>
    private readonly Button _track;

    /// <summary>What the work says about the files chosen, shared with the listing and the graph.</summary>
    private readonly StatusReadings _statuses;

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

    /// <summary>Makes the row.</summary>
    /// <param name="host">The host, which is what the button reaches to run and to report.</param>
    /// <param name="graph">The graph the message is a version of.</param>
    /// <param name="statuses">What the work says about the files chosen.</param>
    /// <param name="hub">What every command this plugin runs reports itself to.</param>
    public VersionGraphTrackBar(
        IPluginHost host,
        VersionGraphView graph,
        StatusReadings statuses,
        ProgressHub hub
    )
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(graph);
        ArgumentNullException.ThrowIfNull(statuses);
        ArgumentNullException.ThrowIfNull(hub);

        _statuses = statuses;
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

            // The field is the row rather than a control laid on it: it draws no frame and no corner
            // of its own in any state, so nothing about it changes when it has the keyboard. The room
            // its frame would have taken is kept as padding instead.
            BorderThickness = new Thickness(0),
            CornerRadius = new CornerRadius(0),
            Background = Brushes.Transparent,
            Padding = new Thickness(Room),
            VerticalContentAlignment = VerticalAlignment.Center,
        };

        // The theme draws a ring and a line on the template's border once the field has the keyboard.
        // The template's own border is given the nothing it is to draw as local values: a local value
        // outranks a style, so the focus rule has nothing left to change.
        _message.TemplateApplied += (_, applied) => Framed(applied.NameScope);

        _message.TextChanged += (_, _) => Paint();
        _message.KeyDown += (_, pressed) => Entered(pressed);

        // The button is the rest of the row rather than a control laid on it: it takes the whole of the
        // room beside the field, with no frame of its own beyond the one line separating the two, so the
        // whole right-hand end of the strip is the thing to press.
        _track = new Button
        {
            Content = RolaI18N.Get("rorolala_vcs.version_graph.track"),
            Background = Brushes.Transparent,
            BorderThickness = new Thickness(1, 0, 0, 0),
            CornerRadius = new CornerRadius(0),
            Padding = new Thickness(Room, 0),
            Margin = new Thickness(0),
            HorizontalAlignment = HorizontalAlignment.Stretch,
            VerticalAlignment = VerticalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Center,
            VerticalContentAlignment = VerticalAlignment.Center,
        };

        _track[!TemplatedControl.BorderBrushProperty] = new DynamicResourceExtension(
            PaletteKeys.Splitter
        );

        _track.Click += (_, _) => Track();

        // Twice as wide as it is tall, the height being the row's own: the mark is a word that is read
        // across the strip, and a wide one is what fills the end of it without a frame of its own.
        _track.SizeChanged += (_, sized) =>
        {
            var wanted = sized.NewSize.Height * 2;

            if (wanted > 0 && Math.Abs(_track.Width - wanted) > 0.5)
            {
                _track.Width = wanted;
            }
        };

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

    /// <summary>Listens while the row is on screen, since a dock that was closed acts on nothing.</summary>
    private void Wearing()
    {
        ChosenFiles.Changed += Paint;
        ChosenEntry.Changed += Paint;
        Paint();
    }

    /// <summary>Stops listening, so a closed dock is not kept alive by the catalogues.</summary>
    private void Unwearing()
    {
        ChosenFiles.Changed -= Paint;
        ChosenEntry.Changed -= Paint;
    }

    /// <summary>Gives the template's border the nothing every state of the field is drawn as.</summary>
    /// <param name="scope">The names the template was made with.</param>
    private static void Framed(INameScope scope)
    {
        if (scope.Find<Border>("border") is not { } border)
        {
            return;
        }

        border.BoxShadow = default;
        border.BorderThickness = new Thickness(0);
        border.CornerRadius = new CornerRadius(0);
        border.BorderBrush = Brushes.Transparent;
    }

    /// <summary>
    /// Runs the track the message and the files chosen say may be run.
    /// </summary>
    /// <remarks>
    /// A choice with a file that has parted from the Vault is still tracked; what the parting changes is
    /// that the version being made is named back to the reader, and the run waits for an answer.
    /// </remarks>
    private void Track()
    {
        if (_busy || _message.Text is not { } written)
        {
            return;
        }

        var message = written.Trim();
        var targets = Targets();

        if (message.Length == 0 || targets.Count == 0)
        {
            return;
        }

        var diverged = targets
            .Where(target => target.Diverged)
            .Select(target => Path.GetFileName(target.File))
            .ToArray();

        if (diverged.Length > 0)
        {
            _dialogs.Show(
                new Dialog(
                    RolaI18N.Get("rorolala_vcs.version_graph.track_diverged_title"),
                    RolaI18N.Get(
                        "rorolala_vcs.version_graph.track_diverged_message",
                        string.Join(", ", diverged)
                    ),
                    () => Run(message, targets)
                )
            );

            return;
        }

        Run(message, targets);
    }

    /// <summary>Runs the track over the files the question was answered for.</summary>
    /// <param name="message">What the versions are to say.</param>
    /// <param name="targets">The files to record, read when the question was put.</param>
    private void Run(string message, IReadOnlyList<Target> targets)
    {
        if (_busy || targets.Count == 0)
        {
            return;
        }

        _busy = true;
        Paint();

        var run = _hub.Begin("rorolala_vcs.progress.track");
        var directory = targets[0].Directory;
        var files = targets.Select(target => target.File).ToArray();

        _ = Task.Run(() =>
        {
            var (done, said) = TrackCommand.Run(directory, files, message);

            Dispatcher.UIThread.Post(() =>
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

    /// <summary>
    /// The files a track would record: those chosen whose content changed, less the ones that need no
    /// recording or must not be recorded, each with whether its chain has parted from the Vault's.
    /// </summary>
    /// <remarks>
    /// A file that stands as recorded is nothing to make a version of, and the command line skips it;
    /// skipping it here is what makes the button's answer the same as the run's. A new file is work to
    /// record; a file whose base upstream moved past is too, and is the one the question is put for.
    /// A file another account holds, a move that changed nothing, and one waiting to be merged are
    /// each a different action's to settle rather than a track's, so they are left out.
    /// </remarks>
    /// <returns>The files to record, in the order the listing shows them.</returns>
    private IReadOnlyList<Target> Targets()
    {
        var chosen = ChosenFiles.Current;

        IReadOnlyList<Entry> considered = chosen.Count > 0
            ? chosen
            : ChosenEntry.Current is { } one
                ? [one]
                : [];

        var targets = new List<Target>(considered.Count);

        foreach (var entry in considered)
        {
            if (
                entry.Kind != EntryKind.File
                || Path.GetDirectoryName(entry.Path) is not { Length: > 0 } directory
                || _statuses.Reading(directory) is not { } status
            )
            {
                continue;
            }

            try
            {
                var state = status.StateOf(entry.Path);

                // A change either account may record, a file the Layout does not name yet, and a change
                // against a base upstream moved past — that last one is the track with a question.
                var recordable = state switch
                {
                    PathState.Modified or PathState.Untagged => true,
                    PathState.Wrong => !status.Unowned.Any(named =>
                        StatusReadings.Names(status.Root, named, entry.Path)
                    ),
                    _ => false,
                };

                if (recordable)
                {
                    targets.Add(
                        new Target(
                            directory,
                            entry.Path,
                            status.SyncOf(entry.Path) == SyncStanding.Diverged
                        )
                    );
                }
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                // A reading that cannot answer about one path is one file left out, rather than a row
                // that refuses every file beside it. What happened is the reading's to say, as it did
                // when the listing drew the same file.
            }
        }

        return targets;
    }

    /// <summary>Draws the button as the files chosen and what is written say it is.</summary>
    private void Paint()
    {
        var targets = Targets();
        var ready = !_busy && targets.Count > 0 && !string.IsNullOrWhiteSpace(_message.Text);

        _track.IsEnabled = ready;

        // The button keeps the look's ink while it can be pressed and the faint one while it cannot:
        // what the row offers is said by the weight of the word as well as by the press being refused.
        _track[!TemplatedControl.ForegroundProperty] = new DynamicResourceExtension(
            ready ? PaletteKeys.Front : PaletteKeys.Faint
        );

        var tip = RolaI18N.Get("rorolala_vcs.version_graph.track_tip");

        if (targets.Any(target => target.Diverged))
        {
            tip = $"{tip} — {RolaI18N.Get("rorolala_vcs.version_graph.track_diverged")}";
        }

        ToolTip.SetTip(_track, tip);
    }

    /// <summary>Runs what the field holds when Enter is pressed.</summary>
    /// <param name="pressed">The key event.</param>
    private void Entered(KeyEventArgs pressed)
    {
        if (pressed.Key is not Key.Enter)
        {
            return;
        }

        Track();
        pressed.Handled = true;
    }

    /// <summary>One file a track would record, and whether its chain has parted from the Vault's.</summary>
    /// <param name="Directory">The directory the file is in, and where a run over it is made.</param>
    /// <param name="File">The file, as the program names it.</param>
    /// <param name="Diverged">Whether tracking it makes a version the Vault's chain does not carry.</param>
    private sealed record Target(string Directory, string File, bool Diverged);
}
