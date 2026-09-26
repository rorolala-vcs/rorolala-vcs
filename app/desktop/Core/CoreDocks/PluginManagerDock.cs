using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Runtime.CompilerServices;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Templates;
using Avalonia.Data;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Plugins;

namespace RorolalaDesktop.CoreDocks;

/// <summary>
/// The Plugin Manager: the plugins that were found, their state, and what went wrong.
/// </summary>
/// <remarks>
/// A kernel dock, always available from <c>Window</c> and not disableable. Editing here writes
/// <c>plugins.json</c>; the change takes effect on the next start, since a plugin assembly is never
/// unloaded in-process. That is also why the dock is kernel: the natural place to repair plugin
/// configuration must not itself be a plugin that could fail to load.
/// </remarks>
internal sealed class PluginManagerDock : IDockView
{
    /// <summary>The view this dock shows.</summary>
    private readonly Control _view;

    /// <summary>Makes the dock over the plugins it shows.</summary>
    /// <param name="manager">What was discovered and what went wrong.</param>
    /// <param name="i18n">The host's translations.</param>
    /// <param name="configuration">The user state that edits are written back to.</param>
    public PluginManagerDock(
        PluginManager manager,
        I18nService i18n,
        PluginsConfiguration configuration
    ) => _view = new PluginManagerView(manager, i18n, configuration);

    /// <inheritdoc />
    public Control View => _view;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderCommand> HeaderCommands => [];
}

/// <summary>The Plugin Manager's control.</summary>
internal sealed class PluginManagerView : UserControl
{
    /// <summary>Makes the control over the plugins it shows.</summary>
    /// <param name="manager">What was discovered and what went wrong.</param>
    /// <param name="i18n">The host's translations.</param>
    /// <param name="configuration">The user state that edits are written back to.</param>
    public PluginManagerView(
        PluginManager manager,
        I18nService i18n,
        PluginsConfiguration configuration
    )
    {
        var rows = new ObservableCollection<PluginRow>(
            manager.Discovered.Select(plugin =>
                new PluginRow(
                    configuration,
                    plugin.Id,
                    i18n.Get(plugin.Manifest.DisplayNameKey)
                )
            )
        );

        var panel = new StackPanel { Spacing = 12, Margin = new Thickness(12) };

        panel.Children.Add(
            Section(
                i18n.Get("plugin_manager.plugins"),
                new ItemsControl
                {
                    ItemsSource = rows,
                    ItemTemplate = new FuncDataTemplate<PluginRow>(
                        (row, _) => RowView(row, i18n),
                        true
                    ),
                }
            )
        );

        var problems = manager.Problems.Concat(manager.OrderingNotes).ToArray();

        if (problems.Length > 0)
        {
            var notes = new StackPanel { Spacing = 8 };

            for (var i = 0; i < problems.Length; i++)
            {
                // The rule sits above every note but the first, which is already under the heading's own.
                if (i > 0)
                {
                    notes.Children.Add(Divider());
                }

                notes.Children.Add(Note(problems[i].Subject, problems[i].Description));
            }

            panel.Children.Add(Section(i18n.Get("plugin_manager.problems"), notes));
        }

        panel.Children.Add(
            new TextBlock
            {
                Text = i18n.Get("plugin_manager.next_start"),
                TextWrapping = TextWrapping.Wrap,
                Classes = { "caption", "muted" },
            }
        );

        Content = new ScrollViewer { Content = panel };
    }

    /// <summary>A section: its heading, the rule under it, and what it holds.</summary>
    private static Control Section(string heading, Control content) =>
        new StackPanel
        {
            Spacing = 8,
            Children =
            {
                new TextBlock { Text = heading, Classes = { "section" } },
                Divider(),
                content,
            },
        };

    /// <summary>The hairline separating a heading from what follows it, in the theme's line colour.</summary>
    private static Border Divider() =>
        new()
        {
            BorderThickness = new Thickness(0, 0, 0, 1),
            [!Border.BorderBrushProperty] = new DynamicResourceExtension("rorolala.theme.line"),
        };

    /// <summary>One problem as two lines: what it is about, then what is wrong with it.</summary>
    private static Control Note(string subject, string description) =>
        new StackPanel
        {
            Spacing = 4,
            Children =
            {
                new TextBlock { Text = subject, TextWrapping = TextWrapping.Wrap },
                new TextBlock
                {
                    Text = description,
                    TextWrapping = TextWrapping.Wrap,
                    Classes = { "caption", "muted" },
                },
            },
        };

