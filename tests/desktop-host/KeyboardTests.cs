using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Threading;
using Avalonia.VisualTree;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>What the keyboard does in a directory dock, which is where the dock's keys are answered from.</summary>
/// <remarks>
/// A dock answers Ctrl+C, Ctrl+V, F2 and F5 at the top of itself and passes them down to whichever view is being read,
/// so a keyboard that is nowhere in the dock is a press nothing answers — which reads from the outside exactly like a
/// key that arrived and did nothing. The listing is the control that takes the keyboard, and the space around the rows
/// is a place a press lands with no row to focus: the two facts checked here are that the listing can take the keyboard
/// at all, and that a key the clipboard answers reaches the dock once it has.
/// </remarks>
public sealed class KeyboardTests
{
    /// <summary>The listing takes the keyboard, which is what a press in the space around the rows asks of it.</summary>
    /// <remarks>
    /// The toolkit focuses a row where there is one and nothing where there is not, and the view's own press handler
    /// focuses the listing in that case. A listing that cannot take the keyboard leaves it nowhere, and every key the
    /// dock answers is lost until a row is clicked again.
    /// </remarks>
    [Fact]
    public void TheListingTakesTheKeyboard()
    {
        var (window, _, list) = Open();

        Assert.True(list.Focusable);
        Assert.True(list.Focus(), "the listing would not take the keyboard");

        var on = window.FocusManager?.GetFocusedElement();

        Assert.True(
            global::FileSystemPlugin.Keys.Holds(on, list),
            $"the keyboard is on `{on?.GetType().Name ?? "nothing"}` rather than in the listing"
        );

        window.Close();
    }

    /// <summary>The clipboard's key reaches the dock while the listing has the keyboard, with nothing chosen.</summary>
    [Fact]
    public void TheClipboardKeysReachTheDock()
    {
        var (window, dock, list) = Open();

        var seen = new List<Key>();
        dock.AddHandler(
            InputElement.KeyDownEvent,
            (_, e) =>
            {
                if (e.Key is Key.V or Key.C)
                {
                    seen.Add(e.Key);
                }
            },
            RoutingStrategies.Tunnel
        );

        Assert.True(list.Focus(), "the listing would not take the keyboard");

        window.KeyPress(Key.V, RawInputModifiers.Control, PhysicalKey.V, "v");

        Assert.Contains(Key.V, seen);

        window.Close();
    }

    /// <summary>A dock over a scratch directory with one file, shown in a headless window.</summary>
    /// <returns>The window, the dock, and the listing inside it.</returns>
    private static (Window Window, Control Dock, ListBox List) Open()
    {
        var directory = Scratch.New("keyboard");
        File.WriteAllText(Path.Combine(directory, "hero.psd"), "x");

        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);
        var shared = new global::FileSystemPlugin.Shared(
            directory,
            new global::FileSystemPlugin.HideRegistry(host.Config),
            host.Config,
            host.Files
        );
        var browser = new global::FileSystemPlugin.Browser(
            host.Log,
            shared,
            directory,
            host.LogicalViews
        );
        var control = new global::FileSystemPlugin.DirectoryControl(
            host,
            shared,
            browser,
            new global::FileSystemPlugin.Clip(host.Log),
            new global::FileSystemPlugin.LogicalClip()
        );

        var window = new Window { Content = control, Width = 900, Height = 600 };
        window.Show();
        window.Activate();

        // The listing is built when the view is drawn, and a view that has not been drawn has no rows to read.
        for (var turn = 0; turn < 50 && window.GetVisualDescendants().OfType<ListBox>().FirstOrDefault() is null; turn++)
        {
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
            Dispatcher.UIThread.RunJobs();
            Thread.Sleep(10);
        }

        return (window, control, window.GetVisualDescendants().OfType<ListBox>().First());
    }
}
