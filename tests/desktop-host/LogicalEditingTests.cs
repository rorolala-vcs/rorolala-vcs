using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Input;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// Typing a name in a place's listing: the box a new folder is given, and what a redraw arriving mid-name must not take.
/// </summary>
/// <remarks>
/// The box is a child of the row, which is a child of the view, which is what a redraw replaces. So what is checked
/// here happens in a real tree on a headless window: the box is there when the folder is, and a reading landing while
/// a name is being typed leaves the box and the words in it where they are.
/// </remarks>
public sealed class LogicalEditingTests
{
    /// <summary>A new folder's name is typed as soon as its row is drawn, with no second gesture.</summary>
    [Fact]
    public void ANewFolderIsNamedAsSoonAsItsRowIsDrawn()
    {
        var (window, provider, _) = Open();

        try
        {
            var list = Listing(window);
            Assert.True(list.Focus(), "the listing would not take the keyboard");

            window.KeyPress(Key.N, RawInputModifiers.Control | RawInputModifiers.Shift, PhysicalKey.N, null);

            var box = WaitForBox(window);

            Assert.NotNull(box);

            var written = box!.Text ?? string.Empty;

            Assert.Equal(provider.Made, written);

            // The whole name is chosen, so typing over it names the folder rather than adding to the name it was given.
            Assert.Equal(0, box.SelectionStart);
            Assert.Equal(written.Length, box.SelectionEnd);

            window.KeyTextInput("Shots");
            window.KeyPress(Key.Enter, RawInputModifiers.None, PhysicalKey.Enter, null);
            Pump();

            Assert.Contains(provider.Renamed, taken => taken.Name == "Shots");
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A reading landing while a name is being typed keeps the box and what was typed in it.</summary>
    /// <remarks>
    /// The drawing is owed rather than dropped, which is the second half: the row that arrived while the name was being
    /// typed is there once the name is settled, so an edit does not leave the listing stale either.
    /// </remarks>
    [Fact]
    public void ARedrawWhileNamingKeepsTheBoxAndItsWords()
    {
        var (window, provider, _) = Open();

        try
        {
            Assert.True(Listing(window).Focus(), "the listing would not take the keyboard");

            window.KeyPress(Key.N, RawInputModifiers.Control | RawInputModifiers.Shift, PhysicalKey.N, null);

            var box = WaitForBox(window);

            Assert.NotNull(box);

            // The listing the box was put in, which is the one a redraw must not replace while the box is up.
            var list = Listing(window);

            window.KeyTextInput("Shots");

            Assert.Equal("Shots", box!.Text);

            // A row arrives while the name is being typed, which is a reading landing and the place saying so.
            provider.Holds("later.psd");
            provider.Redraw();
            Pump();

            // The same box with the same words, and the same listing: drawing would have replaced the row the box is in.
            Assert.Same(box, Box(window));
            Assert.Equal("Shots", box.Text);
            Assert.Same(list, Listing(window));
            Assert.Equal(1, list.ItemCount);

            window.KeyPress(Key.Enter, RawInputModifiers.None, PhysicalKey.Enter, null);
            Pump();

            // The name is taken, and what was owed is drawn: the row that arrived while the name was being typed.
            Assert.Contains(provider.Renamed, taken => taken.Name == "Shots");
            Assert.Equal(2, Listing(window).ItemCount);
            Assert.Null(Box(window));
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>Every folder made is named in turn, so a second one is not left with the name it was given.</summary>
    [Fact]
    public void EveryFolderMadeIsNamedInTurn()
    {
        var (window, provider, _) = Open();

        try
        {
            var list = Listing(window);
            Assert.True(list.Focus(), "the listing would not take the keyboard");

            window.KeyPress(Key.N, RawInputModifiers.Control | RawInputModifiers.Shift, PhysicalKey.N, null);

            var first = WaitForBox(window);

            Assert.NotNull(first);

            window.KeyTextInput("Shots");
            window.KeyPress(Key.Enter, RawInputModifiers.None, PhysicalKey.Enter, null);
            Pump();

            Assert.True(Listing(window).Focus(), "the listing would not take the keyboard back");

            window.KeyPress(Key.N, RawInputModifiers.Control | RawInputModifiers.Shift, PhysicalKey.N, null);

            var second = WaitForBox(window);

            Assert.NotNull(second);

            Assert.Equal(provider.Made, second!.Text);
            Assert.Contains("New folder", second.Text, StringComparison.Ordinal);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>Keys pressed while a name is being typed act on the name, not on the listing behind it.</summary>
    /// <remarks>
    /// The dock answers its keys at the top of itself, so a key that reached the listing while a box was up would be a
    /// row removed, a second box opened or a second folder made, under a name the user was still typing.
    /// </remarks>
    [Fact]
    public void KeysPressedWhileNamingDoNotActOnTheListing()
    {
        var (window, provider, _) = Open();

        try
        {
            Assert.True(Listing(window).Focus(), "the listing would not take the keyboard");

            window.KeyPress(Key.N, RawInputModifiers.Control | RawInputModifiers.Shift, PhysicalKey.N, null);

            var box = WaitForBox(window);

            Assert.NotNull(box);

            window.KeyTextInput("Shots");
            window.KeyPress(Key.Delete, RawInputModifiers.None, PhysicalKey.Delete, null);
            window.KeyPress(Key.F2, RawInputModifiers.None, PhysicalKey.F2, null);
            window.KeyPress(Key.N, RawInputModifiers.Control | RawInputModifiers.Shift, PhysicalKey.N, null);
            Pump();

            // The same box, the words still in it, nothing removed and no second folder made.
            Assert.Same(box, Box(window));
            Assert.Equal("Shots", box!.Text);
            Assert.Empty(provider.Removed);
            Assert.Equal(1, provider.RowCount);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>
    /// A name the place refuses puts the row back, rather than leaving a field nothing answers in.
    /// </summary>
    /// <remarks>
    /// A place names a row without the files changing, so nothing in the file system reads the listing again after the
    /// edit: the box has to be taken back where it came from, and the row has to show the name it still has.
    /// </remarks>
    [Fact]
    public void ANameThePlaceRefusesPutsTheRowBack()
    {
        var (window, provider, _) = Open(rows: ["Photos"]);

        try
        {
            provider.RefusesNames = true;

            Assert.True(Listing(window).Focus(), "the listing would not take the keyboard");

            // One row, chosen from the keyboard: what a key acts on where nothing was clicked.
            window.KeyPress(Key.Down, RawInputModifiers.None, PhysicalKey.ArrowDown, null);
            Pump();
            window.KeyPress(Key.F2, RawInputModifiers.None, PhysicalKey.F2, null);

            var box = WaitForBox(window);

            Assert.NotNull(box);

            window.KeyTextInput("Shots");
            window.KeyPress(Key.Enter, RawInputModifiers.None, PhysicalKey.Enter, null);
            Pump();

            Assert.Null(Box(window));
            Assert.Empty(provider.Renamed);
            Assert.Equal(
                ["Photos"],
                Listing(window)
                    .ItemsSource!.Cast<LogicalEntry>()
                    .Select(entry => entry.Name)
            );
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>
    /// A place's dock behind another tab is drawn again for the same reading, and does not take the name with it.
    /// </summary>
    /// <remarks>
    /// Every dock showing a place is read again when the place says it changed, and a dock that is hidden is still a
    /// dock that draws: a name claimed there would be a box in a dock nobody is looking at. The hidden dock is made
    /// first, so that it is the first of the two to draw.
    /// </remarks>
    [Fact]
    public void ANameIsTypedInTheOpenDock()
    {
        var (window, provider, _, hidden) = OpenWithHiddenDock();

        try
        {
            var shown = window
                .GetVisualDescendants()
                .OfType<ListBox>()
                .First(list => !list.GetVisualAncestors().Contains(hidden));

            Assert.True(shown.Focus(), "the listing would not take the keyboard");

            window.KeyPress(Key.N, RawInputModifiers.Control | RawInputModifiers.Shift, PhysicalKey.N, null);

            var box = WaitForBox(window);

            Assert.NotNull(box);
            Assert.False(
                box!.GetVisualAncestors().Contains(hidden),
                "the box is not in the dock the user was working in"
            );
            Assert.False(hidden.IsEffectivelyVisible);
            Assert.Equal(provider.Made, box.Text);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A dock over a place, on a headless window, drawn once.</summary>
    /// <param name="rows">The suggested folders the place holds to begin with.</param>
    /// <returns>The window, the place answering it, and the dock.</returns>
    private static (Window Window, VirtualPlace Provider, DirectoryControl Dock) Open(params string[] rows)
    {
        var (window, provider, dock, _) = SetUp(rows, hiddenDock: false);

        return (window, provider, dock);
    }

    /// <summary>A dock over a place with another, hidden, dock over the same place made before it.</summary>
    /// <param name="rows">The suggested folders the place holds to begin with.</param>
    /// <returns>The window, the place, the dock, and the hidden one.</returns>
    private static (Window Window, VirtualPlace Provider, DirectoryControl Dock, Control Hidden) OpenWithHiddenDock(
        params string[] rows
    )
    {
        var (window, provider, dock, hidden) = SetUp(rows, hiddenDock: true);

        return (window, provider, dock, hidden!);
    }

    /// <summary>
    /// A window over one place, with one dock and possibly another hidden one.
    /// </summary>
    /// <remarks>
    /// The rows are made before the window is drawn, so that the first read of the place is the answer the listing is
    /// built from, which is what a row the dock is showing always is.
    /// </remarks>
    /// <param name="rows">The suggested folders the place holds to begin with.</param>
    /// <param name="hiddenDock">Whether a second dock, hidden as one behind a tab is, is made before this one.</param>
    /// <returns>The window, the place answering it, the dock, and the hidden one when there is one.</returns>
    private static (Window Window, VirtualPlace Provider, DirectoryControl Dock, Control? Hidden) SetUp(
        string[] rows,
        bool hiddenDock
    )
    {
        var directory = Scratch.New("logical-editing");
        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(
            directory,
            new HideRegistry(host.Config),
            host.Config,
            host.Files
        );
        var provider = new VirtualPlace(host);
        var address = new LogicalAddress("logic", "somewhere");

        foreach (var row in rows)
        {
            _ = provider.Suggest(row);
        }

        host.LogicalViews.Register(address.Scheme, provider);

        var browser = new Browser(host.Log, shared, address.ToString(), host.LogicalViews);

        DirectoryControl Dock() =>
            new(host, shared, browser, new Clip(host.Log), new LogicalClip());

        var hidden = hiddenDock ? Dock() : null;
        var dock = Dock();
        var grid = new Grid();

        grid.Children.Add(dock);

        if (hidden is not null)
        {
            hidden.IsVisible = false;
            grid.Children.Add(hidden);
        }

        var window = new Window
        {
            Width = 900,
            Height = 600,
            Content = grid,
        };

        window.Show();
        window.Activate();
        Pump();

        return (window, provider, dock, hidden);
    }

    /// <summary>The listing the dock is drawing now, which is a new one after every read.</summary>
    /// <param name="window">The window the dock is in.</param>
    /// <returns>The rows.</returns>
    private static ListBox Listing(Window window) =>
        window.GetVisualDescendants().OfType<ListBox>().First();

    /// <summary>The name box on screen, or nothing while no row is being named.</summary>
    /// <param name="window">The window the dock is in.</param>
    /// <returns>The box.</returns>
    private static TextBox? Box(Window window) =>
        window
            .GetVisualDescendants()
            .OfType<TextBox>()
            .FirstOrDefault(text => text.IsEffectivelyVisible);

    /// <summary>Waits for a name box to appear, which the view puts up a turn after the rows are drawn.</summary>
    /// <param name="window">The window the dock is in.</param>
    /// <returns>The box.</returns>
    private static TextBox? WaitForBox(Window window)
    {
        for (var turn = 0; turn < 40; turn++)
        {
            Pump(turns: 1);

            if (Box(window) is { } box)
            {
                return box;
            }
        }

        return null;
    }

    /// <summary>Lets the loop run what is queued and the window lay itself out.</summary>
    /// <param name="turns">How many turns to pump.</param>
    private static void Pump(int turns = 40)
    {
        for (var turn = 0; turn < turns; turn++)
        {
            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
            Thread.Sleep(5);
        }
    }
}
