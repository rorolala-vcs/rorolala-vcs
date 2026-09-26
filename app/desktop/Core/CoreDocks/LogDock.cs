using Avalonia.Controls;
using Avalonia.Controls.Templates;
using Avalonia.Data;
using Avalonia.Layout;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Logging;

namespace RorolalaDesktop.CoreDocks;

/// <summary>
/// The Log dock: the host's one log, shown.
/// </summary>
/// <remarks>
/// A kernel dock, always available from <c>Window</c> and not disableable. It is a view over the
/// host's log rather than a second logging system, so a plugin does not choose where its words go —
/// they go where everything goes.
/// </remarks>
internal sealed class LogDock : IDockView
{
    /// <summary>The view this dock shows.</summary>
    private readonly LogView _view;

    /// <summary>Makes the dock over the log it shows.</summary>
    /// <param name="log">The host's log.</param>
    public LogDock(LogService log) => _view = new LogView(log);

    /// <inheritdoc />
    public Control View => _view;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderCommand> HeaderCommands => [];
}

/// <summary>The Log dock's control: each distinct line once, with its repeat count.</summary>
internal sealed class LogView : UserControl
{
    /// <summary>Makes the control over the log it shows.</summary>
    /// <param name="log">The host's log.</param>
    public LogView(LogService log)
    {
        var list = new ListBox
        {
            ItemsSource = log.Entries,
            ItemTemplate = new FuncDataTemplate<LogEntry>((entry, _) => Row(entry), true),
        };

        // Laid over the list rather than beside it, so the empty line sits in the list's own room and
        // the two are not measured against each other.
        var empty = new TextBlock
        {
            Text = RolaI18N.Get("core.empty_log"),
            Classes = { "muted" },
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        // The log only ever gains lines, so this state changes once; the subscription has nothing to
        // release because a kernel dock lives as long as the window does.
        void Show()
        {
            var isEmpty = log.Entries.Count == 0;

            list.IsVisible = !isEmpty;
            empty.IsVisible = isEmpty;
        }

        log.Entries.CollectionChanged += (_, _) => Show();
        Show();

        Content = new Grid { Children = { list, empty } };
    }

    /// <summary>One line: level, source, message, and how often it was said.</summary>
    private static Control Row(LogEntry entry)
    {
        var row = new Grid { ColumnDefinitions = new ColumnDefinitions("56,160,*,48") };

        // Every cell is monospaced, which is what keeps the four columns aligned down the list; the
        // level and source are metadata and so read smaller, and the level is dimmed further.
        row.Children.Add(Cell(entry, nameof(LogEntry.Level), "{0}", 0, "mono", "caption", "muted"));
        row.Children.Add(Cell(entry, nameof(LogEntry.Source), "{0}", 1, "mono", "caption"));
        row.Children.Add(Cell(entry, nameof(LogEntry.Message), "{0}", 2, "mono"));
        row.Children.Add(Cell(entry, nameof(LogEntry.Count), "\u00d7{0}", 3, "mono"));

        return row;
    }

    /// <summary>One cell of a line, bound to one property of the entry, wearing the given text roles.</summary>
    private static TextBlock Cell(
        LogEntry entry,
        string property,
        string format,
        int column,
        params string[] roles
    )
    {
        var text = new TextBlock { VerticalAlignment = VerticalAlignment.Center };

        foreach (var role in roles)
        {
            text.Classes.Add(role);
        }

        text.Bind(
            TextBlock.TextProperty,
            new Binding(property) { StringFormat = format, Source = entry }
        );

        Grid.SetColumn(text, column);

        return text;
    }
}