    /// <summary>One plugin's row: its name and identity, its switch, and its order.</summary>
    private static Control RowView(PluginRow row, I18nService i18n)
    {
        var identity = new StackPanel { Spacing = 4, VerticalAlignment = VerticalAlignment.Center };
        identity.Children.Add(new TextBlock { Text = row.Name });
        identity.Children.Add(
            new TextBlock { Text = row.Id, Classes = { "caption", "muted" } }
        );

        var enabled = new CheckBox { IsChecked = row.Enabled };
        enabled.Bind(
            CheckBox.IsCheckedProperty,
            new Binding(nameof(PluginRow.Enabled)) { Mode = BindingMode.TwoWay }
        );

        var order = new NumericUpDown
        {
            Value = row.Order,
            Minimum = -9999,
            Maximum = 9999,
            Increment = 1,
            Width = 88,
        };
        order.Bind(
            NumericUpDown.ValueProperty,
            new Binding(nameof(PluginRow.Order)) { Mode = BindingMode.TwoWay }
        );

        var orderLabel = new TextBlock
        {
            Text = i18n.Get("plugin_manager.order"),
            VerticalAlignment = VerticalAlignment.Center,
            Classes = { "muted" },
        };

        var rowView = new Grid { ColumnDefinitions = new ColumnDefinitions("*,Auto,Auto,Auto") };
        rowView.Children.Add(identity);
        rowView.Children.Add(Place(enabled, 1));
        rowView.Children.Add(Place(orderLabel, 2));
        rowView.Children.Add(Place(order, 3));

        return rowView;
    }

    /// <summary>Puts a control in a grid column, centred vertically.</summary>
    private static Control Place(Control control, int column)
    {
        control.VerticalAlignment = VerticalAlignment.Center;
        control.Margin = new Thickness(8, 0, 0, 0);

        Grid.SetColumn(control, column);

        return control;
    }
}

/// <summary>
/// One plugin as the manager shows it: its name and identity, whether it loads, and its order.
/// </summary>
/// <remarks>
/// Editing writes the state straight back to <c>plugins.json</c>, so the file is what the user sees
/// and what the next start reads, rather than a copy that is written on exit and lost on a crash.
/// </remarks>
internal sealed class PluginRow : INotifyPropertyChanged
{
    /// <summary>The user state edits are written back to.</summary>
    private readonly PluginsConfiguration _configuration;

    /// <summary>The plugin this row is for.</summary>
    private readonly PluginId _id;

    /// <summary>Whether the plugin loads.</summary>
    private bool _enabled;

    /// <summary>The user's ordering, as the numeric control holds it.</summary>
    private decimal? _order;

    /// <summary>Makes a row for one plugin.</summary>
    /// <param name="configuration">The user state edits are written back to.</param>
    /// <param name="id">The plugin's identity.</param>
    /// <param name="name">The plugin's display name, already translated.</param>
    public PluginRow(PluginsConfiguration configuration, PluginId id, string name)
    {
        _configuration = configuration;
        _id = id;
        Name = name;
        Id = id.Value;

        var state = configuration.Plugins.TryGetValue(id, out var found)
            ? found
            : new PluginState(Enabled: true, Order: 0);

        _enabled = state.Enabled;
        _order = state.Order;
    }

    /// <summary>The plugin's display name.</summary>
    public string Name { get; }

    /// <summary>The plugin's identity.</summary>
    public string Id { get; }

    /// <summary>Whether the plugin loads.</summary>
    public bool Enabled
    {
        get => _enabled;
        set
        {
            if (_enabled == value)
            {
                return;
            }

            _enabled = value;
            Store();
            Raise();
        }
    }

    /// <summary>The user's ordering within one dependency tier.</summary>
    public decimal? Order
    {
        get => _order;
        set
        {
            if (_order == value)
            {
                return;
            }

            _order = value;
            Store();
            Raise();
        }
    }

    /// <inheritdoc />
    public event PropertyChangedEventHandler? PropertyChanged;

    /// <summary>Writes this row's state back to the file.</summary>
    private void Store()
    {
        _configuration.Plugins[_id] = new PluginState(_enabled, (int)(_order ?? 0));
        ConfigurationLoader.WritePlugins(_configuration);
    }

    /// <summary>Tells the view a property changed.</summary>
    private void Raise([CallerMemberName] string? name = null) =>
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}
