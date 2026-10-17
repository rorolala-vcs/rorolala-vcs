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
/// What the commands this plugin runs are doing, as one card per command down the right of the Overview.
/// </summary>
/// <remarks>
/// Every command this plugin runs — a sync, a claim, a release, and whatever is added later — reports itself as
/// a run gathered in the <see cref="ProgressHub"/>, and each is drawn as a card of its own: the command's name,
/// and a bar for every task it says it is doing. A reader watching the work sees which commands are going and
/// how far each has got, rather than one bar that would have to stand for all of them at once.
/// <para>
/// A card is taken away the moment its command ends, and the room the panel stands in is given back to the table
/// when the last one goes, so an Overview with nothing running is the table and nothing else.
/// </para>
/// <para>
/// A card is drawn once and brought up to date rather than rebuilt on every record: a bar is a control whose fill
/// tweens towards a new value, and a bar made afresh each time would slide from nothing on every step instead of
/// from where it already was.
/// </para>
/// </remarks>
internal sealed class ProgressPanel : Border
{
    /// <summary>How tall a row's bar is, which is tall enough to hold the line drawn inside it.</summary>
    private const double BarHeight = 18;

    /// <summary>How wide the name column is, fixed so that every bar of a card starts at the same place.</summary>
    private const double NameWidth = 140;

    /// <summary>
    /// How wide the percentage column is, fixed so that a percentage changing its number of digits does not
    /// change the length of the bar beside it.
    /// </summary>
    /// <remarks>
    /// Wide enough for `100%` in the look's own type: the column is not sized to what a row currently says, since
    /// what it says changes as the run works and a bar that grew and shrank with it would be a bar a reader cannot
    /// read at a glance.
    /// </remarks>
    private const double ShareWidth = 48;

    /// <summary>How far a name is inset where its bar stands under another's.</summary>
    private const double Indent = 12;

    /// <summary>Where the cards go, one per run the hub holds, in the order they began.</summary>
    private readonly StackPanel _cards = new() { Spacing = 10 };

    /// <summary>The card drawn for each run that is being shown.</summary>
    /// <remarks>
    /// Keyed by the run itself, because a run is what says the same card over again: the panel is handed a fresh
    /// list on every change, but the runs in it are the ones it drew last time, and matching on them is what lets
    /// a card be found rather than made anew.
    /// </remarks>
    private readonly Dictionary<ProgressRun, Card> _drawn = new(ReferenceEqualityComparer.Instance);

    /// <summary>What the panel is drawn from.</summary>
    private readonly ProgressHub _hub;

