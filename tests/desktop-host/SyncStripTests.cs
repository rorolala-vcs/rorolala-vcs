using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Media;
using Avalonia.Styling;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;
using RorolalaDesktop.Theming;
using RorolalaVCSPlugin;
using Vcs = RorolalaVCSPlugin.RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The sync strip and the panel it draws beside: what a reader sees of the two.
/// </summary>
/// <remarks>
/// What the strip does about a run is the command line's and is not run here, so what is checked is what it
/// draws: the buttons a reader presses, the colour they turn to while the key that changes a run is held, and
/// what the panel makes of a run that has reported itself. The two are drawn inside a window with the look over
/// them, because a mark's colour is the look's to answer and a tree that never reached one would answer nothing.
/// <para>
/// What a box leaves out is not checked here: what is carried is kept in the preference file, which a confined
/// run cannot write (the same handful of failures every suite here has), and what a scope comes to in flags and
/// arguments is checked without a file in <see cref="SyncTests"/>.
/// </para>
/// </remarks>
public sealed class SyncStripTests
{
    /// <summary>
    /// The switch for the ignored is drawn first, then the reading, then the three as one group, and the
    /// dropdown last.
    /// </summary>
    /// <remarks>
    /// The group is what the reader asked for: three buttons that belong together are drawn inside one rule, so
    /// that what they are about is one thing rather than three, and the button that opens what a run leaves out
    /// stands outside it, since what it is about is the run and not the work. The switch stands before the reading
    /// because it is about what the reading shows rather than about the work, and a reader reaches for it less
    /// often than for the reading beside it.
    /// </remarks>
    [Fact]
    public void TheIgnoredSwitchStandsBeforeTheReadingAndTheThreeAsOneGroup()
    {
        var (strip, _, _) = Drawn();

        Assert.Equal(4, strip.Children.Count);

        // The switch that shows what the ignore rules govern, then the reading.
        Assert.IsType<Button>(strip.Children[0]);
        Assert.IsType<Button>(strip.Children[1]);

        var group = Assert.IsType<Border>(strip.Children[2]);
        var inside = Assert.IsType<StackPanel>(group.Child);

        Assert.Equal(3, inside.Children.Count);
        Assert.All(inside.Children, child => Assert.IsType<Button>(child));

        // The dropdown is the last button of the row, and it is the one with something to open.
        var more = Assert.IsType<Button>(strip.Children[3]);

        var flyout = Assert.IsType<Flyout>(more.Flyout);
        var boxes = Assert.IsType<StackPanel>(flyout.Content);

        // One box per part of a run, all of them on until the user says otherwise.
        Assert.Equal(3, boxes.Children.Count);
        Assert.All(
            boxes.Children,
            child =>
            {
                var box = Assert.IsType<CheckBox>(child);

                Assert.True(box.IsChecked);
            }
        );

        // The three marks are the three pictures this plugin contributes for them, and the last is the one that
        // opens something rather than running anything.
        Assert.Equal(3, Assert.IsType<StackPanel>(group.Child).Children.Count);

        // Nothing has been asked of the Vault yet, so there is nothing known to carry: the three are drawn
        // unpressable rather than offering a run nobody has said there is work for.
        Assert.All(inside.Children, child => Assert.False(Assert.IsType<Button>(child).IsEnabled));

        // The switch, the reading and the dropdown are pressable whatever the three are: they are about the work
        // and about what is shown of it, neither of which waits on there being something to carry.
        Assert.True(Assert.IsType<Button>(strip.Children[0]).IsEnabled);
        Assert.True(Assert.IsType<Button>(strip.Children[1]).IsEnabled);
        Assert.True(more.IsEnabled);
    }

