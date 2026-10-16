using System.Globalization;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// What a run is doing, as one row per bar the run reports, down the right of the Overview.
/// </summary>
/// <remarks>
/// The command line draws its progress as a stack of bars — each task over the tasks it is part of — and this is
/// that stack drawn as a panel rather than as a terminal: one row per task, with what it is called, how far it
/// has got, and what it is moving right now. It stands beside the work rather than over it, so that the table
/// and the run are read at once.
/// <para>
/// Nothing is drawn while no run is going, and the room it stands in is given back to the table then: a panel
/// that stayed, empty, would be a strip down the Overview that says nothing.
/// </para>
/// <para>
/// A task keeps the row it was first drawn as for as long as it is going, and only its contents are drawn
/// again. That is not an economy: a row rebuilt on every record is a bar with no past, so its fill would be read
/// as dropping back to nothing each time a step is said, and the tween it moves with — which is what turns one
/// said step into a slide — would begin again from that nothing rather than from where the bar already was.
/// </para>
/// </remarks>
internal sealed class SyncPanel : Border
{
    /// <summary>How tall a row's bar is, which is the measure the band above the table uses for its own.</summary>
    private const double BarHeight = 4;

    /// <summary>Where the rows go, one per task the run has opened and not yet finished.</summary>
    private readonly StackPanel _rows = new() { Spacing = 10 };

    /// <summary>The row drawn for each task that is still going.</summary>
    /// <remarks>
    /// Keyed by the task itself, because a task is what says the same bar over again: the panel is handed a
    /// fresh list on every record, but the tasks in it are the ones it drew last time, and matching on them is
    /// what lets a row be found rather than made anew.
    /// </remarks>
    private readonly Dictionary<SyncTask, Row> _drawn = new(ReferenceEqualityComparer.Instance);

