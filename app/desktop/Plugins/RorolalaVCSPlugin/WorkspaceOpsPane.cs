using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Threading;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using Path = Avalonia.Controls.Shapes.Path;

namespace RorolalaVCSPlugin;

/// <summary>
/// The right pane of the Workspace dock: the operations the Workspace recorded, newest first.
/// </summary>
/// <remarks>
/// The list is the stack <c>rola undo</c> pops, one row per run. Opening a row shows what the run
/// changed, which the command line's own listing does not print: the effects the journal recorded,
/// read through the C ABI so this side knows nothing of the file's format.
/// <para>
/// Undo acts on the newest operation and only on it, which is what a header's one button can mean.
/// The run is made off the window's thread because it puts files back, and every view of the work is
/// told afterwards, since a file put back is a file whose state has changed.
/// </para>
/// </remarks>
internal sealed class WorkspaceOpsPane : UserControl
{
    /// <summary>How tall the pane's heading is.</summary>
    private const double Head = 36;

    /// <summary>The room the pane keeps at its left.</summary>
    private const double Edge = 16;

    /// <summary>The room the pane keeps at its right.</summary>
    private const double Trail = 12;

    /// <summary>The host, whose dialogs show a failure and whose progress the undo reports to.</summary>
    private readonly IPluginHost _host;

    /// <summary>Where a log that could not be read is said.</summary>
    private readonly ILog _log;

    /// <summary>The number of operations beside the heading.</summary>
    private readonly TextBlock _count;

    /// <summary>What undoes the newest operation.</summary>
    private readonly Button _undo;

    /// <summary>Where the rows are drawn.</summary>
    private readonly StackPanel _rows;

    /// <summary>The line shown when there is nothing to list.</summary>
    private readonly TextBlock _empty;

    /// <summary>The whole list, which is hidden while it holds nothing.</summary>
    private readonly ScrollViewer _scroll;

    /// <summary>What the list is drawn from.</summary>
    private IReadOnlyList<RecordedOperation> _operations = [];

    /// <summary>The operations whose effects are shown, by the run that made them.</summary>
    private readonly HashSet<(string Command, string Time)> _open = [];

    /// <summary>Whether the first row has already been opened by itself.</summary>
    private bool _begun;

    /// <summary>Whether a log that could not be read is what there is to say.</summary>
    private bool _unreadable;

    /// <summary>Whether the log has been read at all, which the first paint is not.</summary>
    private bool _read;

    /// <summary>The Workspace the list is read from, or nothing before one is read.</summary>
    private string? _root;

    /// <summary>Whether an undo is going, which is what holds the button.</summary>
    private bool _busy;

    /// <summary>Makes the pane.</summary>
    /// <param name="host">The host, whose dialogs show a failure and whose progress the undo reports to.</param>
    /// <param name="log">Where a log that could not be read is said.</param>
    public WorkspaceOpsPane(IPluginHost host, ILog log)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(log);

        _host = host;
        _log = log;

        _count = WorkspaceFurniture.Count(0);
        _undo = WorkspaceFurniture.Compact("rorolala_vcs.workspace.ops_undo", null, Undo);
        ToolTip.SetTip(_undo, RolaI18N.Get("rorolala_vcs.workspace.ops_undo_tip"));

        _rows = new StackPanel();
        _empty = new TextBlock
        {
            TextAlignment = TextAlignment.Center,
            TextWrapping = TextWrapping.Wrap,
            FontSize = 12,
            Margin = new Thickness(Edge, 32, Edge, 12),
            IsVisible = false,
        };
        _empty[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Muted);

        var body = new StackPanel { Children = { _empty, _rows } };

        _scroll = new ScrollViewer
        {
            Content = body,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        };

        var head = new Border
        {
            Height = Head,
            Padding = new Thickness(Edge, 0, Trail, 0),
            Child = Heading(),
        };

        var pane = new Grid { RowDefinitions = new RowDefinitions("Auto,*") };
        pane.Children.Add(head);
        pane.Children.Add(_scroll);
        Grid.SetRow(_scroll, 1);

        Content = pane;

        // The log grows whenever a run writes to it, so it is read again when the files may have
        // changed rather than waiting for the reader to ask. A closed dock is no view of anything,
        // which is why it listens while it is on screen.
        AttachedToVisualTree += (_, _) => _host.Files.Changed += Reload;
        DetachedFromVisualTree += (_, _) => _host.Files.Changed -= Reload;

