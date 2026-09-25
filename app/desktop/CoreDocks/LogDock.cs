using Avalonia.Controls;
using Avalonia.Controls.Templates;
using Avalonia.Data;
using Avalonia.Layout;
using RorolalaDesktop.Contract;
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

        Content = list;
    }

    /// <summary>One line: level, source, message, and how often it was said.</summary>
    private static Control Row(LogEntry entry)
    {
        var row = new Grid { ColumnDefinitions = new ColumnDefinitions("60,150,*,48") };

        row.Children.Add(Cell(entry, nameof(LogEntry.Level), "{0}", 0));
        row.Children.Add(Cell(entry, nameof(LogEntry.Source), "{0}", 1));
        row.Children.Add(Cell(entry, nameof(LogEntry.Message), "{0}", 2));
        row.Children.Add(Cell(entry, nameof(LogEntry.Count), "\u00d7{0}", 3));

        return row;
    }

    /// <summary>One cell of a line, bound to one property of the entry.</summary>
    private static TextBlock Cell(LogEntry entry, string property, string format, int column)
    {
        var text = new TextBlock { VerticalAlignment = VerticalAlignment.Center };

        text.Bind(
            TextBlock.TextProperty,
            new Binding(property) { StringFormat = format, Source = entry }
        );

        Grid.SetColumn(text, column);

        return text;
    }
}
