using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Headless;
using Avalonia.Input;
using Avalonia.Threading;
using Avalonia.VisualTree;
using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What a press on a step of the tree does: the chevron of a step opens or closes that step alone, and a press
/// on the row goes there and leaves the step open.
/// </summary>
/// <remarks>
/// The two are one gesture apart and used to be one gesture: a press on a row that toggled closed on its second
/// press made walking into a directory a press that took the tree a step back, so closing a step is its chevron's
/// to do. Checked through presses rather than by reading the handlers, because a press is what the toolkit routes
/// — the chevron and the row are separate controls in the row's template, and which of them a press reaches is
/// the whole question.
/// </remarks>
public sealed class TreeTests
{
    /// <summary>
    /// Begins each test with no configuration of its own, and with no provider registered.
    /// </summary>
    /// <remarks>
    /// The providers that hide entries are the run's rather than a view's (see
    /// <see cref="global::FileSystemPlugin.HideRegistry"/>), so a test that drove a listing before this one may
    /// have left a rule in force that hides the whole of a scratch directory. A tree with nothing under it is a
    /// tree with no chevron and nothing to open, which is a test of nothing at all — and it reads from the
    /// outside exactly like a press that did not work.
    /// </remarks>
    public TreeTests()
    {
        DataHome.Clean();
        global::FileSystemPlugin.HideRegistry.Clear();
    }

    /// <summary>The chevron opens a step and closes it again without touching the steps under it.</summary>
    /// <remarks>
    /// The steps under a step are closed with it only in the sense that they are not drawn: whether they were
    /// open is what the tree remembers, and opening the step again has to show them as they were rather than
    /// collapsed to one level.
    /// </remarks>
    [Fact]
    public void TheChevronTogglesTheStepItBelongsToAlone()
    {
        var (window, root) = Open();

        Assert.False(root.IsExpanded, "the tree opened a step with nothing to say it should be");

        Press(window, Chevron(root));

        Assert.True(root.IsExpanded, "the chevron did not open the step");

        var alpha = Until(window, () => Step(root, "alpha"));

        Assert.False(alpha.IsExpanded);

        Press(window, Chevron(alpha));

        Assert.True(alpha.IsExpanded, "the chevron of a step under the root did not open it");

        Press(window, Chevron(root));

        Assert.False(root.IsExpanded, "the chevron did not close the step");
        Assert.True(alpha.IsExpanded, "closing a step closed the steps under it as well");
        Assert.Contains(alpha, root.Items.OfType<TreeViewItem>());

        window.Close();
    }

    /// <summary>
    /// A press on the row of a directory goes there and leaves the step open, and the steps already open under
    /// it are left open as well.
    /// </summary>
    /// <remarks>
    /// A row that closed on a second press is a row that undoes the walk a user is making: the place under the
    /// pointer is the place they are in, and a press there is never a request to put it away. Going to the place
    /// the location is already in reads the directory again, which builds the tree again, so what is checked is
    /// the tree as it is drawn after the press rather than the row that was pressed — the two are not always the
    /// same row, and the one the user goes on looking at is the drawn one.
    /// </remarks>
    [Fact]
    public void APressOnADirectoryRowLeavesItOpen()
    {
        var (window, root) = Open();
        TreeViewItem alpha;

        Press(window, Row(root));
        (root, alpha) = Drawn(window);

        Assert.True(root.IsExpanded, "a press on the row of a closed step did not leave it open");

        Press(window, Row(root));
        (root, alpha) = Drawn(window);

        Assert.True(root.IsExpanded, "a second press on the row closed the step");

        Press(window, Row(alpha));
        (root, alpha) = Drawn(window);

        Assert.True(alpha.IsExpanded, "a press on a step under the root did not leave it open");

        Press(window, Row(alpha));
        (root, alpha) = Drawn(window);

        Assert.True(alpha.IsExpanded, "a second press on a step under the root closed it");

        Press(window, Row(root));
        (root, alpha) = Drawn(window);

        Assert.True(root.IsExpanded, "a press on the root did not leave it open");
        Assert.True(alpha.IsExpanded, "a press on the root closed the step that was open under it");

        window.Close();
    }

    /// <summary>A tree over a scratch directory holding one directory, one directory under that, and one file.</summary>
    /// <returns>The window it is shown in, and the step the tree is rooted at.</returns>
    private static (Window Window, TreeViewItem Root) Open()
    {
        var directory = Scratch.New("tree");
        Directory.CreateDirectory(Path.Combine(directory, "alpha", "inner"));
        File.WriteAllText(Path.Combine(directory, "hero.psd"), "x");

        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);