        Draw();
    }

    /// <summary>Reads the log of a Workspace and draws it.</summary>
    /// <param name="root">The Workspace's root.</param>
    public void Load(string root)
    {
        ArgumentNullException.ThrowIfNull(root);

        _root = root;
        _read = true;

        try
        {
            _operations = WorkspaceOpsReading.Read(root);
            _unreadable = false;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _operations = [];
            _unreadable = true;
            _log.Warn($"the operation log of `{root}` could not be read: {error.Message}");
        }

        if (!_begun && _operations.Count > 0)
        {
            _begun = true;
            _ = _open.Add((_operations[0].Command, _operations[0].Time));
        }

        var held = _operations.Select(operation => (operation.Command, operation.Time)).ToHashSet();

        _open.IntersectWith(held);

        Draw();
    }

    /// <summary>Reads the log again from where it was last read.</summary>
    private void Reload()
    {
        if (_root is { Length: > 0 } root)
        {
            Load(root);
        }
    }

    /// <summary>The heading: what the pane is, how many operations it holds, and how to undo one.</summary>
    /// <returns>The heading.</returns>
    private Control Heading()
    {
        var label = WorkspaceFurniture.Label("rorolala_vcs.workspace.ops_title");

        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("Auto,Auto,*,Auto"),
            ColumnSpacing = 8,
            VerticalAlignment = VerticalAlignment.Center,
        };

        row.Children.Add(label);
        Grid.SetColumn(_count, 1);
        row.Children.Add(_count);
        Grid.SetColumn(_undo, 3);
        row.Children.Add(_undo);

        return row;
    }

    /// <summary>Draws the rows, the count, and whether there is an operation to undo.</summary>
    private void Draw()
    {
        _rows.Children.Clear();

        foreach (var operation in _operations)
        {
            _rows.Children.Add(Operation(operation));
        }

        _count.Text = _operations.Count.ToString(
            System.Globalization.CultureInfo.InvariantCulture
        );
        _undo.IsEnabled = _read && _operations.Count > 0 && !_busy;

        _empty.IsVisible = _read && _operations.Count == 0;

        if (_unreadable)
        {
            _empty.Text = RolaI18N.Get("rorolala_vcs.workspace.ops_unreadable");
        }
        else
        {
            _empty.Text = RolaI18N.Get("rorolala_vcs.workspace.ops_empty");
        }
    }

    /// <summary>One run: what it did, when, and what it changed once it is opened.</summary>
    /// <param name="operation">The run.</param>
    /// <returns>The row.</returns>
    private Control Operation(RecordedOperation operation)
    {
        var key = (operation.Command, operation.Time);
        var open = _open.Contains(key);
        var caret = WorkspaceFold.Caret(open);

        var label = new TextBlock
        {
            Text = WorkspaceOpsReading.Command(operation.Command),
            FontSize = 12.5,
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
            TextWrapping = TextWrapping.NoWrap,
        };
        label[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Front);
        ToolTip.SetTip(label, operation.Command);

        var count = new TextBlock
        {
            Text = RolaI18N.Get("rorolala_vcs.workspace.ops_changes", operation.Changes.Count),
            FontSize = 11,
            VerticalAlignment = VerticalAlignment.Center,
        };
        count[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Muted);

        var time = new TextBlock
        {
            Text = WorkspaceOpsReading.When(operation.Time, DateTimeOffset.UtcNow),
            FontSize = 11,
            MinWidth = 62,
            TextAlignment = TextAlignment.Right,
            VerticalAlignment = VerticalAlignment.Center,
        };
        time[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Muted);
        ToolTip.SetTip(time, WorkspaceOpsReading.Exact(operation.Time));

        var motion = WorkspaceFold.Motion();
        var detail = new StackPanel
        {
            ClipToBounds = true,
            MaxHeight = open ? double.PositiveInfinity : 0,
            Opacity = open ? 1 : 0,
            Transitions = motion,
            Children = { Changes(operation) },
        };

        var chevron = new Border { Width = 12, Child = caret };

        var head = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("16,*,Auto,Auto,12"),
            ColumnSpacing = 8,
            VerticalAlignment = VerticalAlignment.Center,
        };
        head.Children.Add(
            WorkspaceFurniture.Glyph(
                _host,
                WorkspaceOpsReading.CommandIcon(operation.Command),
                PaletteKeys.Faint,
                16
            )
        );
        Grid.SetColumn(label, 1);
        head.Children.Add(label);
        Grid.SetColumn(count, 2);
        head.Children.Add(count);
        Grid.SetColumn(time, 3);
        head.Children.Add(time);
        Grid.SetColumn(chevron, 4);
        head.Children.Add(chevron);

        var row = new Border
        {
            MinHeight = 34,
            Padding = new Thickness(Edge, 3, Trail, 3),
            Background = Brushes.Transparent,
            Cursor = new Cursor(StandardCursorType.Hand),
            Child = head,
        };
        WorkspaceFurniture.Hover(row);

        row.PointerPressed += (_, pressed) =>
        {
            if (!pressed.GetCurrentPoint(row).Properties.IsLeftButtonPressed)
            {
                return;
            }

            var showing = !_open.Remove(key);

            if (showing)
            {
                _ = _open.Add(key);
            }

            WorkspaceFold.Turn(caret, showing);
            WorkspaceFold.Rows(detail, motion, !showing);
            pressed.Handled = true;
        };

        return new StackPanel { Children = { row, detail } };
    }

    /// <summary>What one run changed, one change per row.</summary>
    /// <param name="operation">The run.</param>
    /// <returns>The rows.</returns>
    private Control Changes(RecordedOperation operation)
    {
        var changes = new StackPanel { Margin = new Thickness(40, 2, Trail, 8) };

        foreach (var change in operation.Changes)
        {
            changes.Children.Add(Change(change));
        }

        return changes;
    }

    /// <summary>One change: what it was about, and what became of it.</summary>
    /// <param name="change">The change.</param>
    /// <returns>The row.</returns>
    private Control Change(RecordedChange change)
    {
        var move = change.To.Length > 0;
        var paths = new StackPanel();

        if (move)
        {
            var from = new TextBlock
            {
                Text = change.From,
                FontFamily = GraphMeasure.Mono,
                FontSize = 11,
                LineHeight = 17,
                TextTrimming = TextTrimming.CharacterEllipsis,
                TextWrapping = TextWrapping.NoWrap,
            };
            from[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Muted);

            var to = new TextBlock
            {
                Text = change.To,
                FontFamily = GraphMeasure.Mono,
                FontSize = 11,
                LineHeight = 17,
                TextTrimming = TextTrimming.CharacterEllipsis,
                TextWrapping = TextWrapping.NoWrap,
            };
            to[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Front);

            paths.Children.Add(from);
            paths.Children.Add(to);
        }
        else
        {
            var path = new TextBlock
            {
                Text = change.From,
                FontFamily = GraphMeasure.Mono,
                FontSize = 11,
                LineHeight = 17,
                TextTrimming = TextTrimming.CharacterEllipsis,
                TextWrapping = TextWrapping.NoWrap,
            };
            path[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Front);
            paths.Children.Add(path);
        }

        var state = move ? null : WorkspaceOpsReading.State(change.State);
        var word = new TextBlock
        {
            Text = state?.ToUpperInvariant() ?? string.Empty,
            FontSize = 10,
            LetterSpacing = 0.5,
            VerticalAlignment = VerticalAlignment.Top,
            Margin = new Thickness(0, 3, 0, 0),
            IsVisible = state is { Length: > 0 },
        };
        word[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Faint);

        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("12,*,Auto"),
            ColumnSpacing = 6,
            Margin = new Thickness(0, 2),
            Children =
            {
                new Border
                {
                    Width = 12,
                    Height = 12,
                    Margin = new Thickness(0, 3, 0, 0),
                    Child = WorkspaceFurniture.Glyph(
                        _host,
                        move ? Icons.ArrowRight : Icons.File,
                        PaletteKeys.Faint,
                        12
                    ),
                },
                paths,
                word,
            },
        };
        Grid.SetColumn(paths, 1);
        Grid.SetColumn(word, 2);

        return row;
    }

    /// <summary>Undoes the newest operation.</summary>
    private void Undo()
    {
        if (_root is not { Length: > 0 } root || _busy || _operations.Count == 0)
        {
            return;
        }

        _busy = true;
        _undo.IsEnabled = false;

        var run = _host.Progress.Begin("rorolala_vcs.progress.undo");

        _ = Task.Run(() =>
        {
            var (done, said) = WorkspaceUndo.Run(root);

            Dispatcher.UIThread.Post(() =>
            {
                _busy = false;
                _host.Progress.End(run);

                if (!done)
                {
                    FailureNotice.Show(
                        _host.Dialogs,
                        _host.Icons,
                        RolaI18N.Get("rorolala_vcs.workspace.ops_undo_failed"),
                        said
                    );

                    Draw();

                    return;
                }

                // The log loses the operation it undid, and what the run put back is drawn again by
                // every view of the work: both are the journal's and the files' answers rather than
                // this pane's.
                _host.Files.Touch();
                Load(root);
            });
        });
    }
}