    /// <summary>
    /// The switch for the ignored says which way it stands, and pressing it turns the eye and the words over.
    /// </summary>
    /// <remarks>
    /// What it shows is the Overview's answer rather than the switch's: the strip reads it each time it draws,
    /// so a state changed anywhere is drawn from that state. The pictures are the plugin's own pair — the eye
    /// and the eye struck through — and what is checked is that pressing it asks and that the drawing follows
    /// the answer, not the colours the look gives them.
    /// </remarks>
    [Fact]
    public void TheIgnoredSwitchSaysWhichWayItStands()
    {
        var shown = false;
        var (strip, _, _) = Drawn(ignoredShown: () => shown, toggleIgnored: () => shown = !shown);

        var toggle = Assert.IsType<Button>(strip.Children[0]);

        Assert.Equal(RolaI18N.Get("rorolala_vcs.status.show_ignored"), ToolTip.GetTip(toggle));

        // Pressing it asks the Overview to change, and the strip draws the answer it is given.
        toggle.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));

        Assert.True(shown);
        Assert.Equal(RolaI18N.Get("rorolala_vcs.status.hide_ignored"), ToolTip.GetTip(toggle));
    }

    /// <summary>
    /// The three are drawn in the ordinary ink, and in the colour of what must not be done while Shift is held.
    /// </summary>
    /// <remarks>
    /// A run made with the key held changes history rather than refusing, which is the one thing a reader has to
    /// see before pressing rather than after: the strip says so by turning the three and the rule around them
    /// the colour the look spends on what must not be done. What is checked is the colour actually drawn on
    /// screen, which is the look's answer and not this test's.
    /// </remarks>
    [Fact]
    public void TheThreeTurnRedWhileTheKeyThatChangesARunIsHeld()
    {
        var (strip, window, resources) = Drawn();

        var plain = Fill(resources, PaletteKeys.Front);
        var rule = Fill(resources, PaletteKeys.Splitter);
        var held = Fill(resources, PaletteKeys.Del);

        Assert.NotEqual(plain, held);

        var around = Assert.IsType<Border>(strip.Children[2]);

        foreach (var mark in Marks(strip))
        {
            Assert.Equal(plain, Colour(mark.Background));
        }

        Assert.Equal(rule, Colour(around.BorderBrush));

        // The key being held is what the strip watches for, wherever the pointer is: it is told by the window
        // rather than by one of its own buttons having the keyboard.
        window.RaiseEvent(
            new KeyEventArgs
            {
                RoutedEvent = InputElement.KeyDownEvent,
                Key = Key.LeftShift,
                KeyModifiers = KeyModifiers.Shift,
            }
        );

        foreach (var mark in Marks(strip))
        {
            Assert.Equal(held, Colour(mark.Background));
        }

        Assert.Equal(held, Colour(around.BorderBrush));

        // And letting it go draws them as they were, since the next run is an ordinary one again.
        window.RaiseEvent(
            new KeyEventArgs
            {
                RoutedEvent = InputElement.KeyUpEvent,
                Key = Key.LeftShift,
                KeyModifiers = KeyModifiers.None,
            }
        );

        foreach (var mark in Marks(strip))
        {
            Assert.Equal(plain, Colour(mark.Background));
        }

        Assert.Equal(rule, Colour(around.BorderBrush));
    }

    /// <summary>A capsule is drawn for every command, and the whole bar adds up its own phases.</summary>
    /// <remarks>
    /// What is drawn is what the commands said of themselves: one capsule per command, the command's own phases
    /// added up as the bar at its head, and a row under it for every phase it reports — the rows are the capsule's
    /// own detail, shown while the pointer is on it.
    /// </remarks>
    [Fact]
    public void TheStackDrawsACapsuleForEveryRunAndAddsUpItsOwnPhases()
    {
        var hub = new ProgressHub();
        var services = Host.Services();
        var panel = new ProgressPanel(hub, services.Icons);

        Assert.False(panel.IsVisible);

        var run = hub.Begin("rorolala_vcs.progress.sync");

        Assert.True(
            run.Records.Read(
                """{"signal":"begin","id":1,"parent":null,"what":"","direction":null,"total":4}"""
            )
        );
        Assert.True(
            run.Records.Read(
                """{"signal":"begin","id":2,"parent":1,"what":"","direction":"up","total":10}"""
            )
        );
        Assert.True(run.Records.Read("""{"signal":"advance","id":2,"done":3}"""));

        panel.Draw();

        Assert.True(panel.IsVisible);

        var capsule = Assert.Single(Capsules(panel));

        // The whole bar adds up the run's own tasks rather than the direction under one of them, which is a row
        // in the capsule's detail instead of a second bar beside the command's own.
        Assert.Equal(4, Whole(capsule).Maximum);
        Assert.Equal(0, Whole(capsule).Value);

        // A second phase of the same command is what the bar follows, rather than the two phases' lengths being
        // added together: a plan of steps and a transfer of blocks are not counted in the same thing.
        Assert.True(
            run.Records.Read(
                """{"signal":"begin","id":4,"parent":null,"what":"","direction":null,"total":8}"""
            )
        );
        panel.Draw();

        Assert.Equal(8, Whole(capsule).Maximum);
        Assert.Equal(0, Whole(capsule).Value);

        var claim = hub.Begin("rorolala_vcs.progress.hold");
        Assert.True(
            claim.Records.Read(
                """{"signal":"begin","id":9,"parent":null,"what":"","direction":null,"total":2}"""
            )
        );

        panel.Draw();

        Assert.Equal(2, Capsules(panel).Count);
    }

    /// <summary>The capsules a panel has drawn, in the order the commands began.</summary>
    /// <param name="panel">The panel.</param>
    /// <returns>The capsules.</returns>
    private static IReadOnlyList<Border> Capsules(ProgressPanel panel) =>
        Assert.IsType<StackPanel>(panel.Children[0]).Children.OfType<Border>().ToArray();

    /// <summary>The bar for the whole of a command, at the head of its capsule.</summary>
    /// <param name="capsule">The capsule.</param>
    /// <returns>The bar.</returns>
    private static ProgressBar Whole(Border capsule) =>
        Assert.IsType<StackPanel>(capsule.Child).Children.OfType<ProgressBar>().Single();

    /// <summary>The plugin's identity, which is what its settings are kept under.</summary>
    private const string Identity = "rorolala.vcs";

    /// <summary>The strip, drawn inside a window with the look over it, as the program draws it.</summary>
    /// <param name="ignoredShown">Whether what the ignore rules govern is being shown, or nothing for not.</param>
    /// <param name="toggleIgnored">What the switch asks for, or nothing for asking nothing.</param>
    /// <returns>The strip, its window, and the look's resources.</returns>
    private static (SyncStrip Strip, Window Window, ResourceDictionary Resources) Drawn(
        Func<bool>? ignoredShown = null,
        Action? toggleIgnored = null
    )
    {
        var services = Host.Services();
        var host = services.For(new PluginId(Identity), 0);

        new Vcs().Initialize(host);

        var look = new RorolalaTheme(
            ThemePalette.DefaultDark,
            Color.Parse("#223300"),
            WindowStyle.Windows
        );

        var resources = Assert.IsAssignableFrom<ResourceDictionary>(
            look.Palette.ThemeDictionaries[ThemeVariant.Dark]
        );

        var hub = new ProgressHub();
        var strip = new SyncStrip(
            host,
            new SyncWatcher(host, host.Log, hub, new VaultFetch(host, hub)),
            () => null,
            refresh: () => { },
            ignoredShown: ignoredShown ?? (() => false),
            toggleIgnored: toggleIgnored ?? (() => { })
        );

        var window = new Window
        {
            // The program is drawn on the dark ground, and a mark's colour is read for the variant the tree is
            // drawn in: a window left at the toolkit's default would be answered by the light look, and what is
            // checked here is the ink the program actually draws with.
            Resources = resources,
            RequestedThemeVariant = ThemeVariant.Dark,
            Content = strip,
        };

        window.Show();

        return (strip, window, resources);
    }

    /// <summary>The three marks of the group, in the order the buttons are read.</summary>
    /// <param name="strip">The strip.</param>
    /// <returns>The marks.</returns>
    private static IReadOnlyList<Border> Marks(SyncStrip strip)
    {
        var group = Assert.IsType<Border>(strip.Children[2]);
        var inside = Assert.IsType<StackPanel>(group.Child);

        return inside
            .Children.OfType<Button>()
            .Select(button => Assert.IsType<Border>(button.Content))
            .ToArray();
    }

    /// <summary>What one of the look's own names is drawn in.</summary>
    /// <param name="resources">The look's resources.</param>
    /// <param name="key">The name.</param>
    /// <returns>The colour.</returns>
    private static Color Fill(ResourceDictionary resources, string key) =>
        Colour(resources[key]);

    /// <summary>The colour of a brush, which is the only kind the look publishes.</summary>
    /// <param name="brush">The brush, or the resource itself.</param>
    /// <returns>The colour.</returns>
    private static Color Colour(object? brush) =>
        Assert.IsType<SolidColorBrush>(brush).Color;
}
