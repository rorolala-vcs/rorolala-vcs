using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Plugins;

namespace RorolalaDesktop.CoreDocks;

/// <summary>
/// The Preference dock: every setting an owner declared, grouped by owner and by group.
/// </summary>
/// <remarks>
/// A kernel dock, always available from <c>Window</c>. It shows what plugins declared rather than what the
/// host knows about them — a plugin says what it can be told to do, and the host renders it — so a plugin
/// needs no UI of its own to be settable.
/// <para>
/// Editing here writes <c>preference.json</c> at once. A setting that changes what the program does without
/// a restart takes effect there and then; one that does not says so where it is shown.
/// </para>
/// </remarks>
internal sealed class PreferenceDock : IDockView
{
    /// <summary>The view this dock shows.</summary>
    private readonly Control _view;

    /// <summary>Makes the dock over the settings it shows.</summary>
    /// <param name="settings">What every owner declared, and what each is worth.</param>
    /// <param name="plugins">The plugins that were discovered and ordered.</param>
    /// <param name="i18n">The host's translations.</param>
    public PreferenceDock(SettingRegistry settings, PluginManager plugins, I18nService i18n) =>
        _view = new PreferenceView(settings, plugins, i18n);

    /// <inheritdoc />
    public Control View => _view;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderCommand> HeaderCommands => [];
}

/// <summary>The Preference dock's control: owners on the left, what the chosen one declared on the right.</summary>
internal sealed class PreferenceView : UserControl
{
    /// <summary>What every owner declared, and what each is worth.</summary>
    private readonly SettingRegistry _settings;

    /// <summary>The host's translations.</summary>
    private readonly I18nService _i18n;

    /// <summary>The owners to choose between, and which of them is being shown.</summary>
    private readonly ListBox _owners = new();

    /// <summary>What the chosen owner declared.</summary>
    private readonly StackPanel _shown = new();

    /// <summary>Makes the control over the settings it shows.</summary>
    /// <param name="settings">What every owner declared, and what each is worth.</param>
    /// <param name="plugins">The plugins that were discovered and ordered.</param>
    /// <param name="i18n">The host's translations.</param>
    public PreferenceView(SettingRegistry settings, PluginManager plugins, I18nService i18n)
    {
        _settings = settings;
        _i18n = i18n;

        var owners = new List<Chooseable> { new(Core.Id, i18n.Get("core.name")) };

        foreach (var plugin in plugins.LoadOrder)
        {
            owners.Add(new Chooseable(plugin.Manifest.Id, i18n.Get(plugin.Manifest.DisplayNameKey)));
        }

        _owners.ItemsSource = owners;
        _owners.Width = 180;
        _owners.SelectedIndex = 0;
        _owners.SelectionChanged += (_, _) => Show();

        var left = new Border
        {
            BorderThickness = new Thickness(0, 0, 1, 0),
            Child = _owners,
        };

        var right = new ScrollViewer
        {
            Content = new StackPanel { Margin = new Thickness(12), Spacing = 12, Children = { _shown } },
        };

        var panel = new DockPanel { LastChildFill = true };
        DockPanel.SetDock(left, Dock.Left);
        panel.Children.Add(left);
        panel.Children.Add(right);

        Content = panel;

        Show();
    }

    /// <summary>One owner the left list offers.</summary>
    /// <param name="Id">The owner's identity.</param>
    /// <param name="Name">What it is called.</param>
    private sealed record Chooseable(PluginId Id, string Name)
    {
        /// <inheritdoc />
        public override string ToString() => Name;
    }

    /// <summary>Draws what the chosen owner declared, grouped as it declared it.</summary>
    private void Show()
    {
        _shown.Children.Clear();

        if (_owners.SelectedItem is not Chooseable chosen)
        {
            return;
        }

        var settings = _settings.Of(chosen.Id);

        if (settings.Count == 0)
        {
            _shown.Children.Add(new TextBlock { Text = _i18n.Get("setting.none") });

            return;
        }

        // Groups appear in the order they were first declared in, and settings within one by their order,
        // so that what a reader sees follows what the owner wrote rather than a sort of the host's own.
        foreach (var group in settings.Select(Group).Distinct(StringComparer.Ordinal))
        {
            _shown.Children.Add(GroupBox(chosen.Id, group, settings.Where(setting => Group(setting) == group)));
        }
    }

