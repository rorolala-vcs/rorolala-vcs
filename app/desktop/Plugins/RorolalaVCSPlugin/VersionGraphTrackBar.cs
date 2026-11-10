using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Data;
using Avalonia.Input;
using Avalonia.Interactivity;
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
/// need no recording (the unchanged, the lost, the ones waiting to be merged) are left out rather
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

    /// <summary>What the button is drawn in while Shift is held: the semantic force.</summary>
    private const string Held = PaletteKeys.Force;

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
    private readonly IProgressHub _hub;

    /// <summary>Whether a track is going, which is what holds the button until it ends.</summary>
    private bool _busy;

    /// <summary>Whether Shift is held, which makes the button a forced track and draws it red.</summary>
    private bool _shifted;

    /// <summary>What each tree was read as, kept for the drawing between file changes.</summary>
    private readonly Dictionary<string, RolaTreeStatus?> _trees = new(StringComparer.Ordinal);

    /// <summary>The trees a reading is going for, so one tree is read once at a time.</summary>
    private readonly HashSet<string> _askingTrees = new(StringComparer.Ordinal);

    /// <summary>Which reading of the work the kept tree answers are of.</summary>
    private int _treeGeneration;

    /// <summary>Makes the row.</summary>
    /// <param name="host">The host, which is what the button reaches to run and to report.</param>
    /// <param name="graph">The graph the message is a version of.</param>
    /// <param name="statuses">What the work says about the files chosen.</param>
    /// <param name="hub">What every command this plugin runs reports itself to.</param>
    public VersionGraphTrackBar(
        IPluginHost host,
        VersionGraphView graph,
        StatusReadings statuses,
        IProgressHub hub
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

            // The field is the whole of the row's left side, not only the line the text sits on: the
            // base theme puts the beam on the text part alone, which leaves the field's own room
            // reading as ground. The cursor is inherited, so naming it here covers that room as well.
            Cursor = new Cursor(StandardCursorType.Ibeam),
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
        ChosenDirectory.Changed += Paint;
        _files.Changed += TreesChanged;
        _files.Redrawn += Paint;

        // Shift is held while the pointer is anywhere, so the key is watched on the window rather than on
        // this row: a row that only heard it while its own field had the keyboard would not turn red for
        // the hand that is already moving to the button.
        if (TopLevel.GetTopLevel(this) is { } window)
        {
            window.AddHandler(InputElement.KeyDownEvent, KeyHeld, RoutingStrategies.Tunnel);
            window.AddHandler(InputElement.KeyUpEvent, KeyLetGo, RoutingStrategies.Tunnel);
        }

        Paint();
    }

    /// <summary>Stops listening, so a closed dock is not kept alive by the catalogues.</summary>
    private void Unwearing()
    {
        ChosenFiles.Changed -= Paint;
        ChosenEntry.Changed -= Paint;
        ChosenDirectory.Changed -= Paint;
        _files.Changed -= TreesChanged;
        _files.Redrawn -= Paint;

        if (TopLevel.GetTopLevel(this) is { } window)
        {
            window.RemoveHandler(InputElement.KeyDownEvent, KeyHeld);
            window.RemoveHandler(InputElement.KeyUpEvent, KeyLetGo);
        }
    }

    /// <summary>Drops the tree readings a file change may have made stale, then draws the row again.</summary>
    private void TreesChanged()
    {
        _treeGeneration++;
        _trees.Clear();
        Paint();
    }

    /// <summary>Draws the button red while the key that forces the run is down.</summary>
    /// <param name="source">What raised it.</param>
    /// <param name="pressed">The event.</param>
    private void KeyHeld(object? source, KeyEventArgs pressed)
    {
        _ = source;
        Modifiers(pressed.KeyModifiers);
    }

    /// <summary>Draws it as it was once the key is up.</summary>
    /// <param name="source">What raised it.</param>
    /// <param name="released">The event.</param>
    private void KeyLetGo(object? source, KeyEventArgs released)
    {
        _ = source;
        Modifiers(released.KeyModifiers);
    }

    /// <summary>Redraws the button in the colour the modifiers say.</summary>
    /// <param name="modifiers">What is being held.</param>
    private void Modifiers(KeyModifiers modifiers)
    {
        var shifted = modifiers.HasFlag(KeyModifiers.Shift);

        if (shifted == _shifted)
        {
            return;
        }

        _shifted = shifted;
        Paint();
    }

    /// <summary>Gives the template's frame and hint the values the row needs.</summary>
    /// <remarks>
    /// The frame is the nothing every state of the field is drawn as. The hint takes the field's own
    /// font size, so a hint and a message make the row the same height.
    /// </remarks>
    /// <param name="scope">The names the template was made with.</param>
    private static void Framed(INameScope scope)
    {
        if (scope.Find<Border>("border") is { } border)
        {
            border.BoxShadow = default;
            border.BorderThickness = new Thickness(0);
            border.CornerRadius = new CornerRadius(0);
            border.BorderBrush = Brushes.Transparent;
        }

        // The hint is a text block, so the base theme's text style gives it the body type rather
        // than the field's: it would be taller than the line that replaces it, and the row would
        // shrink by that much as soon as something was typed. The hint takes the field's own type
        // so the two lines are the same height.
        if (scope.Find<TextBlock>("placeholder") is { } placeholder)
        {
            placeholder[!TextBlock.FontSizeProperty] = new TemplateBinding(
                TemplatedControl.FontSizeProperty
            );
        }
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

        var force = _shifted;
        var message = written.Trim();
        var targets = Targets(force);

        if (message.Length == 0 || targets.Count == 0)
        {
            return;
        }

        var forced = targets
            .Where(target => target.Forced)
            .Select(target => Path.GetFileName(target.File))
            .ToArray();

        if (force && forced.Length > 0)
        {
            _dialogs.Show(
                new Dialog(
                    RolaI18N.Get("rorolala_vcs.version_graph.track_force_title"),
                    RolaI18N.Get(
                        "rorolala_vcs.version_graph.track_force_message",
                        string.Join(", ", forced)
                    ),
                    () => AskDiverged(message, targets, force)
                )
            );

            return;
        }

        AskDiverged(message, targets, force);
    }

    /// <summary>Asks about a version that would part from the Vault, then runs the track.</summary>
    /// <param name="message">What the versions are to say.</param>
    /// <param name="targets">The files to record.</param>
    /// <param name="force">Whether the run records changes the work would otherwise refuse.</param>
    private void AskDiverged(string message, IReadOnlyList<Target> targets, bool force)
    {
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
                    () => Run(message, targets, force)
                )
            );

            return;
        }

        Run(message, targets, force);
    }

    /// <summary>Runs the track over the files the question was answered for.</summary>
    /// <param name="message">What the versions are to say.</param>
    /// <param name="targets">The files to record, read when the question was put.</param>
    /// <param name="force">Whether the run records changes the work would otherwise refuse.</param>
    private void Run(string message, IReadOnlyList<Target> targets, bool force)
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
            var (done, said) = TrackCommand.Run(directory, files, message, force);

            Dispatcher.UIThread.Post(() =>
            {
                _busy = false;
                _hub.End(run);

                if (!done)
                {
                    FailureNotice.Show(
                        _dialogs,
                        _icons,
                        RolaI18N.Get("rorolala_vcs.version_graph.track_failed"),
                        said
                    );

                    Paint();

                    return;
                }

                // What was written is now a version, so the field starts again empty and every view of
                // the work, the graph among them, is told that the files may have changed.
                _message.Text = string.Empty;
                _files.Touch();
                Paint();
            });
        });
    }

    /// <summary>
    /// The files a track would record: those chosen whose content changed, less the ones that need no
    /// recording or that only a forced run may record, each with whether its chain has parted from the
    /// Vault's.
    /// </summary>
    /// <remarks>
    /// A file that stands as recorded is nothing to make a version of, and the command line skips it;
    /// skipping it here is what makes the button's answer the same as the run's. A new file is work to
    /// record; a file whose base upstream moved past is too, and is the one the question is put for.
    /// A file another account holds is left out unless the run is forced, because the version made here
    /// would be one this side cannot push. A move that changed nothing and a file waiting to be merged
    /// are different actions' to settle rather than a track's.
    /// </remarks>
    /// <param name="force">Whether the files a plain track refuses are to be recorded anyway.</param>
    /// <returns>The files to record, in the order the listing shows them.</returns>
    private IReadOnlyList<Target> Targets(bool force)
    {
        // A directory is a place, and what a track on it means is every untracked entry under it.
        // That is asked before the file choice because the click that made the place was about the
        // place: a listing's previous files are not what the reader is pointing at any more. A
        // directory inside a tree is the tree's work, so it is the tree that is recorded.
        if (ChosenDirectory.Current is { } place)
        {
            if (RolaTrees.RootOf(place.Path) is { } tree)
            {
                return TreeTarget(tree, force) is { } target ? [target] : [];
            }

            return DirectoryTargets(place, force);
        }

        var chosen = ChosenFiles.Current;

        IReadOnlyList<Entry> considered = chosen.Count > 0
            ? chosen
            : ChosenEntry.Current is { } one
                ? [one]
                : [];

        var targets = new List<Target>(considered.Count);
        var trees = new HashSet<string>(
            OperatingSystem.IsWindows() ? StringComparer.OrdinalIgnoreCase : StringComparer.Ordinal
        );

        foreach (var entry in considered)
        {
            // A file or a directory inside a tree is the tree's work, and the tree is one entry:
            // tracking any of them is tracking the tree once, rather than the path the click landed on.
            if (RolaTrees.RootOf(entry.Path) is { } tree)
            {
                if (trees.Add(tree) && TreeTarget(tree, force) is { } treeTarget)
                {
                    targets.Add(treeTarget);
                }

                continue;
            }

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

                // A change either account may record, a file the Layout does not name yet, and a file
                // waiting for a variant to be joined into it, recording the last one is what ends the
                // merge. The merge itself is not a change: only a file the tree changed while it waits
                // has something to record, and the command line would record nothing for one that did
                // not. A change against a base upstream moved past is recordable too, and that is the
                // one the question is put for.
                var recordable = state switch
                {
                    PathState.Modified or PathState.Untagged => true,
                    PathState.Merging => status.Modified.Any(named =>
                        StatusReadings.Names(status.Root, named, entry.Path)
                    ),
                    PathState.Wrong => force || !status.Unowned.Any(named =>
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
                            status.SyncOf(entry.Path) == SyncStanding.Diverged,
                            state == PathState.Wrong
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

    /// <summary>The target a track on a tree would record, when the tree has work to record.</summary>
    /// <remarks>
    /// A tree is one entry, so the run names the directory rather than a path inside it. Its work is
    /// what its own status says is staged, changed, untracked or in conflict, and a staged description
    /// is work even when every path still agrees with the record.
    /// </remarks>
    /// <param name="tree">The tree's directory.</param>
    /// <param name="force">Whether a tree another account holds may be recorded anyway.</param>
    /// <returns>The tree as one target, or nothing when it has nothing to record.</returns>
    private Target? TreeTarget(string tree, bool force)
    {
        var reading = _statuses.Reading(tree);
        var status = TreeStatus(tree);
        var described = File.Exists(Path.Combine(tree, ".rola-tree", "description"));

        if ((status is null || status.Clean) && !described)
        {
            return null;
        }

        var unowned =
            reading is { } read
            && read.Unowned.Any(named => StatusReadings.Names(read.Root, named, tree));

        if (unowned && !force)
        {
            return null;
        }

        var diverged =
            reading is { } standing && standing.SyncOf(tree) == SyncStanding.Diverged;

        return new Target(tree, tree, diverged, unowned, Tree: true);
    }

    /// <summary>What a tree stands as, read once between file changes.</summary>
    /// <remarks>
    /// A tree's status walks the tree, so it is read on a thread of its own like the Work's own status.
    /// Nothing is answered until it lands, and a drawing that asked in the meantime has no target and
    /// keeps the button still; what lands is drawn again.
    /// </remarks>
    /// <param name="tree">The tree's directory.</param>
    /// <returns>The reading, or nothing while it is being read or when it cannot be.</returns>
    private RolaTreeStatus? TreeStatus(string tree)
    {
        if (_trees.TryGetValue(tree, out var known))
        {
            return known;
        }

        if (!_askingTrees.Add(tree))
        {
            return null;
        }

        var generation = _treeGeneration;

        _ = Task.Run(() =>
        {
            var status = RolaTreeStatus.Locate(tree);

            Dispatcher.UIThread.Post(() =>
            {
                _askingTrees.Remove(tree);

                if (generation == _treeGeneration)
                {
                    _trees[tree] = status;
                }

                Paint();
            });
        });

        return null;
    }

    /// <summary>The files a track on a directory would record: every recordable entry under it.</summary>
    /// <remarks>
    /// The reading is taken for the directory the place sits in rather than for the place itself:
    /// the listing has already read the directory it shows, while a place that has just been clicked
    /// has not been read and would answer nothing until a read made for it came back. What the
    /// reading names is relative to the Workspace, so the place is made absolute here before the
    /// entries are sifted by it.
    /// <para>
    /// What is considered is the reading's own lists, the untracked, the modified, the wrong-based,
    /// and the files a merge is waiting for, rather than every file under the directory: a directory
    /// of a thousand files with one edit is one track, and walking the tree to find the edit is work
    /// the reading has already done. What each candidate then means is the same answer a listing's
    /// row is acted on by, so a directory and a file choice cannot disagree about what may be recorded.
    /// </para>
    /// </remarks>
    /// <param name="place">The directory clicked in a listing.</param>
    /// <param name="force">Whether the files a plain track refuses are to be recorded anyway.</param>
    /// <returns>The files under it a track would record.</returns>
    private IReadOnlyList<Target> DirectoryTargets(Entry place, bool force)
    {
        var holder = Path.GetDirectoryName(place.Path) ?? place.Path;
        var status = _statuses.Reading(holder) ?? _statuses.Reading(place.Path);

        if (status is not { } read)
        {
            return [];
        }

        var directory = Path.GetFullPath(place.Path);
        var targets = new List<Target>();
        var trees = new HashSet<string>(
            OperatingSystem.IsWindows() ? StringComparer.OrdinalIgnoreCase : StringComparer.Ordinal
        );

        foreach (var file in RecordableUnder(read, directory, force))
        {
            // A file under a tree is the tree's work, so a directory choice that reaches one records
            // the tree once rather than each changed file in it.
            if (RolaTrees.RootOf(file) is { } tree)
            {
                if (trees.Add(tree) && TreeTarget(tree, force) is { } target)
                {
                    targets.Add(target);
                }

                continue;
            }

            targets.Add(
                new Target(
                    directory,
                    file,
                    read.SyncOf(file) == SyncStanding.Diverged,
                    read.StateOf(file) == PathState.Wrong
                )
            );
        }

        return targets;
    }

    /// <summary>The files under one directory a track would record.</summary>
    /// <remarks>
    /// The reading's own lists are what is sifted rather than the whole tree: a directory of a
    /// thousand files with one edit is one track, and walking the tree to find the edit is work the
    /// reading has already done. What each candidate then means is the same answer a listing's row
    /// is acted on by, so a directory and a file choice cannot disagree about what may be recorded.
    /// </remarks>
    /// <param name="read">What the work says about itself.</param>
    /// <param name="directory">The place whose entries are wanted.</param>
    /// <param name="force">Whether the files a plain track refuses are to be recorded anyway.</param>
    /// <returns>The full paths to record, in the order the reading names them.</returns>
    internal static IReadOnlyList<string> RecordableUnder(
        RolaStatus read,
        string directory,
        bool force = false
    )
    {
        ArgumentNullException.ThrowIfNull(read);
        ArgumentNullException.ThrowIfNull(directory);

        var root = Path.GetFullPath(directory);
        if (!root.EndsWith(Path.DirectorySeparatorChar))
        {
            root += Path.DirectorySeparatorChar;
        }

        var comparison = OperatingSystem.IsWindows()
            ? StringComparison.OrdinalIgnoreCase
            : StringComparison.Ordinal;
        var considered = new HashSet<string>(
            read.Untagged.Concat(read.Modified).Concat(read.WrongBase),
            comparison == StringComparison.OrdinalIgnoreCase
                ? StringComparer.OrdinalIgnoreCase
                : StringComparer.Ordinal
        );

        foreach (var merge in read.Merging)
        {
            if (merge.Target.Length > 0)
            {
                considered.Add(merge.Target);
            }
        }

        var targets = new List<string>();

        foreach (var named in considered)
        {
            var file = Path.GetFullPath(Path.Combine(read.Root, named));

            if (!file.StartsWith(root, comparison))
            {
                continue;
            }

            var state = read.StateOf(file);

            // The same states a chosen file is offered a track for. A wrong-based file another account
            // holds is not this account's to record, and a merge is only a change where the file under
            // it has changed.
            var recordable = state switch
            {
                PathState.Modified or PathState.Untagged => true,
                PathState.Merging => read.Modified.Any(named =>
                    StatusReadings.Names(read.Root, named, file)
                ),
                PathState.Wrong => force || !read.Unowned.Any(named =>
                    StatusReadings.Names(read.Root, named, file)
                ),
                _ => false,
            };

            if (recordable)
            {
                targets.Add(file);
            }
        }

        return targets;
    }

    /// <summary>Draws the button as the files chosen and what is written say it is.</summary>
    private void Paint()
    {
        var targets = Targets(_shifted);
        var ready = !_busy && targets.Count > 0 && !string.IsNullOrWhiteSpace(_message.Text);
        var held = _shifted && ready;

        _track.IsEnabled = ready;

        // The button keeps the look's ink while it can be pressed and the faint one while it cannot:
        // what the row offers is said by the weight of the word as well as by the press being refused.
        // Shift makes the run a forced one, so the word and the line are drawn in the colour the look
        // spends on what must not be done.
        _track[!TemplatedControl.ForegroundProperty] = new DynamicResourceExtension(
            held ? Held : ready ? PaletteKeys.Front : PaletteKeys.Faint
        );
        _track[!TemplatedControl.BorderBrushProperty] = new DynamicResourceExtension(
            held ? Held : PaletteKeys.Splitter
        );

        if (held)
        {
            _track[!TemplatedControl.BackgroundProperty] = new DynamicResourceExtension(
                PaletteKeys.ForceWash
            );
        }
        else
        {
            _track.Background = Brushes.Transparent;
        }

        var tip = targets.Any(target => target.Tree)
            ? RolaI18N.Get("rorolala_vcs.version_graph.track_tree_tip")
            : ChosenDirectory.Current is null
                ? RolaI18N.Get("rorolala_vcs.version_graph.track_tip")
                : RolaI18N.Get("rorolala_vcs.version_graph.track_directory_tip");

        if (_shifted)
        {
            tip = $"{tip}, {RolaI18N.Get("rorolala_vcs.version_graph.track_force_hint")}";
        }

        if (targets.Any(target => target.Diverged))
        {
            tip = $"{tip}, {RolaI18N.Get("rorolala_vcs.version_graph.track_diverged")}";
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
    /// <param name="Forced">Whether it is a change a plain track refuses and only a forced one records.</param>
    /// <param name="Tree">Whether the target is a rola-tree rather than a file.</param>
    private sealed record Target(
        string Directory,
        string File,
        bool Diverged,
        bool Forced,
        bool Tree = false
    );
}
