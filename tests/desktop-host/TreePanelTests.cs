using System.Diagnostics;

using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Threading;
using Avalonia.VisualTree;

using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The Tree panel: the description it writes to a tree's mark, and the staging area it draws.
/// </summary>
/// <remarks>
/// The panel is a view of a tree the browser chose, so a test names the tree through
/// <see cref="ChosenDirectory"/> and reads what the panel drew. The tree's own reading is the native
/// one, so the fixture is a real tree made by the command line rather than a stand-in for one.
/// </remarks>
public sealed class TreePanelTests
{
    /// <summary>A description typed into the panel is written to the tree's mark without a save.</summary>
    [Fact]
    public void ADescriptionIsWrittenToTheTreesMark()
    {
        var tree = TreeMark("description");
        var services = Host.Services();
        var view = Panel(services);
        var window = new Window { Width = 520, Height = 520, Content = view };

        window.Show();

        try
        {
            BaseDirectory.Publish(tree);
            ChosenDirectory.Publish(new Entry(tree, EntryKind.Directory));
            Pump();

            var field = Field(window);

            Assert.True(field.IsEnabled, "the description field is not enabled for a tree");
            Assert.Equal(
                Path.GetFileName(Path.TrimEndingDirectorySeparator(tree)),
                view.Title
            );

            field.Text = "the next version";
            Pump();

            Assert.Equal(
                "the next version",
                File.ReadAllText(TreePanelCommands.Description.PathOf(tree))
            );

            field.Text = "  ";
            Pump();

            Assert.False(
                File.Exists(TreePanelCommands.Description.PathOf(tree)),
                "a blank description left the file behind"
            );
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A tree's changed path is drawn, with the checkbox that stages it.</summary>
    [Fact]
    public void AChangedPathIsDrawnWithItsStageCheckbox()
    {
        var workspace = TreeRecorded("panel");
        var tree = Path.Combine(workspace, "tree");

        File.WriteAllText(Path.Combine(tree, "a.txt"), "two\n");

        var services = Host.Services();
        var view = Panel(services);
        var window = new Window { Width = 520, Height = 620, Content = view };

        window.Show();

        try
        {
            BaseDirectory.Publish(workspace);
            ChosenDirectory.Publish(new Entry(tree, EntryKind.Directory));

            var texts = Until(
                window,
                () =>
                {
                    var shown = window
                        .GetVisualDescendants()
                        .OfType<TextBlock>()
                        .Select(text => text.Text ?? string.Empty)
                        .ToArray();

                    return shown.Contains("a.txt") ? shown : null;
                }
            );

            Assert.Contains("a.txt", texts, StringComparer.Ordinal);

            var boxes = window
                .GetVisualDescendants()
                .OfType<CheckBox>()
                .Select(box => (Box: box, Tip: ToolTip.GetTip(box) as string))
                .ToArray();

            Assert.Contains(
                boxes,
                one => one.Tip == "rorolala_vcs.tree_panel.stage"
            );

            var stageBox = boxes.First(one => one.Tip == "rorolala_vcs.tree_panel.stage").Box;
            var allBox = boxes.First(one => one.Tip == "rorolala_vcs.tree_panel.stage_all").Box;
            var stageAt = stageBox.TranslatePoint(new Point(0, 0), window);
            var allAt = allBox.TranslatePoint(new Point(0, 0), window);

            Assert.NotNull(stageAt);
            Assert.NotNull(allAt);
            Assert.Equal(0, allAt.Value.X.CompareTo(stageAt.Value.X));
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>The two lists stand above each other in a narrow dock and beside each other when wide.</summary>
    [Fact]
    public void TheListsChangeFromAboveToBesideAtTheWidth()
    {
        var tree = TreeMark("wide");
        var services = Host.Services();
        var view = Panel(services);
        var window = new Window { Width = 250, Height = 500, Content = view };

        window.Show();

        try
        {
            BaseDirectory.Publish(tree);
            ChosenDirectory.Publish(new Entry(tree, EntryKind.Directory));
            Pump();

            Assert.False(view.IsWide, "the lists were beside each other in a narrow dock");

            window.Width = 500;
            Pump();

            Assert.True(view.IsWide, "the lists stayed above each other in a wide dock");
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A tree with many paths makes rows for the viewport rather than for every path.</summary>
    [Fact]
    public void ManyPathsDoNotMakeEveryRow()
    {
        var workspace = TreeRecorded("many");
        var tree = Path.Combine(workspace, "tree");

        for (var index = 0; index < 300; index++)
        {
            File.WriteAllText(Path.Combine(tree, $"{index}.txt"), "one\n");
        }

        Given(workspace, "tree", "add", "tree");
        Given(workspace, "track", "tree", "--message", "many", "--no-editor");

        for (var index = 0; index < 300; index++)
        {
            File.WriteAllText(Path.Combine(tree, $"{index}.txt"), "changed\n");
        }

        var services = Host.Services();
        var view = Panel(services);
        var window = new Window { Width = 500, Height = 500, Content = view };

        window.Show();

        try
        {
            BaseDirectory.Publish(workspace);
            ChosenDirectory.Publish(new Entry(tree, EntryKind.Directory));

            var boxes = Until(
                window,
                () =>
                {
                    var found = window
                        .GetVisualDescendants()
                        .OfType<CheckBox>()
                        .Where(box => ToolTip.GetTip(box) as string == "rorolala_vcs.tree_panel.stage")
                        .ToArray();

                    return found.Length > 0 ? found : null;
                }
            );

            Assert.True(
                boxes.Length < 100,
                $"the panel made {boxes.Length} row checkboxes for 300 paths"
            );

            var all = window
                .GetVisualDescendants()
                .OfType<CheckBox>()
                .First(box => ToolTip.GetTip(box) as string == "rorolala_vcs.tree_panel.stage_all");
            var stageAt = boxes[0].TranslatePoint(new Point(0, 0), window);
            var allAt = all.TranslatePoint(new Point(0, 0), window);

            Assert.NotNull(stageAt);
            Assert.NotNull(allAt);
            Assert.Equal(0, allAt.Value.X.CompareTo(stageAt.Value.X));
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A tree with a mark but no workspace, which is enough for the description field.</summary>
    /// <param name="label">What the scratch directory is for.</param>
    /// <returns>The tree's root.</returns>
    private static string TreeMark(string label)
    {
        var tree = Scratch.New($"tree-panel-{label}");

        Directory.CreateDirectory(Path.Combine(tree, ".rola-tree"));

        return tree;
    }

    /// <summary>Makes the panel over a plugin-scoped host.</summary>
    /// <param name="services">The host's services.</param>
    /// <returns>The panel.</returns>
    private static TreePanelView Panel(HostServices services)
    {
        var host = services.For(new PluginId("it.tree.panel"), 1);

        return new TreePanelView(
            host,
            host.Progress,
            new TreeReadings(host.Log, host.Files)
        );
    }

    /// <summary>The panel's description field.</summary>
    /// <param name="window">The window holding the panel.</param>
    /// <returns>The field.</returns>
    private static TextBox Field(Window window) =>
        window.GetVisualDescendants().OfType<TextBox>().Single();

    /// <summary>What a view says, once it says it.</summary>
    /// <param name="window">The window holding it.</param>
    /// <param name="read">What is being waited for, or nothing while it is not there.</param>
    /// <typeparam name="T">What is being waited for.</typeparam>
    /// <returns>What was read.</returns>
    private static T Until<T>(Window window, Func<T?> read)
        where T : class
    {
        for (var frame = 0; frame < 400; frame++)
        {
            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();

            if (read() is { } found)
            {
                return found;
            }

            Thread.Sleep(5);
        }

        throw new TimeoutException("the panel did not say what the test waited for");
    }

    /// <summary>Runs the dispatcher and the render timer for a moment.</summary>
    private static void Pump()
    {
        for (var frame = 0; frame < 20; frame++)
        {
            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        }
    }

    /// <summary>A workspace whose one recorded entry is a tree.</summary>
    /// <param name="label">The scratch directory's label.</param>
    /// <returns>The workspace.</returns>
    private static string TreeRecorded(string label)
    {
        var parent = Scratch.New($"tree-panel-{label}");
        var workspace = Path.Combine(parent, "ws");
        var tree = Path.Combine(workspace, "tree");

        Given(parent, "create", workspace);
        Given(workspace, "key", "generate", "--install", "alice");
        Given(workspace, "account", "alice");

        Directory.CreateDirectory(tree);
        File.WriteAllText(Path.Combine(tree, "a.txt"), "one\n");

        Given(workspace, "tree", "init", "tree");
        Given(workspace, "tree", "add", "tree");
        Given(workspace, "track", "tree", "--message", "first tree", "--no-editor");

        return workspace;
    }

    /// <summary>Runs one command in a directory, failing the test when it does not end successfully.</summary>
    /// <param name="directory">Where to run it.</param>
    /// <param name="arguments">What the command is told.</param>
    private static void Given(string directory, params string[] arguments)
    {
        var start = new ProcessStartInfo(Program()) { WorkingDirectory = directory };

        foreach (var argument in arguments)
        {
            start.ArgumentList.Add(argument);
        }

        using var command = Process.Start(start)!;

        command.WaitForExit();

        Assert.Equal(0, command.ExitCode);
    }

    /// <summary>The built command line, whose directory the suites are told.</summary>
    /// <returns>The program to run.</returns>
    private static string Program() =>
        Path.Combine(
            Environment.GetEnvironmentVariable("ROLA_BIN_DIR")
                ?? throw new InvalidOperationException("the suites are run with ROLA_BIN_DIR"),
            OperatingSystem.IsWindows() ? "rola.exe" : "rola"
        );
}