    /// <summary>One group of settings: its name and the settings it holds.</summary>
    /// <param name="owner">The owner that declared them.</param>
    /// <param name="group">The group's name.</param>
    /// <param name="settings">The settings in it.</param>
    private Control GroupBox(PluginId owner, string group, IEnumerable<PluginSetting> settings)
    {
        var rows = new StackPanel { Spacing = 10 };

        foreach (var setting in settings.OrderBy(setting => setting.Order))
        {
            rows.Children.Add(Row(owner, setting));
        }

        return new StackPanel
        {
            Spacing = 10,
            Children =
            {
                new TextBlock { Text = group, FontWeight = FontWeight.SemiBold },
                rows,
            },
        };
    }

    /// <summary>One setting: what it is called, its editor, and what it is until it is changed.</summary>
    /// <param name="owner">The owner that declared it.</param>
    /// <param name="setting">The setting.</param>
    private Control Row(PluginId owner, PluginSetting setting)
    {
        var value = _settings.Value(owner, setting) ?? string.Empty;
        var rows = new StackPanel { Spacing = 4 };

        rows.Children.Add(new TextBlock { Text = _i18n.Get(setting.LabelKey) });

        switch (setting.Kind)
        {
            case SettingKind.Bool:
                var check = new CheckBox { IsChecked = string.Equals(value, "true", StringComparison.Ordinal) };
                check.IsCheckedChanged += (_, _) =>
                    _settings.Keep(owner, setting, check.IsChecked == true ? "true" : "false");
                rows.Children.Add(check);
                break;

            case SettingKind.Choice:
                var options = (setting.Options ?? []).ToArray();
                var choice = new ComboBox
                {
                    ItemsSource = options.Select(option => _i18n.Get(option.LabelKey)).ToArray(),
                    SelectedIndex = Array.FindIndex(options, option => string.Equals(option.Value, value, StringComparison.Ordinal)),
                    MinWidth = 240,
                    HorizontalAlignment = HorizontalAlignment.Left,
                };
                choice.SelectionChanged += (_, _) =>
                {
                    if (choice.SelectedIndex >= 0)
                    {
                        _settings.Keep(owner, setting, options[choice.SelectedIndex].Value);
                    }
                };
                rows.Children.Add(choice);
                break;

            default:
                var field = new TextBox { Text = value, MinWidth = 240, HorizontalAlignment = HorizontalAlignment.Left };

                // Written when the field is left or entered rather than on every letter, because the file is
                // written on every change and a setting is a word rather than a keystroke.
                field.LostFocus += (_, _) => _settings.Keep(owner, setting, field.Text);
                field.KeyDown += (_, key) =>
                {
                    if (key.Key == Avalonia.Input.Key.Enter)
                    {
                        _settings.Keep(owner, setting, field.Text);
                    }
                };
                rows.Children.Add(field);
                break;
        }

        var notes = new List<string>();

        if (setting.Default is { Length: > 0 } declared)
        {
            notes.Add(_i18n.Get("setting.default", declared));
        }

        if (setting.RestartRequired)
        {
            notes.Add(_i18n.Get("setting.restart"));
        }

        if (notes.Count > 0)
        {
            rows.Children.Add(
                new TextBlock
                {
                    Text = string.Join(" · ", notes),
                    FontSize = 11,
                    Opacity = 0.7,
                }
            );
        }

        return rows;
    }

    /// <summary>The group a setting is shown under, which is the part of its identity before the slash.</summary>
    /// <param name="setting">The setting.</param>
    private static string Group(PluginSetting setting)
    {
        var at = setting.Id.IndexOf('/', StringComparison.Ordinal);

        return at <= 0 ? setting.Id : setting.Id[..at];
    }
}
