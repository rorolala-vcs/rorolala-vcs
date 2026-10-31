using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Headless;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RorolalaDesktop;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Theming;
using ContractMenuItem = RorolalaDesktop.Contract.MenuItem;
using DesktopIcons = RorolalaDesktop.Icons;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;
using MenuItem = Avalonia.Controls.MenuItem;
using Terminal = global::TerminalPlugin.TerminalPlugin;
using Vcs = global::RorolalaVCSPlugin.RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The picture a dock registers: what the item that opens it in the <c>Window</c> menu wears, and what is drawn at the
/// left of its tab.
/// </summary>
/// <remarks>
/// A dock is a kind of dock rather than a place, and one dock of a kind looks like another until it says otherwise: the
/// picture is what says so at a glance, in the menu that lists the kinds and in the strip that lists what is open. What
/// is checked here is that a registration's key reaches the two marks, that a dock with no key has neither, that the
/// check of an item and its picture are drawn side by side rather than one over the other, and that every dock the
/// shipping plugins register names a picture the library actually holds.
/// </remarks>
public sealed class DockIconTests
{
    /// <summary>Begins each test from no configuration at all, and from no provider registered.</summary>
    /// <remarks>
    /// The plugins are started in process, as the other suite's plugin tests start them, and what they register is
    /// static: what this leaves behind is cleaned away before anything asks.
    /// </remarks>
    public DockIconTests()
    {
        DataHome.Clean();
        HideRegistry.Clear();
        EntryLockProviders.Clear();
        EntryMarkProviders.Clear();
    }

    /// <summary>A dock that names a picture has it in the Window menu item and at the left of its tab.</summary>
    [Fact]
    public void AWindowMenuItemAndItsTabWearTheDocksMark()
    {
        var (_, main, _) = Open(DesktopIcons.Extension);

        // The menu the docks are listed under: the entry for this one wears what the dock is drawn by, beside its word.
        var header = Assert.IsType<StackPanel>(Entry(main).Header);
        var fitted = Assert.IsType<Panel>(header.Children[0]);
        var mark = Assert.IsType<Border>(fitted.Children[0]);

        Assert.NotNull(mark.OpacityMask);

        // And the tab says the same thing in the same picture, beside its word.
        var tab = Tab(main);

        Assert.Contains(
            tab.GetVisualDescendants().OfType<Control>(),
            worn => worn.Classes.Contains(DockArea.IconClass)
        );
        Assert.Equal("it_dock.title", ((Grid)tab.Content!).Children.OfType<TextBlock>().First().Text);
    }

    /// <summary>The room on a tab's left is the picture, its padding, and the room its word kept from it.</summary>
    /// <remarks>
    /// The room between the picture and the word is the room the word kept from the tab's own edge before there was a
    /// picture — widening the padding for the picture widens that side alone, rather than pushing the word away twice.
    /// </remarks>
    [Fact]
    public void ATabKeepsRoomForItsPictureAndItsWord()
    {
        var (_, main, _) = Open(DesktopIcons.Extension);

        var tab = Tab(main);
        var columns = ((Grid)tab.Content!).ColumnDefinitions;
        var mark = tab
            .GetVisualDescendants()
            .OfType<Control>()
            .Single(control => control.Classes.Contains(DockArea.IconClass));
        var markAt = mark.TranslatePoint(default, tab)!.Value;

        Assert.Equal(40.0, columns[0].Width.Value);
        Assert.Equal(26.0, columns[2].Width.Value);

        // The picture is drawn at a whole share of the size the set draws it at, which is what keeps its strokes on
        // pixels rather than between them.
        Assert.Equal(16.0, mark.Bounds.Width);

        // The picture stands at the tab's own edge — one pixel of border before it — and the word begins as much after it
        // as it kept from that edge before there was a picture.
        Assert.InRange(markAt.X, 18.0, 20.0);
        Assert.InRange(columns[0].Width.Value - ((markAt.X - 1) + mark.Bounds.Width), 5.0, 7.0);
    }