        // The providers the plugin brings itself, which is the state a run starts in; what any other test
        // registered was cleared before this one began.
        global::FileSystemPlugin.HideRegistry.Declare(host);

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
        var dock = new global::FileSystemPlugin.TreeDock(
            host,
            browser,
            new global::FileSystemPlugin.Clip(host.Log)
        );

        var window = new Window
        {
            Content = dock.View,
            Width = 900,
            Height = 600,
        };

        window.Show();
        window.Activate();

        return (window, Until(window, () => Root(window)));
    }

    /// <summary>
    /// The tree as it is drawn now: the step it is rooted at, and the step under that which the fixture holds.
    /// </summary>
    /// <remarks>
    /// Asked after every press rather than kept from before it: going to the place the location is already in
    /// reads the directory again, and the tree that is drawn after it is another one with other rows — a row kept
    /// from before it is a row that is no longer drawn anywhere.
    /// </remarks>
    /// <param name="window">The window the tree is in.</param>
    /// <returns>The root step, and the step named <c>alpha</c> under it.</returns>
    private static (TreeViewItem Root, TreeViewItem Alpha) Drawn(Window window)
    {
        var root = Until(window, () => Root(window));

        return (root, Until(window, () => Step(root, "alpha")));
    }

    /// <summary>
    /// The step the tree in a window is rooted at, or nothing while the tree is not drawn yet.
    /// </summary>
    /// <param name="window">The window the tree is in.</param>
    /// <returns>The root step, or nothing.</returns>
    private static TreeViewItem? Root(Window window) =>
        window.GetVisualDescendants()
            .OfType<TreeView>()
            .SelectMany(tree => tree.Items.OfType<TreeViewItem>())
            .FirstOrDefault(step => step.Bounds.Height > 0);

    /// <summary>The step a row stands under, by the name of the directory it is.</summary>
    /// <param name="under">The step to look under.</param>
    /// <param name="name">The name of the directory.</param>
    /// <returns>The row, or nothing while it is not drawn yet.</returns>
    private static TreeViewItem? Step(TreeViewItem under, string name) =>
        under.Items
            .OfType<TreeViewItem>()
            .FirstOrDefault(step =>
                step.Tag is string path
                && string.Equals(Path.GetFileName(path), name, StringComparison.Ordinal)
            );

    /// <summary>The chevron of a step, which is what opens and closes it.</summary>
    /// <remarks>
    /// The row's only toggle button: the theme draws the chevron as one and nothing else in a row of this tree
    /// is one, so the name the theme gives it is not repeated here.
    /// </remarks>
    /// <param name="step">The step.</param>
    /// <returns>Its chevron.</returns>
    /// <exception cref="InvalidOperationException">The step draws no chevron.</exception>
    private static ToggleButton Chevron(TreeViewItem step) =>
        step.GetVisualDescendants().OfType<ToggleButton>().FirstOrDefault()
        ?? throw new InvalidOperationException($"the step `{step.Tag}` draws no chevron");

    /// <summary>The band a step's name is drawn in, which is what a press on the row lands on.</summary>
    /// <param name="step">The step.</param>
    /// <returns>The band.</returns>
    /// <exception cref="InvalidOperationException">The step's header is not a row of this tree.</exception>
    private static Border Row(TreeViewItem step) =>
        step.Header as Border
        ?? throw new InvalidOperationException($"the step `{step.Tag}` is not drawn as a row");

    /// <summary>Presses the left button once in the middle of a control, and lets it go there.</summary>
    /// <remarks>
    /// Pressed and released in one place, because a press that moves is a drag of the step rather than a click on
    /// it, and what is checked here is the click.
    /// </remarks>
    /// <param name="window">The window the control is drawn in.</param>
    /// <param name="at">The control to press.</param>
    private static void Press(Window window, Visual at)
    {
        var middle =
            at.TranslatePoint(new Point(at.Bounds.Width / 2, at.Bounds.Height / 2), window)
            ?? throw new InvalidOperationException(
                $"the control `{at.GetType().Name}` is not drawn in the window"
            );

        window.MouseDown(middle, MouseButton.Left);
        window.MouseUp(middle, MouseButton.Left);
    }

    /// <summary>Waits for something to be drawn, which happens a turn or so after it is asked for.</summary>
    /// <param name="window">The window to draw.</param>
    /// <param name="find">What is being waited for, which answers nothing until it is there.</param>
    /// <returns>What was waited for.</returns>
    /// <exception cref="InvalidOperationException">It was never drawn.</exception>
    private static T Until<T>(Window window, Func<T?> find)
        where T : class
    {
        for (var turn = 0; turn < 200; turn++)
        {
            if (find() is { } found)
            {
                return found;
            }

            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
            Dispatcher.UIThread.RunJobs();
            Thread.Sleep(10);
        }

        throw new InvalidOperationException("what was asked for was never drawn");
    }
}
