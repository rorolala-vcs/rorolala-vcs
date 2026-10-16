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
/// </remarks>
internal sealed class SyncPanel : Border
{
    /// <summary>How tall a row's bar is, which is the measure the band above the table uses for its own.</summary>
    private const double BarHeight = 4;

    /// <summary>Where the rows go, one per task the run has opened and not yet finished.</summary>
    private readonly StackPanel _rows = new() { Spacing = 10 };

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
    /// </remarks>
    /// <param name="progress">What the run has said of itself.</param>
    /// <param name="running">Whether a run is going at all.</param>
    internal void Draw(SyncProgress progress, bool running)
    {
        ArgumentNullException.ThrowIfNull(progress);

        IsVisible = running;
        _rows.Children.Clear();

        if (!running)
        {
            return;
        }

        foreach (var task in progress.Tasks)
        {
            _rows.Children.Add(Row(task));
        }
    }

    /// <summary>One task as a row: what it is called, how far it has got, and what it is moving.</summary>
    /// <param name="task">The task.</param>
    /// <returns>The row.</returns>
    private static Control Row(SyncTask task)
    {
        var head = new TextBlock
        {
            Text = Head(task),
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };

        var share = new TextBlock
        {
            Text = Share(task),
            Classes = { "muted" },
            VerticalAlignment = VerticalAlignment.Center,
        };

        var line = new Grid { ColumnDefinitions = new ColumnDefinitions("*,Auto") };
        Grid.SetColumn(head, 0);
        Grid.SetColumn(share, 1);
        line.Children.Add(head);
        line.Children.Add(share);

        var row = new StackPanel { Spacing = 3, Children = { line, Bar(task) } };

        // What is being moved stands under the bar rather than beside the name, because it is a path and a path is
        // read on a line of its own: a name that shared the line above it would be squeezed into nothing on a
        // narrow panel and cut off where it matters.
        if (task.Moving is { Length: > 0 } moving)
        {
            row.Children.Add(
                new TextBlock
                {
                    Text = moving,
                    Classes = { "muted" },
                    TextTrimming = TextTrimming.CharacterEllipsis,
                }
            );
        }

        return row;
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

    /// <summary>A task's bar: how far it has got, or one that moves while its length is unknown.</summary>
    /// <param name="task">The task.</param>
    /// <returns>The bar.</returns>
    private static ProgressBar Bar(SyncTask task)
    {
        var bar = new ProgressBar { Height = BarHeight, VerticalAlignment = VerticalAlignment.Center };

        if (task.Total is not { } total)
        {
            // A task that has not said how long it is — the first record of one has not necessarily been read
            // yet — is drawn as a bar that moves rather than as one at nothing done, which would read as no
            // progress at all.
            bar.IsIndeterminate = true;
        }
        else
        {
            // A task with nothing to do is a task that is already done, which is what the command line draws for
            // one: a bar of no length is full rather than empty.
            bar.Maximum = Math.Max(total, 1);
            bar.Value = total > 0 ? Math.Clamp(task.Done, 0, total) : 1;
        }

        return bar;
    }
}