    /// <summary>
    /// A dock's item in the Window menu carries its picture in its own header, so the check and the picture cannot be
    /// drawn in one slot.
    /// </summary>
    /// <remarks>
    /// The look's base theme keeps an item's check and its icon in one column, which is where a checked item with a picture
    /// draws the two over each other. The dock's picture is carried by the item's header instead, which the base theme
    /// draws after the check, and the icon slot is left empty.
    /// </remarks>
    [Fact]
    public void AWindowMenuItemKeepsItsCheckAndItsPictureApart()
    {
        var (_, main, _) = Open(DesktopIcons.Extension);
        var entry = Entry(main);

        // The item is a toggle: its check is the base theme's to draw, in the slot the base theme keeps for one.
        Assert.Equal(MenuItemToggleType.CheckBox, entry.ToggleType);
        Assert.Null(entry.Icon);

        // The picture and the word are the item's header, drawn after the check.
        var header = Assert.IsType<StackPanel>(entry.Header);
        var fitted = Assert.IsType<Panel>(header.Children[0]);
        var mark = Assert.IsType<Border>(fitted.Children[0]);
        var words = Assert.IsType<AccessText>(header.Children[1]);

        Assert.NotNull(mark.OpacityMask);
        Assert.Equal("it_dock.title", words.Text);
    }

    /// <summary>A dock that names no picture has no mark, in the menu or in its tab.</summary>
    [Fact]
    public void ADockWithNoPictureWearsNoMark()
    {
        var (_, main, _) = Open(null);

        var header = Assert.IsAssignableFrom<TextBlock>(Entry(main).Header);

        Assert.Equal("it_dock.title", header.Text);
        Assert.DoesNotContain(
            Tab(main).GetVisualDescendants().OfType<Control>(),
            mark => mark.Classes.Contains(DockArea.IconClass)
        );
    }

    /// <summary>Every dock the shipping plugins register names a picture, and the library holds it.</summary>
    /// <remarks>
    /// What a registration names is a key rather than a picture, so a key nothing contributed is a dock with no mark
    /// and a menu item with no picture, which is a thing no test of one plugin's own registrations can see. The three
    /// plugins are started here, as the program starts them, and the library they hand their marks to is read back.
    /// </remarks>
    [Fact]
    public void EveryDockThatShipsNamesAPictureTheLibraryHolds()
    {
        var services = Host.Services();

        new FileSystem().Initialize(services.For(new PluginId(FileSystem.Identity), 0));

        var vcs = new Vcs();

        vcs.Initialize(services.For(vcs.Manifest.Id, 1));

        var terminal = new Terminal();

        terminal.Initialize(services.For(terminal.Manifest.Id, 2));

        Assert.NotEmpty(services.Docks.Registrations);

        Assert.All(
            services.Docks.Registrations,
            registration =>
            {
                Assert.False(
                    string.IsNullOrEmpty(registration.IconKey),
                    $"`{registration.DockNameId}` names no picture"
                );
                Assert.NotNull(services.Icons.Find(registration.IconKey!));
            }
        );
    }

    /// <summary>A dock's item is the height of every other item of the Window menu, and its word is the menu's size.</summary>
    /// <remarks>
    /// The look's base theme sets a menu's word through the presenter it draws it in, and a plain text block put into an
    /// item's header is set at the program's size instead: a row of those is larger than the menu it is in, which is a
    /// thing a test without the look applied cannot see. What is checked here is what a reader sees — the row, and the
    /// word, are the same as an item that says its word alone.
    /// </remarks>
    [Fact]
    public void ADockItemIsTheHeightOfAnyOtherMenuItem()
    {
        var application = Application.Current!;
        var variant = application.RequestedThemeVariant;
        var styles = application.Styles.Count;
        var (theme, _) = ThemeService.Make(application, new ThemeConfiguration());

        theme.Apply();

        try
        {
            var (_, main, _) = Open(DesktopIcons.Extension);
            var window = Window(main);

            window.IsSubMenuOpen = true;
            Pump();

            var popup = window.GetVisualDescendants().OfType<Popup>().Single();
            var items = popup.Child!.GetVisualDescendants().OfType<MenuItem>().ToArray();
            var plain = items.Single(item => HeaderText(item) == "it.open_main");
            var dock = items.Single(item => HeaderText(item) == "it_dock.title");

            Assert.Equal(plain.Bounds.Height, dock.Bounds.Height);
            Assert.Equal(Word(plain).FontSize, Word(dock).FontSize);
        }
        finally
        {
            while (application.Styles.Count > styles)
            {
                application.Styles.RemoveAt(application.Styles.Count - 1);
            }

            application.RequestedThemeVariant = variant;
        }
    }

