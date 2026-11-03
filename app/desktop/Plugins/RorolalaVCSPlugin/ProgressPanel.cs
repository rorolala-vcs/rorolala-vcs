using System.Globalization;
using Avalonia;
using Avalonia.Animation;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Threading;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// What the commands this plugin runs are doing, as a stack of capsules at the bottom right of the Overview.
/// </summary>
/// <remarks>
/// A capsule is one command: what it is called, how far its own phases have got, and — under the pointer — the
/// phases themselves. The stack floats over the reading rather than taking a column of it, so the table keeps
/// its width whether a run is going or not; a command that appears fades into the stack, and one that ends
/// fades out of it.
/// <para>
/// The runs come from the <see cref="ProgressHub"/>, which outlives every Overview: a run begun from a menu has
/// to reach a panel that may not exist yet, and a panel opened after the run began has to see it. A capsule is
/// drawn once per run and brought up to date afterwards, so a bar slides from where it was rather than from
/// nothing on every record.
/// </para>
/// </remarks>
internal sealed class ProgressPanel : StackPanel
{
    /// <summary>How wide the stack is, which is what a name and a bar are read in.</summary>
    private const double WidthOfStack = 300;

    /// <summary>How long a capsule takes to leave the stack after its command ends.</summary>
    private static readonly TimeSpan Leaving = TimeSpan.FromMilliseconds(230);

    /// <summary>The pictures the capsule heads are drawn with.</summary>
    private readonly IIconLibrary _icons;

    /// <summary>What the stack is drawn from.</summary>
    private readonly ProgressHub _hub;

    /// <summary>The capsule drawn for each run being shown, and the runs that are leaving.</summary>
    private readonly Dictionary<ProgressRun, Capsule> _drawn = new(ReferenceEqualityComparer.Instance);

    /// <summary>The row the capsules stand in, under the small heading.</summary>
    private readonly StackPanel _capsules = new() { Spacing = 7 };

    /// <summary>The small heading that says how many commands are running.</summary>
    private readonly Border _heading;

    /// <summary>How many commands the heading says are running.</summary>
    private readonly TextBlock _count = new() { Classes = { "muted" } };

    /// <summary>Makes the stack.</summary>
    /// <param name="hub">What the stack is drawn from.</param>
    /// <param name="icons">The pictures a command's capsule is headed with.</param>
    public ProgressPanel(ProgressHub hub, IIconLibrary icons)
    {
        ArgumentNullException.ThrowIfNull(hub);
        ArgumentNullException.ThrowIfNull(icons);

        _hub = hub;
        _icons = icons;

        Width = WidthOfStack;
        HorizontalAlignment = HorizontalAlignment.Right;
        VerticalAlignment = VerticalAlignment.Bottom;
        Margin = new Thickness(0, 0, 18, 18);
        Spacing = 7;
        IsVisible = false;

        var heading = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 6,
            VerticalAlignment = VerticalAlignment.Center,
        };

        var dot = new Border
        {
            Width = 5,
            Height = 5,
            CornerRadius = new CornerRadius(3),
            VerticalAlignment = VerticalAlignment.Center,
        };
        dot[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Accent);

        var word = new TextBlock
        {
            Text = RolaI18N.Get("rorolala_vcs.progress.header"),
            FontSize = 10,
            VerticalAlignment = VerticalAlignment.Center,
        };
        word[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Muted);

        heading.Children.Add(dot);
        heading.Children.Add(word);
        heading.Children.Add(_count);

