using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Plugins;
using RorolalaDesktop.Theming;

namespace RorolalaDesktop.CoreDocks;

/// <summary>
/// The Preference dock: every setting an owner declared, grouped by owner and by group.
/// </summary>
/// <remarks>
/// A kernel dock, always available from <c>Window</c>. It shows what plugins declared rather than what the
/// host knows about them — a plugin says what it can be told to do, and the host renders it — so a plugin
/// needs no UI of its own to be settable.
/// <para>
/// Editing here writes <c>preference.json</c> at once, and the three things the look is drawn in, which
/// live in <c>theme.json</c> (Section 5.4), under the kernel's own entry. A colour takes effect there
/// and then; a plugin's setting says where it is shown whether it does (Section 6.1).
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
    /// <param name="theme">How the program looks, and what changing it here does.</param>
    public PreferenceDock(
        SettingRegistry settings,
        PluginManager plugins,
        I18nService i18n,
        ThemeService theme
    ) =>
        _view = new PreferenceView(settings, plugins, i18n, theme);

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

    /// <summary>How the program looks, and what changing it here does.</summary>
    private readonly ThemeService _theme;

    /// <summary>The owners to choose between, and which of them is being shown.</summary>
    private readonly ListBox _owners = new();

    /// <summary>What the chosen owner declared.</summary>
    private readonly StackPanel _shown = new();

    /// <summary>Makes the control over the settings it shows.</summary>
    /// <param name="settings">What every owner declared, and what each is worth.</param>
    /// <param name="plugins">The plugins that were discovered and ordered.</param>
    /// <param name="i18n">The host's translations.</param>
    /// <param name="theme">How the program looks, and what changing it here does.</param>
    public PreferenceView(
        SettingRegistry settings,
        PluginManager plugins,
        I18nService i18n,
        ThemeService theme
    )
    {
        _settings = settings;
        _i18n = i18n;
        _theme = theme;

        var owners = new List<Chooseable> { new(Core.Id, i18n.Get("core.name")) };

        foreach (var plugin in plugins.LoadOrder)
        {
            owners.Add(new Chooseable(plugin.Manifest.Id, i18n.Get(plugin.Manifest.DisplayNameKey)));
        }

        _owners.ItemsSource = owners;
        _owners.Width = 184;
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

        // The look belongs to the kernel, so its three settings sit under the kernel's own entry, above
        // whatever else the kernel later declares.
        if (chosen.Id == Core.Id)
        {
            _shown.Children.Add(ThemeGroup());
        }

        var settings = _settings.Of(chosen.Id);

        if (settings.Count == 0)
        {
            if (chosen.Id != Core.Id)
            {
                _shown.Children.Add(
                    new TextBlock { Text = _i18n.Get("setting.none"), Classes = { "muted" } }
                );
            }

            return;
        }

        // Groups appear in the order they were first declared in, and settings within one by their order,
        // so that what a reader sees follows what the owner wrote rather than a sort of the host's own.
        foreach (var group in settings.Select(Group).Distinct(StringComparer.Ordinal))
        {
            _shown.Children.Add(GroupBox(chosen.Id, group, settings.Where(setting => Group(setting) == group)));
        }
    }

    /// <summary>
    /// The three things the look is drawn in: the variant, the primary, and the accent.
    /// </summary>
    /// <remarks>
    /// They are the kernel's own rather than a plugin's, and they are kept in <c>theme.json</c> rather
    /// than in the panel's own file, so they are drawn here rather than declared like a setting. What a
    /// change does is applied at once: a colour is a resource replaced in place, which reaches every
    /// control without adding a style to a window that already has one (Section 10).
    /// </remarks>
    private Control ThemeGroup()
    {
        var theme = _theme.Theme;
        var rows = new StackPanel { Spacing = 8 };

        rows.Children.Add(ModeRow(theme));
        rows.Children.Add(ColourRow("core.setting.primary", theme.PrimaryOrDefault, chosen => theme.Primary = chosen));
        rows.Children.Add(ColourRow("core.setting.accent", theme.AccentOrDefault, chosen => theme.Accent = chosen));

        rows.Children.Add(
            new TextBlock
            {
                Text = _i18n.Get("core.setting.immediate"),
                Classes = { "caption", "muted" },
            }
        );

        return Section(_i18n.Get("core.setting.theme"), rows);
    }

    /// <summary>The row that chooses the variant, with its way back to the default.</summary>
    /// <param name="theme">What the user chose, edited in place.</param>
    private Control ModeRow(ThemeConfiguration theme)
    {
        var modes = new[] { ColorMode.System, ColorMode.Light, ColorMode.Dark };

        var choice = new ComboBox
        {
            ItemsSource = modes.Select(mode => _i18n.Get($"core.setting.mode.{mode.ToString().ToLowerInvariant()}")).ToArray(),
            SelectedIndex = Array.IndexOf(modes, theme.ModeOrDefault),
            MinWidth = 200,
            VerticalAlignment = VerticalAlignment.Center,
        };

        choice.SelectionChanged += (_, _) =>
        {
            if (choice.SelectedIndex >= 0 && modes[choice.SelectedIndex] != theme.ModeOrDefault)
            {
                theme.Mode = modes[choice.SelectedIndex];
                _theme.Save();

                // Drawn again so that what is on screen follows the default the same way every other
                // row does, and so that Reset appears with something to remove.
                Show();
            }
        };

        return Row("core.setting.mode", choice, () =>
        {
            theme.Mode = null;
            _theme.Save();
            Show();
        });
    }

    /// <summary>One colour as a swatch and its six digits, with the way back to the default.</summary>
    /// <param name="label">The key naming the colour.</param>
    /// <param name="current">What it is now.</param>
    /// <param name="keep">What to set when a valid colour is typed, or nothing to go back to the default.</param>
    private Control ColourRow(string label, Color current, Action<Color?> keep)
    {
        var swatch = new Border
        {
            Width = 20,
            Height = 20,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(0),
            BorderBrush = new SolidColorBrush(Color.FromRgb(0x80, 0x80, 0x80)),
            Background = new SolidColorBrush(current),
            VerticalAlignment = VerticalAlignment.Center,
        };

        var field = new TextBox
        {
            Text = Hex(current),
            MinWidth = 120,
            VerticalAlignment = VerticalAlignment.Center,
        };

        // Written when the field is left or entered rather than on every letter, because the file is
        // written on every change and a colour is a word rather than a keystroke. What will not read as
        // one is put back to what is in force, rather than kept out of the file silently.
        void Commit()
        {
            if (Parse(field.Text) is { } chosen)
            {
                keep(chosen);
                _theme.Save();
                Show();
            }
            else
            {
                field.Text = Hex(current);
            }
        }

        field.LostFocus += (_, _) => Commit();
        field.KeyDown += (_, key) =>
        {
            if (key.Key == Avalonia.Input.Key.Enter)
            {
                Commit();
            }
        };

        var editor = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Children = { swatch, field },
        };

        return Row(label, editor, () =>
        {
            keep(null);
            _theme.Save();
            Show();
        });
    }

    /// <summary>One theme row: what it is called, its editor, and its way back to the default.</summary>
    /// <param name="label">The key naming it.</param>
    /// <param name="editor">What it is chosen with.</param>
    /// <param name="reset">What going back to the default does.</param>
    private Control Row(string label, Control editor, Action reset)
    {
        var button = new Button { Content = _i18n.Get("setting.reset") };
        button.Click += (_, _) => reset();

        return SettingRow(_i18n.Get(label), editor, button);
    }

    /// <summary>One setting as a row: its name on the left, its editor and Reset on the right.</summary>
    private static Grid SettingRow(string label, Control editor, Control reset)
    {
        var row = new Grid { ColumnDefinitions = new ColumnDefinitions("*,Auto") };

        var name = new TextBlock { Text = label, VerticalAlignment = VerticalAlignment.Center };

        var controls = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Right,
            Children = { editor, reset },
        };

        row.Children.Add(name);
        row.Children.Add(controls);
        Grid.SetColumn(controls, 1);

        return row;
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

    /// <summary>The colour six digits and a hash stand for, or nothing when the text is not one.</summary>
    /// <param name="text">What the field holds.</param>
    private static Color? Parse(string? text)
    {
        if (text is null || text.Length != 7 || text[0] != '#')
        {
            return null;
        }

        for (var i = 1; i < text.Length; i++)
        {
            if (!Uri.IsHexDigit(text[i]))
            {
                return null;
            }
        }

        return Color.FromRgb(
            Convert.ToByte(text[1..3], 16),
            Convert.ToByte(text[3..5], 16),
            Convert.ToByte(text[5..7], 16)
        );
    }

    /// <summary>A colour as the file writes it: six digits, whatever its alpha is.</summary>
    /// <param name="colour">The colour.</param>
    private static string Hex(Color colour) => $"#{colour.R:X2}{colour.G:X2}{colour.B:X2}";

    /// <summary>One group of settings: its name and the settings it holds.</summary>
    /// <param name="owner">The owner that declared them.</param>
    /// <param name="group">The group's name.</param>
    /// <param name="settings">The settings in it.</param>
    private Control GroupBox(PluginId owner, string group, IEnumerable<PluginSetting> settings)
    {
        var rows = new StackPanel { Spacing = 8 };

        foreach (var setting in settings.OrderBy(setting => setting.Order))
        {
            rows.Children.Add(Row(owner, setting));
        }

        return Section(group, rows);
    }

    /// <summary>One setting: what it is called, how it is chosen, and the way back to its default.</summary>
    /// <param name="owner">The owner that declared it.</param>
    /// <param name="setting">The setting.</param>
    private Control Row(PluginId owner, PluginSetting setting)
    {
        var value = _settings.Value(owner, setting) ?? string.Empty;

        var reset = new Button { Content = _i18n.Get("setting.reset") };

        // Going back is taking the value away rather than writing the default down. The two look the same
        // today and are not the same tomorrow: a copy of the default kept in the file would keep out a default
        // the plugin later changes, while a value that is not there at all is the declaration's to answer.
        reset.Click += (_, _) =>
        {
            _settings.Keep(owner, setting, null);

            // Drawn again rather than set here, so that what is on screen is what is in force for every kind
            // alike — a box and a field are not set the same way, and neither should have to be known here.
            Show();
        };

        var row = SettingRow(_i18n.Get(setting.LabelKey), Editor(owner, setting, value), reset);

        var notes = new List<string>();

        if (setting.Default is { Length: > 0 } declared)
        {
            notes.Add(_i18n.Get("setting.default", declared));
        }

        if (setting.RestartRequired)
        {
            notes.Add(_i18n.Get("setting.restart"));
        }

        if (notes.Count == 0)
        {
            return row;
        }

        return new StackPanel
        {
            Spacing = 4,
            Children =
            {
                row,
                new TextBlock
                {
                    Text = string.Join(" · ", notes),
                    Classes = { "caption", "muted" },
                },
            },
        };
    }

    /// <summary>What a setting is chosen with, by its kind.</summary>
    /// <param name="owner">The owner that declared it.</param>
    /// <param name="setting">The setting.</param>
    /// <param name="value">What it is worth now.</param>
    private Control Editor(PluginId owner, PluginSetting setting, string value)
    {
        switch (setting.Kind)
        {
            case SettingKind.Bool:
                var check = new CheckBox { IsChecked = string.Equals(value, "true", StringComparison.Ordinal) };
                check.IsCheckedChanged += (_, _) =>
                    _settings.Keep(owner, setting, check.IsChecked == true ? "true" : "false");

                return check;

            case SettingKind.Choice:
                var options = (setting.Options ?? []).ToArray();
                var choice = new ComboBox
                {
                    ItemsSource = options.Select(option => _i18n.Get(option.LabelKey)).ToArray(),
                    SelectedIndex = Array.FindIndex(options, option => string.Equals(option.Value, value, StringComparison.Ordinal)),
                    MinWidth = 240,
                    VerticalAlignment = VerticalAlignment.Center,
                };
                choice.SelectionChanged += (_, _) =>
                {
                    if (choice.SelectedIndex >= 0)
                    {
                        _settings.Keep(owner, setting, options[choice.SelectedIndex].Value);
                    }
                };

                return choice;

            default:
                var field = new TextBox { Text = value, MinWidth = 240 };

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

                return field;
        }
    }

    /// <summary>The group a setting is shown under, which is the part of its identity before the slash.</summary>
    /// <param name="setting">The setting.</param>
    private static string Group(PluginSetting setting)
    {
        var at = setting.Id.IndexOf('/', StringComparison.Ordinal);

        return at <= 0 ? setting.Id : setting.Id[..at];
    }
}