    /// <summary>Makes the panel.</summary>
    /// <param name="hub">What the panel is drawn from.</param>
    public ProgressPanel(ProgressHub hub)
    {
        ArgumentNullException.ThrowIfNull(hub);

        _hub = hub;

        Padding = new Thickness(10, 8);

        // The rule down the left is the look's own line between two things, drawn like the band above the table:
        // the two are one edge each of the same table, and a panel that drew its own colour would be a seam.
        BorderThickness = new Thickness(1, 0, 0, 0);
        this[!BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

        Child = new ScrollViewer
        {
            Content = _cards,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
        };

        IsVisible = false;

        // The runs are followed only while the panel is on screen: a panel that is not drawn has nothing to draw
        // for, and a handler left on a hub that outlives it would keep the panel alive for nothing.
        AttachedToVisualTree += (_, _) =>
        {
            _hub.Changed += Redraw;
            Draw();
        };

        DetachedFromVisualTree += (_, _) => _hub.Changed -= Redraw;

        Draw();
    }

    /// <summary>Draws again, which is what every change to the runs asks for.</summary>
    /// <param name="sender">What raised it.</param>
    /// <param name="changed">The event.</param>
    private void Redraw(object? sender, EventArgs changed)
    {
        _ = sender;
        _ = changed;

        Draw();
    }

    /// <summary>
    /// Draws a card for every run the hub holds, and takes away the cards of the runs it no longer does.
    /// </summary>
    /// <remarks>
    /// The cards already drawn are kept and brought up to date; one whose run has gone is taken away, and one for
    /// a run that has begun since is given a card of its own at the end, which is where the hub opened it, so the
    /// order of the cards is the order of the commands without one ever having to be moved.
    /// </remarks>
    internal void Draw()
    {
        var runs = _hub.Runs;

        IsVisible = runs.Count > 0;

        var live = new HashSet<ProgressRun>(runs, ReferenceEqualityComparer.Instance);

        foreach (var gone in _drawn.Keys.Where(run => !live.Contains(run)).ToArray())
        {
            _cards.Children.Remove(_drawn[gone].Root);
            _drawn.Remove(gone);
        }

        foreach (var run in runs)
        {
            if (!_drawn.TryGetValue(run, out var card))
            {
                card = new Card();
                _drawn[run] = card;
                _cards.Children.Add(card.Root);
            }

            card.Draw(run);
        }
    }

    /// <summary>What a task is called: its own name when it has one, and its direction when it has not.</summary>
    /// <param name="task">The task.</param>
    /// <returns>What to write at the start of its bar.</returns>
    private static string Head(ProgressTask task) =>
        task.What is { Length: > 0 } named
            ? named
            : task.Aim switch
            {
                MoveAim.Up => "↑ " + RolaI18N.Get("rorolala_vcs.progress.row_up"),
                MoveAim.Down => "↓ " + RolaI18N.Get("rorolala_vcs.progress.row_down"),
                _ => RolaI18N.Get("rorolala_vcs.progress.working"),
            };

    /// <summary>How far something has got, as a percentage, or nothing when its length was never said.</summary>
    /// <param name="done">How much is behind it.</param>
    /// <param name="total">How long it is in all, or nothing when that was never said.</param>
    /// <returns>What to write at the end of the bar, or nothing.</returns>
    private static string Share(long done, long? total) =>
        total switch
        {
            null => string.Empty,
            0 => "100%",
            { } length => ((done * 100) / length).ToString(CultureInfo.InvariantCulture) + "%",
        };

    /// <summary>
    /// One command's card: its name over a bar for the whole of it, and a bar for each task under it.
    /// </summary>
    /// <remarks>
    /// The name is the first bar's own caption rather than a title above it, so that every line of the card reads
    /// the same way and nothing is said twice. The tasks that are part of another stand below the whole and are
    /// inset, so that what is the command's own progress is told apart from what is being done under it.
    /// </remarks>
    private sealed class Card
    {
        /// <summary>Where the bars go, the whole of the command first and its tasks under it.</summary>
        private readonly StackPanel _rows = new() { Spacing = 6 };

        /// <summary>The bar for the whole of the command, which is the sum of its parts that are its own.</summary>
        private readonly Bar _whole = new(indent: false);

        /// <summary>The bar drawn for each task that is part of another.</summary>
        private readonly Dictionary<ProgressTask, Bar> _parts = new(ReferenceEqualityComparer.Instance);

        /// <summary>The card, which is the rows inside a frame.</summary>
        private readonly Border _root;

        /// <summary>Makes a card with nothing but the bar for the whole of a command in it.</summary>
        public Card()
        {
            _root = new Border
            {
                // A card is a section rather than a raised surface: nothing is rounded and nothing is inset, and
                // the one line drawn is the rule under it, which is what tells one command's card from the next.
                CornerRadius = new CornerRadius(0),
                Margin = new Thickness(0),
                BorderThickness = new Thickness(0, 0, 0, 1),
                Padding = new Thickness(8),
                Child = _rows,
            };

            _root[!Border.BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

            _rows.Children.Add(_whole.Root);
        }

        /// <summary>The card, for the panel to hold.</summary>
        public Control Root => _root;

        /// <summary>Draws a run, bringing every bar of the card to what the run now is.</summary>
        /// <param name="run">The run.</param>
        public void Draw(ProgressRun run)
        {
            var tasks = run.Records.Tasks;

            // What is the command's own is a task that is part of nothing: the phases of a sync, the one batch of
            // a claim. What is done under one of those — a direction of a transfer — is a bar of its own below.
            var whole = tasks.Where(task => task.Parent is null).ToArray();
            var parts = tasks.Where(task => task.Parent is not null).ToArray();

            DrawWhole(run, whole);
            DrawParts(parts);
        }

        /// <summary>Draws the bar for the whole of the command, which is its own tasks added up.</summary>
        /// <param name="run">The run, whose name captions the bar.</param>
        /// <param name="whole">The tasks that are the command's own.</param>
        private void DrawWhole(ProgressRun run, IReadOnlyList<ProgressTask> whole)
        {
            var done = 0L;
            var total = 0L;
            var length = whole.Count > 0;
            string? moving = null;

            foreach (var task in whole)
            {
                done += task.Done;

                if (task.Total is { } amount)
                {
                    total += amount;
                }
                else
                {
                    // One phase that has not said how long it is makes the sum unknown, and a bar drawn to a
                    // length nobody stated would be a bar telling the reader less than nothing.
                    length = false;
                }

                moving ??= task.Moving;
            }

            _whole.Draw(
                run.Title,
                Share(done, length ? total : null),
                done,
                length ? total : null,
                moving
            );
        }

        /// <summary>Draws a bar for each task done under the command's own, keeping the bars it drew before.</summary>
        /// <param name="parts">The tasks that are part of another.</param>
        private void DrawParts(IReadOnlyList<ProgressTask> parts)
        {
            foreach (var task in parts)
            {
                if (!_parts.TryGetValue(task, out var bar))
                {
                    bar = new Bar(indent: true);
                    _parts[task] = bar;
                    _rows.Children.Add(bar.Root);
                }

                bar.Draw(Head(task), Share(task.Done, task.Total), task.Done, task.Total, task.Moving);
            }
        }
    }

    /// <summary>
    /// One bar of a card: a name, a bar with what is being moved drawn inside it, and how far it has got.
    /// </summary>
    /// <remarks>
    /// All three on one line, so that a card of several bars is a list rather than a stack of small paragraphs.
    /// What is being moved is drawn inside the bar rather than beside it, over the left of it and clipped to a
    /// third of its width: a name long enough to matter would otherwise push the percentage off the line, and
    /// what a reader wants of it is only the head of it — see how the command line itself shortens a key.
    /// </remarks>
    private sealed class Bar
    {
        /// <summary>What the task is called, at the start of the line.</summary>
        private readonly TextBlock _name = new()
        {
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };

        /// <summary>The name's column, which is the width every bar of a card lines up by.</summary>
        /// <remarks>
        /// A fixed width rather than the text's: a name sized to its own content would put the bar after it
        /// wherever the name ended, and no two rows of a card would start their bars together. The indent of a bar
        /// under another is this cell's padding, so that it moves the name and not the columns beside it.
        /// </remarks>
        private readonly Border _cell;

        /// <summary>How far it has got, at the end of the line.</summary>
        /// <remarks>
        /// Right-aligned in a column of its own width, so that one digit more or fewer moves nothing: the bar's
        /// other edge is the column's, not the text's.
        /// </remarks>
        private readonly TextBlock _share = new()
        {
            Classes = { "muted" },
            Width = ShareWidth,
            TextAlignment = TextAlignment.Right,
            VerticalAlignment = VerticalAlignment.Center,
        };

        /// <summary>What is being moved, drawn over the left of the bar.</summary>
        private readonly TextBlock _moving = new()
        {
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
            FontSize = 11,
            Margin = new Thickness(4, 0, 0, 0),
        };

        /// <summary>The bar itself.</summary>
        private readonly ProgressBar _fill = new()
        {
            Height = BarHeight,
            MinHeight = 0,
            VerticalAlignment = VerticalAlignment.Center,
        };

        /// <summary>The line, which is the name, the bar with what it is moving, and the percentage.</summary>
        private readonly Grid _root;

        /// <summary>Makes a bar, inset where it is one of the tasks under a command's own.</summary>
        /// <param name="indent">Whether it stands under another bar rather than at the head of a card.</param>
        public Bar(bool indent)
        {
            // What is moving is drawn inside the bar, so the bar and the piece share a grid: the bar spans both
            // shares and the piece takes the left third of it, which is the width it is clipped to.
            var inside = new Grid { ColumnDefinitions = new ColumnDefinitions("35*,65*") };

            Grid.SetColumnSpan(_fill, 2);
            Grid.SetColumn(_moving, 0);
            inside.Children.Add(_fill);
            inside.Children.Add(_moving);

            _root = new Grid
            {
                ColumnDefinitions = new ColumnDefinitions("Auto,*,Auto"),
                ColumnSpacing = 6,
            };

            _cell = new Border
            {
                Width = NameWidth,
                VerticalAlignment = VerticalAlignment.Center,
                Padding = indent ? new Thickness(Indent, 0, 0, 0) : default,
                Child = _name,
            };

            Grid.SetColumn(_cell, 0);
            Grid.SetColumn(inside, 1);
            Grid.SetColumn(_share, 2);
            _root.Children.Add(_cell);
            _root.Children.Add(inside);
            _root.Children.Add(_share);
        }

        /// <summary>The line, for the card to hold.</summary>
        public Control Root => _root;

        /// <summary>Draws the bar, bringing every part of the line to what the task now is.</summary>
        /// <param name="name">What it is called.</param>
        /// <param name="share">How far it has got, as a percentage, or nothing when its length is unknown.</param>
        /// <param name="done">How much is behind it.</param>
        /// <param name="total">How long it is in all, or nothing when that was never said.</param>
        /// <param name="moving">What is being moved, or nothing when nothing is.</param>
        public void Draw(string name, string share, long done, long? total, string? moving)
        {
            _name.Text = name;
            _share.Text = share;
            _moving.Text = moving ?? string.Empty;

            if (total is { } length)
            {
                // A task with nothing to do is a task that is already done, which is what the command line draws
                // for one: a bar of no length is full rather than empty. The value is written at once — the bar's
                // own look tweens the fill towards it, so what is said here is where it is going.
                _fill.IsIndeterminate = false;
                _fill.Maximum = Math.Max(length, 1);
                _fill.Value = length > 0 ? Math.Clamp(done, 0, length) : 1;
            }
            else
            {
                // A task that has not said how long it is is drawn as a bar that moves rather than as one at
                // nothing done, which would read as no progress at all.
                _fill.IsIndeterminate = true;
            }
        }
    }
}