    /// <summary>Makes the panel.</summary>
    /// <param name="sync">What the panel is drawn from.</param>
    public SyncPanel(SyncWatcher sync)
    {
        ArgumentNullException.ThrowIfNull(sync);

        Padding = new Thickness(10, 8);

        // The rule down the left is the look's own line between two things, drawn like the band above the table:
        // the two are one edge each of the same table, and a panel that drew its own colour would be a seam.
        BorderThickness = new Thickness(1, 0, 0, 0);
        this[!BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

        Child = new ScrollViewer
        {
            Content = _rows,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
        };

        IsVisible = false;

        sync.Changed += (_, _) => Draw(sync.Progress, sync.Running);

        Draw(sync.Progress, running: false);
    }

    /// <summary>
    /// Draws what a run has said of itself, or nothing at all while none is going.
    /// </summary>
    /// <remarks>
    /// What to draw is handed over rather than read from the state, so that the drawing is one answer to one
    /// question: rows that asked how far a run had got and whether it was going at once could be drawn between
    /// the two and show a run that had ended.
    /// <para>
    /// The rows already drawn are kept and brought up to date: a task that was here last time keeps its row and
    /// its bar — see the class remarks for why that matters — one that has opened since is given a row of its
    /// own, and one that is over has its row taken away.
    /// </para>
    /// </remarks>
    /// <param name="progress">What the run has said of itself.</param>
    /// <param name="running">Whether a run is going at all.</param>
    internal void Draw(SyncProgress progress, bool running)
    {
        ArgumentNullException.ThrowIfNull(progress);

        IsVisible = running;

        if (!running)
        {
            _rows.Children.Clear();
            _drawn.Clear();

            return;
        }

        var tasks = progress.Tasks;

        // A task that is over is taken off, the way the command line takes its bar off the terminal: what is
        // left is what is still going, and a row that stayed after its work had ended would be a row read for
        // nothing.
        var going = new HashSet<SyncTask>(tasks, ReferenceEqualityComparer.Instance);

        foreach (var over in _drawn.Keys.Where(task => !going.Contains(task)).ToArray())
        {
            _rows.Children.Remove(_drawn[over].Root);
            _drawn.Remove(over);
        }

        // What is left is drawn in the run's own order. A task the run has opened since last time takes its row
        // at the end, which is where the run opened it, so the order of the rows is the order of the tasks
        // without a row ever having to be moved.
        foreach (var task in tasks)
        {
            if (!_drawn.TryGetValue(task, out var row))
            {
                row = new Row();
                _drawn[task] = row;
                _rows.Children.Add(row.Root);
            }

            row.Draw(task);
        }
    }

    /// <summary>What a task is called: its own name when it has one, and its direction when it has not.</summary>
    /// <param name="task">The task.</param>
    /// <returns>What to write above its bar.</returns>
    private static string Head(SyncTask task) =>
        task.What is { Length: > 0 } named
            ? named
            : task.Aim switch
            {
                SyncAim.Up => "↑ " + RolaI18N.Get("rorolala_vcs.sync.row_up"),
                SyncAim.Down => "↓ " + RolaI18N.Get("rorolala_vcs.sync.row_down"),
                _ => RolaI18N.Get("rorolala_vcs.sync.working"),
            };

    /// <summary>How far a task has got, as a percentage, or nothing when its length was never said.</summary>
    /// <param name="task">The task.</param>
    /// <returns>What to write beside its name.</returns>
    private static string Share(SyncTask task) =>
        task.Total is { } total && total > 0
            ? ((task.Done * 100) / total).ToString(CultureInfo.InvariantCulture) + "%"
            : string.Empty;

    /// <summary>
    /// One task's row, kept for as long as the run keeps the task.
    /// </summary>
    /// <remarks>
    /// The controls are made once and drawn again rather than remade: the bar especially, since it is the one
    /// thing whose smoothness depends on being the same control from one record to the next.
    /// </remarks>
    private sealed class Row
    {
        /// <summary>What the task is called, at the start of the line above the bar.</summary>
        private readonly TextBlock _head = new()
        {
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };

        /// <summary>How far it has got, at the end of that line.</summary>
        private readonly TextBlock _share = new()
        {
            Classes = { "muted" },
            VerticalAlignment = VerticalAlignment.Center,
        };

        /// <summary>The bar itself.</summary>
        private readonly ProgressBar _bar = new()
        {
            Height = BarHeight,
            VerticalAlignment = VerticalAlignment.Center,
        };

        /// <summary>The row, which is a line and the bar under it, and the moving piece when there is one.</summary>
        private readonly StackPanel _root;

        /// <summary>What is being moved, or nothing while nothing is.</summary>
        private TextBlock? _moving;

        /// <summary>Makes a row with nothing in it.</summary>
        public Row()
        {
            var line = new Grid { ColumnDefinitions = new ColumnDefinitions("*,Auto") };
            Grid.SetColumn(_head, 0);
            Grid.SetColumn(_share, 1);
            line.Children.Add(_head);
            line.Children.Add(_share);

            _root = new StackPanel { Spacing = 3, Children = { line, _bar } };
        }

        /// <summary>The row, for the panel to hold.</summary>
        public Control Root => _root;

        /// <summary>Draws a task, bringing every part of the row to what the task now is.</summary>
        /// <param name="task">The task.</param>
        public void Draw(SyncTask task)
        {
            _head.Text = Head(task);
            _share.Text = Share(task);

            if (task.Total is not { } total)
            {
                // A task that has not said how long it is — the first record of one has not necessarily been
                // read yet — is drawn as a bar that moves rather than as one at nothing done, which would read
                // as no progress at all.
                _bar.IsIndeterminate = true;
            }
            else
            {
                // A task with nothing to do is a task that is already done, which is what the command line
                // draws for one: a bar of no length is full rather than empty. The value is written at once —
                // the bar's own look tweens the fill towards it, so what is said here is where it is going.
                _bar.IsIndeterminate = false;
                _bar.Maximum = Math.Max(total, 1);
                _bar.Value = total > 0 ? Math.Clamp(task.Done, 0, total) : 1;
            }

            Settle(task.Moving);
        }

        /// <summary>
        /// Shows, changes or takes away the line naming what is being moved.
        /// </summary>
        /// <remarks>
        /// The line is put in only while there is something to say, rather than left in empty: an empty line
        /// would keep the room the line needs and hold every row apart by the height of nothing.
        /// </remarks>
        /// <param name="moving">What is being moved, or nothing.</param>
        private void Settle(string? moving)
        {
            var wanted = moving is { Length: > 0 };

            if (wanted && _moving is null)
            {
                _moving = new TextBlock
                {
                    Classes = { "muted" },
                    TextTrimming = TextTrimming.CharacterEllipsis,
                };
                _root.Children.Add(_moving);
            }
            else if (!wanted && _moving is not null)
            {
                _root.Children.Remove(_moving);
                _moving = null;
            }

            if (_moving is not null)
            {
                _moving.Text = moving;
            }
        }
    }
}
