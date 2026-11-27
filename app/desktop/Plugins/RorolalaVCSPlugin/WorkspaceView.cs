using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// The Workspace dock: the work as a project, which is its ignore rules and what it last did.
/// </summary>
/// <remarks>
/// Two panes, the rules on the left and the log on the right, side by side while the dock is wide
/// enough for both to be read and one above the other when it is not. The same line the dock has
/// always said is shown while the base is not a Workspace: there are no rules and no log to read
/// there, and saying so is the whole of what the view has to say.
/// <para>
/// What is read is read again when the work's base moves, and when the reader asks from the dock's
/// header. The log reads itself again when the files may have changed, since a run writes to it; the
/// rules do not, because a rule file is not the work and an edit that was not saved is not to be
/// thrown away by something else touching the tree.
/// </para>
/// </remarks>
internal sealed class WorkspaceView : UserControl
{
    /// <summary>How wide the docking has to be for the two panes to stand side by side.</summary>
    /// <remarks>
    /// Narrower than this and a rule's pattern and its two buttons would share too little room to be
    /// read, so the panes stack instead. The width is the prototype's, where the two were drawn.
    /// </remarks>
    private const double Apart = 620;

    /// <summary>How wide the line said in place of the two panes is allowed to read.</summary>
    private const double Widest = 420;

    /// <summary>Where a base that could not be read is said.</summary>
    private readonly ILog _log;

    /// <summary>The rules pane.</summary>
    private readonly WorkspaceRulesPane _rules;

    /// <summary>The log pane.</summary>
    private readonly WorkspaceOpsPane _ops;

    /// <summary>The two panes, laid out by the width the dock has.</summary>
    private readonly Grid _split;

    /// <summary>The line over the rules pane, which is the divider while they stand side by side.</summary>
    private readonly Border _rulesFrame;

    /// <summary>The line said while the base is not a Workspace.</summary>
    private readonly TextBlock _notice;

    /// <summary>Whether the panes are stacked rather than side by side.</summary>
    private bool _stacked;

    /// <summary>Makes the dock over the host it reads and reports through.</summary>
    /// <param name="host">The host, which is what the panes reach to run and to report.</param>
    /// <param name="log">Where a base or a file that could not be read is said.</param>
    public WorkspaceView(IPluginHost host, ILog log)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(log);

        _log = log;

        _rules = new WorkspaceRulesPane(host, log);
        _ops = new WorkspaceOpsPane(host, log);

        _rulesFrame = new Border
        {
            Child = _rules,
            BorderThickness = new Thickness(0, 0, 1, 0),
        };
        _rulesFrame[!Border.BorderBrushProperty] = new DynamicResourceExtension(
            PaletteKeys.Splitter
        );

        _split = new Grid
        {
            Children = { _rulesFrame, _ops },
            IsVisible = false,
        };
        Grid.SetColumn(_ops, 1);

        _notice = new TextBlock
        {
            TextWrapping = TextWrapping.Wrap,
            TextAlignment = TextAlignment.Center,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            MaxWidth = Widest,
            Classes = { "muted" },
            IsVisible = false,
        };

        var ground = new Grid { Children = { _split, _notice } };
        Content = ground;

        _split.SizeChanged += (_, sized) => Arrange(sized.NewSize.Width);

        // The catalogue outlives every dock, so the base is followed only while this one is on
        // screen: a dock that was closed has no reason to be told where the work moved to.
        AttachedToVisualTree += (_, _) => { BaseDirectory.Changed += Read; Read(); };
        DetachedFromVisualTree += (_, _) => BaseDirectory.Changed -= Read;

        // The base is read once while the dock is made as well, since a view that was just created
        // for a window shows what the base is at that moment rather than an empty panel until the
        // window puts it on screen.
        Read();
    }

    /// <summary>
    /// Whether the line about a base outside every Workspace is the whole of what is shown.
    /// </summary>
    internal bool ShowsNotice => _notice.IsVisible;

    /// <summary>Reads the rules and the log again, which is what the dock's header offers.</summary>
    public void Refresh() => Read();

    /// <summary>Reads what there is of the base: the two panes, or the line that there is nothing.</summary>
    private void Read()
    {
        var directory = BaseDirectory.Current;

        if (!WorkspaceNotice.Holds(directory, _log))
        {
            _notice.Text = WorkspaceNotice.Notice(directory, _log);
            _notice.IsVisible = true;
            _split.IsVisible = false;

            return;
        }

        string root;

        try
        {
            using var workspace = WorkspaceScope.Open(directory!);
            root = workspace.Root.FullName;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Warn($"where `{directory}` is rooted could not be read: {error.Message}");
            _notice.Text = WorkspaceNotice.Notice(null, _log);
            _notice.IsVisible = true;
            _split.IsVisible = false;

            return;
        }

        _notice.IsVisible = false;
        _split.IsVisible = true;

        _rules.Load(root);
        _ops.Load(root);

        Arrange(Bounds.Width);
    }

    /// <summary>Places the two panes for the width the dock has.</summary>
    /// <remarks>
    /// The divider is the rules pane's own edge, drawn on its right while the two stand side by side
    /// and under it while they stack, so the two never draw a line against each other.
    /// </remarks>
    /// <param name="width">The width the dock has.</param>
    private void Arrange(double width)
    {
        var stacked = width < Apart;

        if (stacked == _stacked && _split.ColumnDefinitions.Count > 0)
        {
            return;
        }

        _stacked = stacked;

        if (stacked)
        {
            _split.ColumnDefinitions = new ColumnDefinitions("*");
            _split.RowDefinitions = new RowDefinitions("*,*");
            Grid.SetColumn(_rulesFrame, 0);
            Grid.SetRow(_rulesFrame, 0);
            Grid.SetColumn(_ops, 0);
            Grid.SetRow(_ops, 1);
            _rulesFrame.BorderThickness = new Thickness(0, 0, 0, 1);
        }
        else
        {
            _split.ColumnDefinitions = new ColumnDefinitions("*,*");
            _split.RowDefinitions = new RowDefinitions("*");
            Grid.SetColumn(_rulesFrame, 0);
            Grid.SetRow(_rulesFrame, 0);
            Grid.SetColumn(_ops, 1);
            Grid.SetRow(_ops, 0);
            _rulesFrame.BorderThickness = new Thickness(0, 0, 1, 0);
        }
    }
}
