using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What a row wears is asked again when the answers behind it may have changed.
/// </summary>
/// <remarks>
/// A badge is read as its row is made, so a view that keeps the rows it has keeps the badges it had: an answer
/// that was let go of and read again needs the rows that draw from it to be made again, whether that answer came
/// from a command of the program's or from the watcher.
/// </remarks>
public sealed class BadgeRefreshTests
{
    /// <summary>Begins each test from no provider of its own.</summary>
    public BadgeRefreshTests()
    {
        DataHome.Clean();
        global::FileSystemPlugin.HideRegistry.Clear();
        EntryMarkProviders.Clear();
        EntryColourProviders.Clear();
        EntryLockProviders.Clear();
    }

    /// <summary>A row keeps the badge it wears when the listing beside it is refreshed.</summary>
    [Fact]
    public void ATreeRowKeepsItsBadgeWhenTheListingIsRefreshed()
    {
        var root = Scratch.New("tree-badge");
        var file = Path.Combine(root, "hero.psd");

        File.WriteAllText(file, "x");

        var host = Host.Services().For(new PluginId(FileSystem.Identity), 1);

        Icons.Declare(host.Icons);
        var marking = new Marking();
        EntryMarkProviders.Register(marking);

        global::FileSystemPlugin.HideRegistry.Declare(host);

        var shared = new Shared(
            root,
            new global::FileSystemPlugin.HideRegistry(host.Config),
            host.Config,
            host.Files
        );
        var browser = new Browser(host.Log, shared, root, host.LogicalViews);
        var dock = new TreeDock(host, browser, new Clip(host.Log));
        var window = new Window
        {
            Content = dock.View,
            Width = 900,
            Height = 600,
        };

        window.Show();
        window.Activate();

        try
        {
            var baseRow = Until(() => window.GetVisualDescendants().OfType<TreeView>().FirstOrDefault()?.Items.OfType<TreeViewItem>().FirstOrDefault());

            baseRow.IsExpanded = true;
            Pump();

            var row = Until(() => window.GetVisualDescendants().OfType<TreeViewItem>().FirstOrDefault(step => Same(step.Tag as string, file)));

            Assert.NotEmpty(Badges(row));

            var asked = marking.Asked;

            // The listing's refresh reads its own directory again. The tree reads the same directories, and what a
            // row wears is read from those same answers.
            browser.Refresh();
            Pump();

            row = Until(() => window.GetVisualDescendants().OfType<TreeViewItem>().FirstOrDefault(step => Same(step.Tag as string, file)));

            Assert.True(marking.Asked > asked, "the tree kept rows it should have asked again");
            Assert.NotNull(host.Icons.Find(Icons.Dot));

            var again = Until(() =>
            {
                var found = window.GetVisualDescendants().OfType<TreeViewItem>().FirstOrDefault(step => Same(step.Tag as string, file));

                return found is not null && Badges(found).Count > 0 ? Badges(found) : null;
            });

            Assert.NotEmpty(again);
        }
        finally
        {
            window.Close();
            browser.Dispose();
        }
    }

    /// <summary>A touch makes a listing that keeps its rows ask what they wear again.</summary>
    [Fact]
    public void ATouchMakesAGridAskItsMarksAgain()
    {
        var root = Scratch.New("grid-badge");
        File.WriteAllText(Path.Combine(root, "hero.psd"), "x");
        File.WriteAllText(Path.Combine(root, "other.psd"), "x");

        var host = Host.Services().For(new PluginId(FileSystem.Identity), 1);

        Icons.Declare(host.Icons);

        var marking = new Marking();

        EntryMarkProviders.Register(marking);

        var shared = new Shared(root, new HideRegistry(host.Config), host.Config, host.Files);
        var browser = new Browser(host.Log, shared, root, host.LogicalViews);
        var control = new DirectoryControl(host, shared, browser, new Clip(host.Log), new LogicalClip());
        var window = new Window { Width = 900, Height = 600, Content = control };

        window.Show();
        window.Activate();

        try
        {
            control.OpenAt(root);
            Pump();

            Assert.True(marking.Asked > 0, "the rows were never asked what they wear");

            var before = marking.Asked;
            var shown = browser.Shown;
            var changed = 0;

            browser.Changed += () => changed++;

            // A command of the program's changes what an answer says. It is said through the plugin, which is what
            // lets go of the answers and reads the directory again; the rows drawn from them are asked again.
            browser.Touch();
            Pump();

            Assert.True(changed > 0, "the touch never reached the location");
            Assert.NotSame(shown, browser.Shown);
            Assert.True(marking.Asked > before, "the grid kept rows it should have asked again");
        }
        finally
        {
            window.Close();
            browser.Dispose();
        }
    }

    /// <summary>The strip a row wears its badges in, which is empty when it wears none.</summary>
    /// <param name="row">The row.</param>
    /// <returns>The badges, or nothing when the row has no strip.</returns>
    private static IReadOnlyList<Control> Badges(TreeViewItem row) =>
        [
            .. row.GetVisualDescendants()
                .OfType<StackPanel>()
                .Where(panel => panel.Name == "entry-badges")
                .SelectMany(panel => panel.Children),
        ];

    /// <summary>A provider that speaks for every directory and says one thing about every entry.</summary>
    private sealed class Marking : IEntryMarkProvider
    {
        /// <inheritdoc />
        public string Id => "it.mark.refresh";

        /// <summary>How many times it was asked about an entry.</summary>
        public int Asked { get; private set; }

        /// <inheritdoc />
        public bool Applies(string directory) => true;

        /// <inheritdoc />
        public EntryMark Mark(Entry entry)
        {
            Asked++;

            return new EntryMark(IconKey: Icons.Dot, Ink: MarkInk.Accent);
        }
    }

    /// <summary>Waits for something a turn or so late to be drawn.</summary>
    /// <typeparam name="T">What is waited for.</typeparam>
    /// <param name="until">What is waited for, answering nothing until it is there.</param>
    /// <returns>What was waited for.</returns>
    private static T Until<T>(Func<T?> until)
        where T : class
    {
        for (var turn = 0; turn < 200; turn++)
        {
            if (until() is { } there)
            {
                return there;
            }

            Pump();
            Thread.Sleep(10);
        }

        throw new InvalidOperationException("what was waited for never appeared");
    }

    /// <summary>Draws the window for a few turns.</summary>
    private static void Pump()
    {
        for (var turn = 0; turn < 20; turn++)
        {
            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        }

        Dispatcher.UIThread.RunJobs();
    }

    /// <summary>Whether two paths name the same place.</summary>
    /// <param name="left">One path, or nothing.</param>
    /// <param name="right">The other.</param>
    /// <returns>Whether they are the same place.</returns>
    private static bool Same(string? left, string? right) =>
        left is not null
        && right is not null
        && string.Equals(Full(left), Full(right), OperatingSystem.IsWindows()
            ? StringComparison.OrdinalIgnoreCase
            : StringComparison.Ordinal);

    /// <summary>A path as it is compared.</summary>
    /// <param name="path">The path.</param>
    /// <returns>The path, made full and without a trailing separator.</returns>
    private static string Full(string path) => Path.TrimEndingDirectorySeparator(Path.GetFullPath(path));
}