        _heading = new Border
        {
            HorizontalAlignment = HorizontalAlignment.Right,
            CornerRadius = new CornerRadius(999),
            BorderThickness = new Thickness(1),
            Padding = new Thickness(8, 3),
            Margin = new Thickness(0, 0, 0, 1),
            Background = new SolidColorBrush(Color.FromArgb(150, 0, 0, 0)),
            Child = heading,
            IsHitTestVisible = false,
        };
        _heading[!Border.BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

        Children.Add(_heading);
        Children.Add(_capsules);

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

    /// <summary>Draws a capsule for every run the hub holds, and takes away the ones it no longer does.</summary>
    internal void Draw()
    {
        var runs = _hub.Runs;
        var live = new HashSet<ProgressRun>(runs, ReferenceEqualityComparer.Instance);

        foreach (var gone in _drawn.Keys.Where(run => !live.Contains(run)).ToArray())
        {
            var capsule = _drawn[gone];

            if (capsule.LeavingStarted)
            {
                continue;
            }

            capsule.Leave(() =>
            {
                _capsules.Children.Remove(capsule.Root);
                _drawn.Remove(gone);
                Settle();
            });
        }

        foreach (var run in runs)
        {
            if (!_drawn.TryGetValue(run, out var capsule))
            {
                capsule = new Capsule(_icons, run.TitleKey);
                _drawn[run] = capsule;
                _capsules.Children.Add(capsule.Root);
            }

            capsule.Draw(run);
        }

        Settle();
    }

    /// <summary>Says whether there is anything to look at, and how many commands are still running.</summary>
    private void Settle()
    {
        IsVisible = _drawn.Count > 0;
        _heading.IsVisible = _hub.Any;
        _count.Text = _hub.Any
            ? _hub.Runs.Count.ToString(CultureInfo.InvariantCulture)
            : string.Empty;
    }

    /// <summary>One command's capsule: its head, its own bar, and the phases under it while the pointer is on it.</summary>
    private sealed class Capsule
    {
        /// <summary>The head: what the command is called and how far it has got.</summary>
        private readonly Grid _head = new()
        {
            ColumnDefinitions = new ColumnDefinitions("Auto,*,Auto,Auto"),
            ColumnSpacing = 8,
        };

        /// <summary>The command's name.</summary>
        private readonly TextBlock _title = new()
        {
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };

        /// <summary>Which phase of the command is being drawn, when it reports more than one.</summary>
        private readonly TextBlock _phase = new()
        {
            Classes = { "muted" },
            FontSize = 10,
            VerticalAlignment = VerticalAlignment.Center,
        };

        /// <summary>How far the command has got, as the look's muted ink writes it.</summary>
        private readonly TextBlock _share = new()
        {
            Classes = { "muted" },
            FontSize = 11,
            VerticalAlignment = VerticalAlignment.Center,
        };

        /// <summary>The bar for the whole of the command.</summary>
        private readonly ProgressBar _whole = new()
        {
            Height = 4,
            MinHeight = 0,
            VerticalAlignment = VerticalAlignment.Center,
        };

        /// <summary>The phases, shown only while the pointer is on the capsule.</summary>
        private readonly StackPanel _details = new() { Spacing = 5 };

        /// <summary>The frame the phases stand in, which is what the pointer shows and hides.</summary>
        private readonly Border _detailFrame;

        /// <summary>The phase rows, by the phase they stand for.</summary>
        private readonly Dictionary<ProgressTask, Phase> _phases = new(ReferenceEqualityComparer.Instance);

        /// <summary>The capsule itself.</summary>
        private readonly Border _root;

        /// <summary>The command now drawn, which decides whether there are phases to show.</summary>
        private ProgressRun? _run;

        /// <summary>Whether the fade out has begun, so it is not begun twice.</summary>
        public bool LeavingStarted { get; private set; }

        /// <summary>Makes a capsule headed with the picture of its command.</summary>
        /// <param name="icons">The pictures the commands are drawn with.</param>
        /// <param name="titleKey">What the command is called, which is also what its picture is chosen by.</param>
        public Capsule(IIconLibrary icons, string titleKey)
        {
            var glyph = new Border
            {
                Width = 18,
                Height = 18,
                CornerRadius = new CornerRadius(5),
                VerticalAlignment = VerticalAlignment.Center,
            };
            glyph[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Dark);

            if (icons.Find(IconOf(titleKey)) is { } picture)
            {
                var mark = new Border
                {
                    Width = 12,
                    Height = 12,
                    HorizontalAlignment = HorizontalAlignment.Center,
                    VerticalAlignment = VerticalAlignment.Center,
                    OpacityMask = new ImageBrush(picture) { Stretch = Stretch.Uniform },
                };
                mark[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Front);
                RenderOptions.SetBitmapInterpolationMode(mark, BitmapInterpolationMode.HighQuality);
                glyph.Child = mark;
            }

            Grid.SetColumn(glyph, 0);
            Grid.SetColumn(_title, 1);
            Grid.SetColumn(_phase, 2);
            Grid.SetColumn(_share, 3);
            _head.Children.Add(glyph);
            _head.Children.Add(_title);
            _head.Children.Add(_phase);
            _head.Children.Add(_share);

            _whole[!TemplatedControl.ForegroundProperty] =
                new DynamicResourceExtension(PaletteKeys.Primary);

            _detailFrame = new Border
            {
                IsVisible = false,
                BorderThickness = new Thickness(0, 1, 0, 0),
                Padding = new Thickness(0, 7, 0, 0),
                Child = _details,
            };
            _detailFrame[!Border.BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

            var content = new StackPanel { Spacing = 8 };
            content.Children.Add(_head);
            content.Children.Add(_whole);
            content.Children.Add(_detailFrame);

            _root = new Border
            {
                CornerRadius = new CornerRadius(9),
                BorderThickness = new Thickness(1),
                Padding = new Thickness(10, 9),
                Child = content,
                Opacity = 0,
                Transitions = new Transitions
                {
                    new DoubleTransition
                    {
                        Property = OpacityProperty,
                        Duration = TimeSpan.FromMilliseconds(180),
                    },
                },
            };
            _root[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Bright);
            _root[!Border.BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

            _root.AttachedToVisualTree += (_, _) =>
                Dispatcher.UIThread.Post(() => _root.Opacity = 1);

            _root.PointerEntered += (_, _) =>
                _detailFrame.IsVisible = _run is { Records.Tasks.Count: > 0 };
            _root.PointerExited += (_, _) => _detailFrame.IsVisible = false;
        }

        /// <summary>The capsule, for the stack to hold.</summary>
        public Control Root => _root;

        /// <summary>Draws the capsule from its run, keeping the phase rows it already has.</summary>
        /// <param name="run">The run.</param>
        public void Draw(ProgressRun run)
        {
            _run = run;

            var tasks = run.Records.Tasks;
            var tops = tasks.Where(task => task.Parent is null).ToArray();

            // A run's top-level tasks are its phases, and they are counted in what each phase is:
            // a sync counts the plan's steps, the index's objects, the blocks it moves and the
            // entries it writes, and adding those up would be adding steps to bytes to entries.
            // The bar therefore follows the phase being worked on, and the counters beside it say
            // which phase that is out of how many the run has reported so far.
            var at = Array.FindLastIndex(tops, task => task.Total is not { } total || task.Done < total);
            if (at < 0)
            {
                at = tops.Length - 1;
            }

            var focus = at >= 0 ? tops[at] : null;

            _title.Text = run.Title;
            _phase.IsVisible = focus is not null && tops.Length > 1;
            _phase.Text = focus is null || tops.Length < 2
                ? string.Empty
                : $"{at + 1}/{tops.Length}";
            _share.Text = tasks.Count == 0
                ? RolaI18N.Get("rorolala_vcs.progress.waiting")
                : focus?.Total is { } length
                    ? Share(focus.Done, length)
                    : "…";

            _whole.IsIndeterminate = focus?.Total is null;
            _whole.Maximum = Math.Max(focus?.Total ?? 1, 1);
            _whole.Value = focus?.Total is { } whole
                ? Math.Clamp(focus.Done, 0, Math.Max(whole, 1))
                : 0;

            DrawPhases(tasks);
        }

        /// <summary>Starts the fade that takes a capsule of a command that has ended out of the stack.</summary>
        /// <param name="removed">What to do once the fade is over.</param>
        public void Leave(Action removed)
        {
            LeavingStarted = true;
            _root.IsHitTestVisible = false;
            _root.Opacity = 0;

            var timer = new DispatcherTimer { Interval = Leaving };
            timer.Tick += (_, _) =>
            {
                timer.Stop();
                removed();
            };
            timer.Start();
        }

        /// <summary>Draws a row for every phase, keeping the rows it drew before.</summary>
        /// <param name="tasks">The phases.</param>
        private void DrawPhases(IReadOnlyList<ProgressTask> tasks)
        {
            foreach (var task in tasks)
            {
                if (!_phases.TryGetValue(task, out var row))
                {
                    row = new Phase();
                    _phases[task] = row;
                    _details.Children.Add(row.Root);
                }

                row.Draw(Head(task), task.Done, task.Total);
            }
        }

        /// <summary>What a phase is called: its own name, or the direction it moves in.</summary>
        /// <param name="task">The phase.</param>
        /// <returns>What to write at the start of its row.</returns>
        private static string Head(ProgressTask task) =>
            task.What is { Length: > 0 } named
                ? named
                : task.Aim switch
                {
                    MoveAim.Up => "↑ " + RolaI18N.Get("rorolala_vcs.progress.row_up"),
                    MoveAim.Down => "↓ " + RolaI18N.Get("rorolala_vcs.progress.row_down"),
                    _ => RolaI18N.Get("rorolala_vcs.progress.working"),
                };

        /// <summary>How far something has got, as a percentage.</summary>
        /// <param name="done">How much is behind it.</param>
        /// <param name="total">How long it is in all.</param>
        /// <returns>The percentage.</returns>
        private static string Share(long done, long total) =>
            total == 0
                ? "100%"
                : ((done * 100) / total).ToString(CultureInfo.InvariantCulture) + "%";

        /// <summary>The picture a command is headed with.</summary>
        /// <param name="titleKey">What the command is called.</param>
        /// <returns>The key of its picture.</returns>
        private static string IconOf(string titleKey) => titleKey switch
        {
            "rorolala_vcs.progress.sync" => Icons.Sync,
            "rorolala_vcs.progress.fetch" => Icons.CloudDownload,
            "rorolala_vcs.progress.checkin" => Icons.CloudUpload,
            "rorolala_vcs.progress.hold" => Icons.Lock,
            "rorolala_vcs.progress.giveup" => Icons.LockOpen,
            "rorolala_vcs.progress.track" => Icons.Add,
            "rorolala_vcs.progress.retrack" => Icons.AltRoute,
            "rorolala_vcs.progress.join" => Icons.Merge,
            "rorolala_vcs.progress.restore" => Icons.Download,
            "rorolala_vcs.progress.unmerge" => Icons.CallSplit,
            "rorolala_vcs.progress.path_move" => Icons.ArrowRightAlt,
            "rorolala_vcs.progress.path_remove" => Icons.Error,
            _ => Icons.Work,
        };

        /// <summary>One phase's row: its name, a thin bar and its share.</summary>
        private sealed class Phase
        {
            /// <summary>The name, in the left column.</summary>
            private readonly TextBlock _name = new()
            {
                FontSize = 10.5,
                VerticalAlignment = VerticalAlignment.Center,
                TextTrimming = TextTrimming.CharacterEllipsis,
            };

            /// <summary>The thin bar.</summary>
            private readonly ProgressBar _bar = new()
            {
                Height = 3,
                MinHeight = 0,
                VerticalAlignment = VerticalAlignment.Center,
            };

            /// <summary>The share, in the right column.</summary>
            private readonly TextBlock _share = new()
            {
                Classes = { "muted" },
                FontSize = 10,
                Width = 34,
                TextAlignment = TextAlignment.Right,
                VerticalAlignment = VerticalAlignment.Center,
            };

            /// <summary>The row.</summary>
            private readonly Grid _root;

            /// <summary>Makes a row.</summary>
            public Phase()
            {
                _root = new Grid
                {
                    ColumnDefinitions = new ColumnDefinitions("84,*,34"),
                    ColumnSpacing = 7,
                };
                _bar[!TemplatedControl.ForegroundProperty] =
                    new DynamicResourceExtension(PaletteKeys.Primary);
                Grid.SetColumn(_name, 0);
                Grid.SetColumn(_bar, 1);
                Grid.SetColumn(_share, 2);
                _root.Children.Add(_name);
                _root.Children.Add(_bar);
                _root.Children.Add(_share);
            }

            /// <summary>The row, for the capsule to hold.</summary>
            public Control Root => _root;

            /// <summary>Draws the row from its phase.</summary>
            /// <param name="name">What it is called.</param>
            /// <param name="done">How much is behind it.</param>
            /// <param name="total">How long it is in all, or nothing when that was never said.</param>
            public void Draw(string name, long done, long? total)
            {
                _name.Text = name;

                if (total is not { } length)
                {
                    _bar.IsIndeterminate = true;
                    _share.Text = "…";
                }
                else
                {
                    _bar.IsIndeterminate = false;
                    _bar.Maximum = Math.Max(length, 1);
                    _bar.Value = length > 0 ? Math.Clamp(done, 0, length) : 1;
                    _share.Text = length == 0 ? "100%" : ((done * 100) / length) + "%";
                }
            }
        }
    }
}
