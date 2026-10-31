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
/// Making a directory in a directory dock, and the name box the row it makes is given.
/// </summary>
/// <remarks>
/// Making one reads the directory again, which builds the view the new row is drawn in — so the name is not typed in
/// the view that asked for the folder but in the one the reading after it made. What is checked here happens in a real
/// tree on a headless window: the box is there when the row is, and it is in the dock the user is looking at rather
/// than in a dock behind a tab.
/// </remarks>
public sealed class DirectoryEditingTests
{
    /// <summary>A new directory's name is typed as soon as its row is drawn, with no second gesture.</summary>
    [Fact]
    public void ANewDirectoryIsNamedAsSoonAsItsRowIsDrawn()
    {
        var (window, _) = Open();

        try
        {
            var list = window.GetVisualDescendants().OfType<ListBox>().First();

            Assert.True(list.Focus(), "the listing would not take the keyboard");

            window.KeyPress(
                Key.N,
                RawInputModifiers.Control | RawInputModifiers.Shift,
                PhysicalKey.N,
                null
            );

            var box = WaitForBox(window);

            Assert.NotNull(box);
            Assert.False(string.IsNullOrEmpty(box!.Text), "the box was given nothing to name");

            // The whole name is chosen, so typing over it names the directory rather than adding to the one it was given.
            Assert.Equal(0, box.SelectionStart);
            Assert.Equal(box.Text.Length, box.SelectionEnd);

            window.KeyTextInput("Assets");
            window.KeyPress(Key.Enter, RawInputModifiers.None, PhysicalKey.Enter, null);
            Pump();

            // Taken or refused, the box goes: what it said was the row's answer, and the row is drawn again for it.
            Assert.Null(Box(window));
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>
    /// A dock behind another tab is drawn again for the same reading, and does not take the name with it.
    /// </summary>
    /// <remarks>
    /// Every dock showing a directory reads it again, and a dock that is hidden is still a dock that draws: a name
    /// claimed there would be a box in a dock nobody is looking at, and none in the one the user is working in. The
    /// hidden dock is made first so that it is the first of the two to draw.
    /// </remarks>
    [Fact]
    public void ANameIsTypedInTheDockBeingLookedAt()
    {
        var (window, hidden) = Open(hiddenFirst: true);

        try
        {
            var shown = window
                .GetVisualDescendants()
                .OfType<ListBox>()
                .First(list => !IsUnder(list, hidden));

            Assert.True(shown.Focus(), "the listing would not take the keyboard");

            window.KeyPress(
                Key.N,
                RawInputModifiers.Control | RawInputModifiers.Shift,
                PhysicalKey.N,
                null
            );

            var box = WaitForBox(window);

            Assert.NotNull(box);
            Assert.False(IsUnder(box, hidden), "the box is not in the dock the user was working in");
            Assert.False(hidden.IsEffectivelyVisible);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A directory dock over a scratch directory, on a headless window, drawn once.</summary>
    /// <param name="hiddenFirst">Whether a second dock is made before it and hidden, as a dock behind a tab is.</param>
    /// <returns>The window, and the hidden dock when there is one.</returns>
    private static (Window Window, Control Hidden) Open(bool hiddenFirst = false)
    {
        var directory = Scratch.New("directory-editing");
        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(
            directory,
            new HideRegistry(host.Config),
            host.Config,
            host.Files
        );
        var browser = new Browser(host.Log, shared, directory, host.LogicalViews);

        Control Dock() =>
            new DirectoryControl(host, shared, browser, new Clip(host.Log), new LogicalClip());

        var hidden = hiddenFirst ? Dock() : null;
        var shown = Dock();
        var grid = new Grid();

        grid.Children.Add(shown);

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

        return (window, hidden ?? shown);
    }

    /// <summary>Whether a control is one of a dock's own, which is what says which dock a row or box is in.</summary>
    /// <param name="control">The control to look at.</param>
    /// <param name="dock">The dock's control.</param>
    /// <returns>Whether it is under the dock.</returns>
    private static bool IsUnder(Control control, Control dock) =>
        control.GetVisualAncestors().Contains(dock);

    /// <summary>The name box on screen, or nothing while no row is being named.</summary>
    /// <param name="window">The window the docks are in.</param>
    /// <returns>The box.</returns>
    private static TextBox? Box(Window window) =>
        window
            .GetVisualDescendants()
            .OfType<TextBox>()
            .FirstOrDefault(text => text.IsEffectivelyVisible);

    /// <summary>Waits for a name box to appear, which the view puts up a turn after the rows are drawn.</summary>
    /// <param name="window">The window the docks are in.</param>
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
