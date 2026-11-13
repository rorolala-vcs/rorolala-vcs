using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Headless;
using Avalonia.Input;
using Avalonia.Threading;
using Avalonia.VisualTree;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
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
///, the chevron and the row are separate controls in the row's template, and which of them a press reaches is
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
    /// tree with no chevron and nothing to open, which is a test of nothing at all, and it reads from the
    /// outside exactly like a press that did not work.
    /// </remarks>
    public TreeTests()
    {
        DataHome.Clean();
        global::FileSystemPlugin.HideRegistry.Clear();
        global::FileSystemPlugin.ChosenEntries.Clear();
        global::FileSystemPlugin.ChosenFiles.Clear();
    }

    /// <summary>A control press chooses one row, adds another with Ctrl, and takes it away again.</summary>
    [Fact]
    public void ACtrlClickAddsAndRemovesARow()
    {
        var directory = Scratch.New("tree-choice");
        var alpha = Directory.CreateDirectory(Path.Combine(directory, "alpha")).FullName;
        var beta = Directory.CreateDirectory(Path.Combine(directory, "beta")).FullName;

        var (window, browser) = OpenAt(directory);

        try
        {
            var root = Until(window, () => Root(window));

            root.IsExpanded = true;
            PumpFrames(window, 20);

            Press(window, Row(Until(window, () => RowByPath(window, alpha))));
            Assert.Equal([alpha], ChosenEntries().Select(entry => entry.Path));

            Press(
                window,
                Row(Until(window, () => RowByPath(window, beta))),
                RawInputModifiers.Control
            );
            Assert.Equal([alpha, beta], ChosenEntries().Select(entry => entry.Path));

            Press(
                window,
                Row(Until(window, () => RowByPath(window, beta))),
                RawInputModifiers.Control
            );
            Assert.Equal([alpha], ChosenEntries().Select(entry => entry.Path));
            Assert.Equal(alpha, browser.Current);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A Shift press takes the range of rows that are on screen.</summary>
    [Fact]
    public void AShiftClickSelectsTheVisibleRange()
    {
        var directory = Scratch.New("tree-range");
        var alpha = Directory.CreateDirectory(Path.Combine(directory, "alpha")).FullName;
        var beta = Directory.CreateDirectory(Path.Combine(directory, "beta")).FullName;
        var gamma = Directory.CreateDirectory(Path.Combine(directory, "gamma")).FullName;

        var (window, _) = OpenAt(directory);

        try
        {
            var root = Until(window, () => Root(window));

            root.IsExpanded = true;
            PumpFrames(window, 20);

            Press(window, Row(Until(window, () => RowByPath(window, alpha))));
            Press(
                window,
                Row(Until(window, () => RowByPath(window, gamma))),
                RawInputModifiers.Shift
            );

            Assert.Equal([alpha, beta, gamma], ChosenEntries().Select(entry => entry.Path));
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>Ctrl+A chooses every row the tree shows, including the base row.</summary>
    [Fact]
    public void CtrlASelectsEveryVisibleRow()
    {
        var directory = Scratch.New("tree-select-all");

        Directory.CreateDirectory(Path.Combine(directory, "alpha"));
        File.WriteAllText(Path.Combine(directory, "hero.psd"), "x");

        var (window, _) = OpenAt(directory);

        try
        {
            var root = Until(window, () => Root(window));

            root.IsExpanded = true;
            PumpFrames(window, 20);

            var tree = window.GetVisualDescendants().OfType<TreeView>().First();

            Assert.True(tree.Focus(), "the tree would not take the keyboard");
            PumpFrames(window, 2);

            window.KeyPress(Key.A, RawInputModifiers.Control, PhysicalKey.A, "a");
            PumpFrames(window, 5);

            Assert.Equal(3, ChosenEntries().Count);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>The choice is a list, which is what the rest of the program reads.</summary>
    /// <returns>The current choice.</returns>
    private static IReadOnlyList<Entry> ChosenEntries() =>
        global::FileSystemPlugin.ChosenEntries.Current;

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
    /// the tree as it is drawn after the press rather than the row that was pressed, the two are not always the
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

    /// <summary>F2 puts the chosen step's name in a box, and Escape leaves the step as it was.</summary>
    /// <remarks>
    /// The tree answers the same keys a listing does. What is checked is the box the tree draws in the row, which
    /// is the one thing a tree has to do differently: a listing's row can be replaced, while a tree's row is the
    /// step it stands for.
    /// </remarks>
    [Fact]
    public void F2PutsTheChosenStepsNameIntoABoxAndEscapeLeavesIt()
    {
        var directory = Scratch.New("tree-keys");
        Directory.CreateDirectory(Path.Combine(directory, "alpha"));

        var (window, _) = OpenAt(directory);

        try
        {
            var root = Until(window, () => Root(window));

            Press(window, Chevron(root));

            var alpha = Until(window, () => Step(root, "alpha"));

            Press(window, Row(alpha));
            PumpFrames(window, 20);

            window.KeyPress(Key.F2, RawInputModifiers.None, PhysicalKey.F2, null);

            var box = Until(
                window,
                () => window.GetVisualDescendants().OfType<TextBox>().FirstOrDefault()
            );

            Assert.Equal("alpha", box.Text);

            // Ctrl+A in the box is the box's, not the tree's: the tree must not choose every row under the
            // box that is covering one of them.
            window.KeyPress(Key.A, RawInputModifiers.Control, PhysicalKey.A, "a");
            PumpFrames(window, 5);

            Assert.Equal(
                [Path.Combine(directory, "alpha")],
                ChosenEntries().Select(entry => entry.Path)
            );

            window.KeyPress(Key.Escape, RawInputModifiers.None, PhysicalKey.Escape, null);
            PumpFrames(window, 20);

            Assert.DoesNotContain(
                window.GetVisualDescendants().OfType<TextBox>(),
                text => text.Text == "alpha"
            );
            Assert.True(Directory.Exists(Path.Combine(directory, "alpha")));
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>Ctrl+Shift+N makes a folder in the step that is chosen.</summary>
    [Fact]
    public void CtrlShiftNMakesAFolderInTheChosenStep()
    {
        var directory = Scratch.New("tree-new");
        var alpha = Path.Combine(directory, "alpha");

        Directory.CreateDirectory(alpha);

        var (window, _) = OpenAt(directory);

        try
        {
            var root = Until(window, () => Root(window));

            Press(window, Chevron(root));

            var step = Until(window, () => Step(root, "alpha"));

            Press(window, Row(step));
            PumpFrames(window, 20);

            var before = Directory.GetDirectories(alpha).Length;

            window.KeyPress(
                Key.N,
                RawInputModifiers.Control | RawInputModifiers.Shift,
                PhysicalKey.N,
                null
            );

            Pump(window, () => Directory.GetDirectories(alpha).Length > before);

            Assert.True(Directory.GetDirectories(alpha).Length > before);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>Shift+Delete removes the step that is chosen.</summary>
    [Fact]
    public void ShiftDeleteRemovesTheChosenStep()
    {
        var directory = Scratch.New("tree-delete");
        var alpha = Path.Combine(directory, "alpha");

        Directory.CreateDirectory(alpha);

        var (window, _) = OpenAt(directory);

        try
        {
            var root = Until(window, () => Root(window));

            Press(window, Chevron(root));

            var step = Until(window, () => Step(root, "alpha"));

            Press(window, Row(step));
            PumpFrames(window, 20);

            Assert.Equal([alpha], ChosenEntries().Select(entry => entry.Path));

            window.KeyPress(
                Key.Delete,
                RawInputModifiers.Shift,
                PhysicalKey.Delete,
                null
            );

            Pump(window, () => !Directory.Exists(alpha));

            Assert.False(Directory.Exists(alpha), "the chosen step was not removed");
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>
    /// A tree told that the files changed keeps the scroll it was read at, and the step it had chosen.
    /// </summary>
    /// <remarks>
    /// What a file operation says is that the tree's rows are to be read again, which is a drawing of the same
    /// view rather than a replacement of it: a tree built again comes back with its scroll at the top, and a
    /// reader scrolled into a long tree sees the step they had chosen page away from where they were reading it.
    /// What is checked is the view the reader is looking at rather than the reading itself: the scroll and the
    /// tree the scroll belongs to are the same before and after, and the chosen step is chosen in the rows that
    /// were read again.
    /// </remarks>
    [Fact]
    public void ARereadKeepsTheScrollAndTheChosenStep()
    {
        var directory = Scratch.New("tree-scroll");

        for (var index = 0; index < 240; index++)
        {
            File.WriteAllText(Path.Combine(directory, $"file-{index:D3}.txt"), "x");
        }

        var (window, browser) = OpenAt(directory);

        var root = Until(window, () => Root(window));

        root.IsExpanded = true;

        // The rows are made a turn or so after the step is opened, and a scroll of a tree with nothing drawn in
        // it is a scroll of nowhere. The turns are drawn rather than waited for, since what is being waited for
        // is a layout and not an answer: a reading of the rules behind the rows may be a turn or two behind.
        PumpFrames(window, 30);

        var scroll = Until(
            window,
            () =>
                window.GetVisualDescendants()
                    .OfType<TreeView>()
                    .SelectMany(tree => tree.GetVisualDescendants())
                    .OfType<ScrollViewer>()
                    .FirstOrDefault(bars => bars.Extent.Height > bars.Viewport.Height)
        );

        var chosen = Until(
            window,
            () => root.Items.OfType<TreeViewItem>().ElementAtOrDefault(5)
        );

        var path = Assert.IsType<string>(chosen.Tag);

        // The step a reader has chosen is one they are looking at, so it is chosen before the tree is scrolled:
        // what a reread has to keep is where the reader was reading, and a step chosen and then scrolled out of
        // sight is one the toolkit would bring back into view whatever the reread did.
        chosen.IsSelected = true;

        // What choosing a row leaves behind is a scroll asked for a turn later, so the scroll is read against a
        // tree that has done it: what this is about is the reread, not the selection that came before it.
        PumpFrames(window, 20);

        scroll.Offset = new Vector(0, 96);

        // What the scroll came to rather than what it was asked for: a tree whose extent is not the one the
        // offset was read against clamps it, and it is the clamped offset a reread has to keep.
        PumpFrames(window, 20);

        var offset = scroll.Offset;

        browser.Refresh();

        Pump(window, () => RowByPath(window, path)?.IsSelected == true);

        Assert.Same(
            window.GetVisualDescendants()
                .OfType<ScrollViewer>()
                .First(bars => bars.Extent.Height > bars.Viewport.Height),
            scroll
        );
        Assert.Equal(offset.Y, scroll.Offset.Y, 1);
        Assert.True(RowByPath(window, path)?.IsSelected, "the chosen step is not chosen in the rows read again");

        window.Close();
    }

    /// <summary>
    /// The word for an empty base is about what the base holds now, not about what it held when the tree was made.
    /// </summary>
    /// <remarks>
    /// A base that was empty is a base that can be given something while the tree is looking at it, a sync, a
    /// fetch, a file made by another program, and the word drawn over the tree has to go when it is. A tree read
    /// again is the same view, so a word decided once where the tree was made would be a word about a base that is
    /// no longer there: what is checked is the word after the base is given something, and again after the
    /// something is taken away.
    /// </remarks>
    [Fact]
    public void TheWordForAnEmptyBaseIsNotKeptWhenTheBaseIsGivenSomething()
    {
        var directory = Scratch.New("tree-empty");
        var (window, browser) = OpenAt(directory);

        var word = Assert.IsType<StackPanel>(Until(window, () => EmptyWord(window)));

        Assert.True(word.IsVisible, "a base holding nothing is not said to hold nothing");

        var root = Until(window, () => Root(window));

        Assert.Empty(root.Items);

        var file = Path.Combine(directory, "hero.psd");

        File.WriteAllText(file, "x");
        browser.Refresh();

        Assert.False(word.IsVisible, "the word for an empty base outlived the base being given something");
        Assert.NotEmpty(root.Items);

        File.Delete(file);
        browser.Refresh();

        Assert.True(word.IsVisible, "a base emptied again is not said to hold nothing");
        Assert.Empty(root.Items);

        window.Close();
    }

    /// <summary>The step a path is drawn as, or nothing while it is not drawn.</summary>
    /// <param name="window">The window the tree is in.</param>
    /// <param name="path">The directory the step stands for.</param>
    /// <returns>The step, or nothing.</returns>
    private static TreeViewItem? RowByPath(Window window, string path) =>
        window.GetVisualDescendants()
            .OfType<TreeViewItem>()
            .FirstOrDefault(step => string.Equals(step.Tag as string, path, StringComparison.Ordinal));

    /// <summary>
    /// The panel the word for an empty base is drawn in, or nothing while the tree has no such word.
    /// </summary>
    /// <param name="window">The window the tree is in.</param>
    /// <returns>The panel, whose visibility is whether the word is being said.</returns>
    private static StackPanel? EmptyWord(Window window) =>
        window.GetVisualDescendants()
            .OfType<StackPanel>()
            .FirstOrDefault(panel =>
                panel.Children.OfType<TextBlock>().Any(text =>
                    string.Equals(
                        text.Text,
                        RolaI18N.Get("rorolala_file_system.empty"),
                        StringComparison.Ordinal
                    )
                )
            );

    /// <summary>Waits for something to be drawn, which happens a turn or so after it is asked for.</summary>
    /// <param name="window">The window to draw.</param>
    /// <param name="until">What is being waited for, which answers false until it is there.</param>
    private static void Pump(Window window, Func<bool> until)
    {
        for (var turn = 0; turn < 200; turn++)
        {
            if (until())
            {
                return;
            }

            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
            Dispatcher.UIThread.RunJobs();
            Thread.Sleep(10);
        }

        throw new InvalidOperationException("what was asked for was never drawn");
    }

    /// <summary>Draws the window for a number of turns, whether or not anything is being waited for.</summary>
    /// <param name="window">The window to draw.</param>
    /// <param name="turns">How many turns to draw it for.</param>
    private static void PumpFrames(Window window, int turns)
    {
        for (var turn = 0; turn < turns; turn++)
        {
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
            Dispatcher.UIThread.RunJobs();
            Thread.Sleep(10);
        }
    }

    /// <summary>A tree over a scratch directory holding one directory, one directory under that, and one file.</summary>
    /// <returns>The window it is shown in, and the step the tree is rooted at.</returns>
    private static (Window Window, TreeViewItem Root) Open()
    {
        var directory = Scratch.New("tree");
        Directory.CreateDirectory(Path.Combine(directory, "alpha", "inner"));
        File.WriteAllText(Path.Combine(directory, "hero.psd"), "x");

        var (window, _) = OpenAt(directory);

        return (window, Until(window, () => Root(window)));
    }

    /// <summary>The program file operations are run with, which the suites are told the directory of.</summary>
    /// <returns>The path of `rola`.</returns>
    private static string Program() =>
        Path.Combine(
            Environment.GetEnvironmentVariable("ROLA_BIN_DIR")
                ?? throw new InvalidOperationException("the suites are run with ROLA_BIN_DIR"),
            OperatingSystem.IsWindows() ? "rola.exe" : "rola"
        );

    /// <summary>A tree dock over a directory, shown in a window, and the location it reads.</summary>
    /// <remarks>
    /// The providers the plugin brings itself are the state a run starts in; what any other test registered was
    /// cleared before this one began.
    /// </remarks>
    /// <param name="directory">The directory the tree is rooted at.</param>
    /// <returns>The window the tree is shown in, and the location it reads.</returns>
    private static (Window Window, global::FileSystemPlugin.Browser Browser) OpenAt(string directory)
    {
        Environment.SetEnvironmentVariable("ROLA_EXE", Program());

        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);

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

        return (window, browser);
    }

    /// <summary>
    /// A press on a step of a multiple choice keeps the choice until the press becomes a click.
    /// </summary>
    /// <remarks>
    /// The toolkit collapses the selection to the row under the pointer as the button goes down, and a drag of a
    /// chosen group would then carry one row. What is checked is the choice while the button is down, which is
    /// what a drag reads, and the choice after the release, which is what a click means.
    /// </remarks>
    [Fact]
    public void APressOnAChosenStepKeepsTheChoiceItLandedIn()
    {
        var directory = Scratch.New("tree-drag-choice");

        for (var index = 0; index < 3; index++)
        {
            File.WriteAllText(Path.Combine(directory, $"file-{index}.txt"), "x");
        }

        var (window, _) = OpenAt(directory);
        var root = Until(window, () => Root(window));

        root.IsExpanded = true;
        PumpFrames(window, 30);

        var rows = Until(
            window,
            () =>
            {
                var found = root.Items.OfType<TreeViewItem>().ToArray();

                return found.Length >= 3 ? found : null;
            }
        );

        var tree = window
            .GetVisualDescendants()
            .OfType<global::FileSystemPlugin.TreeBrowser>()
            .Single();

        rows[0].IsSelected = true;
        rows[1].IsSelected = true;
        PumpFrames(window, 5);

        Assert.Equal(2, tree.Chosen().Count);

        var row = Row(rows[1]);
        var at =
            row.TranslatePoint(new Point(row.Bounds.Width / 2, row.Bounds.Height / 2), window)
            ?? throw new InvalidOperationException("the row is not drawn in the window");

        window.MouseDown(at, MouseButton.Left);

        Assert.Equal(2, tree.Chosen().Count);

        window.MouseUp(at, MouseButton.Left);

        Assert.Single(tree.Chosen());
    }

    /// <summary>
    /// The tree as it is drawn now: the step it is rooted at, and the step under that which the fixture holds.
    /// </summary>
    /// <remarks>
    /// Asked after every press rather than kept from before it: going to the place the location is already in
    /// reads the directory again, and the tree that is drawn after it is another one with other rows, a row kept
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
    private static void Press(Window window, Visual at, RawInputModifiers modifiers = RawInputModifiers.None)
    {
        var middle =
            at.TranslatePoint(new Point(at.Bounds.Width / 2, at.Bounds.Height / 2), window)
            ?? throw new InvalidOperationException(
                $"the control `{at.GetType().Name}` is not drawn in the window"
            );

        window.MouseDown(middle, MouseButton.Left, modifiers);
        window.MouseUp(middle, MouseButton.Left, modifiers);
    }

    /// <summary>Waits for something to be drawn, which happens a turn or so after it is asked for.</summary>
    /// <param name="window">The window to draw.</param>
    /// <param name="find">What is being waited for, which answers nothing until it is there.</param>
    /// <returns>What was waited for.</returns>
    /// <exception cref="InvalidOperationException">It was never drawn.</exception>
    private static T Until<T>(Window window, Func<T?> find)
        where T : class
    {
        for (var turn = 0; turn < 400; turn++)
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
