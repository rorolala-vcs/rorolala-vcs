using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Templates;
using Avalonia.Data;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
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
        var empty = new StackPanel
        {
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            Children =
            {
                new TextBlock
                {
                    Text = "\u2261",
                    FontSize = 34,
                    Classes = { "faint" },
                    HorizontalAlignment = HorizontalAlignment.Center,
                },
                new TextBlock
                {
                    Text = RolaI18N.Get("core.empty_log"),
                    Classes = { "muted" },
                    HorizontalAlignment = HorizontalAlignment.Center,
                },
            },
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

        var body = new Grid { Children = { list, empty } };
        var head = Head();

        DockPanel.SetDock(head, Dock.Top);

        Content = new Border
        {
            Margin = new Thickness(16, 8, 16, 16),
            CornerRadius = new CornerRadius(8),
            ClipToBounds = true,
            BorderThickness = new Thickness(1),
            [!Border.BorderBrushProperty] = new DynamicResourceExtension("rorolala.border"),
            [!Border.BackgroundProperty] = new DynamicResourceExtension("rorolala.bg.elevated"),
            Child = new DockPanel { LastChildFill = true, Children = { head, body } },
        };
    }

    /// <summary>The card's head: the four captions over the columns of the rows below.</summary>
    private static Border Head() =>
        new()
        {
            Height = 30,
            Padding = new Thickness(8, 0),
            BorderThickness = new Thickness(0, 0, 0, 1),
            [!Border.BackgroundProperty] = new DynamicResourceExtension("rorolala.bg.sunken"),
            [!Border.BorderBrushProperty] = new DynamicResourceExtension("rorolala.border"),
            Child = new Grid
            {
                ColumnDefinitions = new ColumnDefinitions("56,160,*,48"),
                Children =
                {
                    Caption("log.level", 0),
                    Caption("log.source", 1),
                    Caption("log.message", 2),
                    Caption("log.count", 3),
                },
            },
        };

    /// <summary>One column's caption, over the column it names.</summary>
    private static TextBlock Caption(string key, int column)
    {
        var text = new TextBlock
        {
            // The design sets these in capitals with `text-transform`, which Avalonia does not have,
            // so the capitals are put on the word here instead.
            Text = RolaI18N.Get(key).ToUpperInvariant(),
            Classes = { "label" },
            VerticalAlignment = VerticalAlignment.Center,
        };

        Grid.SetColumn(text, column);

        return text;
    }

    /// <summary>One line: level, source, message, and how often it was said.</summary>
    private static Control Row(LogEntry entry)
    {
        var row = new Grid { ColumnDefinitions = new ColumnDefinitions("56,160,*,48") };

        // Every cell is monospaced, which is what keeps the four columns aligned down the list; the
        // metadata cells read smaller and fainter than the message they are about.
        row.Children.Add(Cell(entry, nameof(LogEntry.Level), "{0}", 0, "mono", "caption", "faint"));
        row.Children.Add(Cell(entry, nameof(LogEntry.Source), "{0}", 1, "mono", "caption", "faint"));
        row.Children.Add(Cell(entry, nameof(LogEntry.Message), "{0}", 2, "mono"));
        row.Children.Add(Cell(entry, nameof(LogEntry.Count), "\u00d7{0}", 3, "mono", "caption", "faint"));

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