    /// <summary>The word a menu item draws, over and above the words of its parts.</summary>
    /// <param name="item">The item.</param>
    /// <returns>The word.</returns>
    private static TextBlock Word(MenuItem item) =>
        item.GetVisualDescendants().OfType<TextBlock>().Single(block => block.Text is { Length: > 0 });

    /// <summary>The Window menu this window holds, whose items list the docks.</summary>
    /// <param name="main">The window.</param>
    /// <returns>The menu.</returns>
    private static MenuItem Window(MainWindow main) =>
        main.MainMenu.Items.OfType<MenuItem>().Single(item => HeaderText(item) == "menu.window");

    /// <summary>The item a dock is opened by, as it stands before its menu is opened.</summary>
    /// <param name="main">The window.</param>
    /// <returns>The item.</returns>
    private static MenuItem Entry(MainWindow main) =>
        Window(main).Items.OfType<MenuItem>().Single(item => HeaderText(item) == "it_dock.title");

    /// <summary>The words a menu item says, which it may say as a word or draw beside its picture.</summary>
    /// <param name="item">The item.</param>
    /// <returns>The words.</returns>
    private static string HeaderText(MenuItem item) =>
        item.Header switch
        {
            string said => said,
            TextBlock text => text.Text ?? string.Empty,
            StackPanel { Children: [Control, TextBlock words] } => words.Text ?? string.Empty,
            _ => string.Empty,
        };

    /// <summary>The one tab of the dock, as the window draws it.</summary>
    /// <param name="main">The window.</param>
    /// <returns>The tab.</returns>
    private static Button Tab(MainWindow main) =>
        main.GetVisualDescendants()
            .OfType<Button>()
            .Single(button => button.Classes.Contains(DockArea.TitleClass));

    /// <summary>Opens a dock that names the picture given, in the main window, and lays the window out.</summary>
    /// <param name="iconKey">The picture the dock is drawn by, or nothing for a dock that names none.</param>
    /// <returns>Everything the host holds, the window, and the dock.</returns>
    private static (HostServices Services, MainWindow Main, DockInstance Dock) Open(string? iconKey)
    {
        var services = Host.Services();

        DesktopIcons.Declare(services.Icons);
        services.Menu.AddTopLevel(new PluginId("it"), 0, "menu.window", 100);
        services.Menu.AddItem(
            new PluginId("it"),
            0,
            "menu.window",
            new ContractMenuItem("it.open_main", 0, () => { })
        );
        services.Windows.Make = (key, dock) => new MainWindow(services, key, dock);

        var registration = new DockRegistration(
            new PluginId("it"),
            "it.icon.dock",
            "it_dock.title",
            DockOpenMode.Toggle,
            DockSide.Center,
            () => new FakeDockView { Control = new Border() },
            iconKey
        );

        services.Docks.Register(registration);

        var dock = services.Docks.Activate(registration)!;

        services.Windows.OpenMain();

        var main = (MainWindow)services.Shell.Window!;

        Pump();

        return (services, main, dock);
    }

    /// <summary>Lets the windows run a few turns of their loop, which is what lays a dock area out.</summary>
    private static void Pump()
    {
        for (var turn = 0; turn < 30; turn++)
        {
            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        }

        Dispatcher.UIThread.RunJobs();
    }
}
