using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.I18n;
using ContractMenuItem = RorolalaDesktop.Contract.MenuItem;
using DockOpenMode = RorolalaDesktop.Contract.DockOpenMode;
using DockRegistration = RorolalaDesktop.Contract.DockRegistration;

namespace RorolalaDesktop;

/// <summary>
/// The window: the always-present top menu bar, and the dock area below it.
/// </summary>
/// <remarks>
/// Both are filled from what was registered — the kernel's own entries and the plugins' — and a
/// plugin registers during initialization, which is before this window is made, so the menu is built
/// once here and the area is kept in step by the dock manager.
/// </remarks>
public partial class MainWindow : Window
{
    /// <summary>The label key of the menu the registered docks appear under.</summary>
    private const string WindowMenu = "menu.window";

    /// <summary>Everything the host holds.</summary>
    private readonly HostServices _services;

    /// <summary>The docks as they appear in the Window menu, so their checks can be kept in step.</summary>
    private readonly List<(DockRegistration Registration, MenuItem Entry)> _docks = [];

    /// <summary>Makes the window over what was registered.</summary>
    /// <param name="services">Everything the host holds.</param>
    internal MainWindow(HostServices services)
    {
        InitializeComponent();

        _services = services;

        // The window is named where every other word is, so changing what it is called is a change
        // to a translation rather than to this file. A window built with no translations to read —
        // which is what the designer builds — keeps the name the markup gives it.
        if (RolaI18N.TranslationDirectory is not null)
        {
            Title = _services.I18n.Get("window.title");
        }

        BuildMenu();
        DockAreaHost.Content = new DockArea(_services.Docks, _services.I18n);

        // A dock can be closed, opened, or dragged to another region by hand as well as from the
        // menu, so the checks are refreshed and the layout written whenever the docks change — a
        // dock moved by hand is remembered from then on, not only if the run ends tidily.
        _services.Docks.Changed += DocksChanged;

        Opened += (_, _) => ShowPopups();
        Closing += (_, _) => LayoutStore.Save(_services.Log, _services.Docks.Snapshot());

#if DEBUG
        this.AttachDevTools();
#endif
    }

    /// <summary>Builds the menu bar from what was registered.</summary>
    private void BuildMenu()
    {
        MainMenu.Items.Clear();

        foreach (var top in _services.Menu.TopLevels)
        {
            var menu = new MenuItem { Header = _services.I18n.Get(top.LabelKey) };

            foreach (var item in _services.Menu.Items(top.LabelKey))
            {
                menu.Items.Add(Item(item));
            }

            // The registered docks appear under the Window menu, since that is what the dock area
            // holds; that menu's items are therefore not registered as ordinary items.
            if (top.LabelKey == WindowMenu)
            {
                AddDocks(menu);
            }

            MainMenu.Items.Add(menu);
        }
    }

    /// <summary>Makes one item, running its command when chosen.</summary>
    private MenuItem Item(ContractMenuItem item)
    {
        var made = new MenuItem { Header = _services.I18n.Get(item.LabelKey) };
        made.Click += (_, _) => item.Command();

        return made;
    }

    /// <summary>Adds an entry for each registered dock.</summary>
    /// <remarks>
    /// A toggle appears as a toggle, checked while its dock is shown; a dock with open mode New
    /// appears as a plain entry, since choosing it makes another instance rather than switching one.
    /// </remarks>
    private void AddDocks(MenuItem menu)
    {
        var registrations = _services.Docks.Registrations;

        if (registrations.Count > 0)
        {
            menu.Items.Add(new Separator());
        }

        foreach (var registration in registrations)
        {
            var entry = new MenuItem
            {
                Header = _services.Docks.Text(registration.DisplayNameKey),
            };

            if (registration.OpenMode == DockOpenMode.Toggle)
            {
                entry.ToggleType = MenuItemToggleType.CheckBox;
                entry.IsChecked = IsShown(registration.DockNameId);
            }

            var dock = registration;
            entry.Click += (_, _) => _services.Docks.Activate(dock);

            _docks.Add((registration, entry));
            menu.Items.Add(entry);
        }
    }

    /// <summary>Keeps the menu in step with the docks, and writes the layout they came to.</summary>
    private void DocksChanged()
    {
        RefreshDocks();
        LayoutStore.Save(_services.Log, _services.Docks.Snapshot());
    }

    /// <summary>Keeps the toggle checks in step with which docks are shown.</summary>
    private void RefreshDocks()
    {
        foreach (var (registration, entry) in _docks)
        {
            if (registration.OpenMode == DockOpenMode.Toggle)
            {
                entry.IsChecked = IsShown(registration.DockNameId);
            }
        }
    }

    /// <summary>Whether a dock name has an instance that is shown.</summary>
    private bool IsShown(string nameId) =>
        _services.Docks.Instances.Any(instance => instance.DockNameId == nameId && instance.IsOpen);

    /// <summary>
    /// Shows what was raised before the window existed, all in one dialog.
    /// </summary>
    /// <remarks>
    /// A plugin that was not loaded, or a skin that could not be read, is raised while there is
    /// nowhere to put a dialog. It is held and shown here, once, all together — one dialog listing
    /// the trouble is easier to take in than a stack of them, and deduplication has already removed
    /// the repeats.
    /// </remarks>
    private void ShowPopups()
    {
        var pending = _services.Popups.Take();

        if (pending.Count == 0)
        {
            return;
        }

        _ = Notice(string.Join(Environment.NewLine, pending));
    }

    /// <summary>Shows one dialog with the given text.</summary>
    private async Task Notice(string text)
    {
        var dialog = new Window
        {
            Title = _services.I18n.Get("window.notice"),
            Width = 560,
            Height = 300,
            Content = new ScrollViewer
            {
                Content = new TextBlock
                {
                    Text = text,
                    TextWrapping = TextWrapping.Wrap,
                    Margin = new Thickness(14),
                    VerticalAlignment = VerticalAlignment.Top,
                },
            },
        };

        await dialog.ShowDialog(this);
    }
}
