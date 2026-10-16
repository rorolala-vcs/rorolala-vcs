using Avalonia.Controls;
using Avalonia.Input;
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
    /// <summary>The three sync buttons are one group beside the reading, and the dropdown is the last of them.</summary>
    /// <remarks>
    /// The group is what the reader asked for: three buttons that belong together are drawn inside one rule, so
    /// that what they are about is one thing rather than three, and the button that opens what a run leaves out
    /// stands outside it, since what it is about is the run and not the work.
    /// </remarks>
    [Fact]
    public void TheThreeAreDrawnAsOneGroupWithTheReadingBesideThem()
    {
        var (strip, _, _) = Drawn();

        Assert.Equal(3, strip.Children.Count);
        Assert.IsType<Button>(strip.Children[0]);

        var group = Assert.IsType<Border>(strip.Children[1]);
        var inside = Assert.IsType<StackPanel>(group.Child);

        Assert.Equal(3, inside.Children.Count);
        Assert.All(inside.Children, child => Assert.IsType<Button>(child));

        // The dropdown is the last button of the row, and it is the one with something to open.
        var more = Assert.IsType<Button>(strip.Children[2]);

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

        // The reading and the dropdown are pressable whatever the three are: they are about the work and about
        // what a run is told, neither of which waits on there being something to carry.
        Assert.True(Assert.IsType<Button>(strip.Children[0]).IsEnabled);
        Assert.True(more.IsEnabled);
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

        var around = Assert.IsType<Border>(strip.Children[1]);

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

    /// <summary>A card is drawn for every command a run reports, and a bar for every task of it.</summary>
    /// <remarks>
    /// What is drawn is what the commands said of themselves: one card per command, the command's own tasks added
    /// up as the bar at the head of it, and a bar under it for each task that is part of another. A task whose
    /// length was never said is drawn as a bar that moves rather than as one at nothing done.
    /// </remarks>
    [Fact]
    public void ThePanelDrawsACardForEveryRunAndABarForEveryTask()
    {
        var services = Host.Services();
        var host = services.For(new PluginId(Identity), 0);

        new Vcs().Initialize(host);

        var hub = new ProgressHub();
        var panel = new ProgressPanel(hub);
        Assert.False(panel.IsVisible);

        var run = hub.Begin("rorolala_vcs.progress.sync");

        // A bar of the run's own, a direction under it, and a piece the direction is moving: the run and the
        // direction are bars of their own, and the piece is what the direction says it is moving.
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
        Assert.True(
            run.Records.Read(
                """{"signal":"begin","id":3,"parent":2,"what":"a/file.psd","direction":null,"total":null}"""
            )
        );
        Assert.True(run.Records.Read("""{"signal":"advance","id":2,"done":3}"""));

        panel.Draw();

        Assert.True(panel.IsVisible);

        var card = Assert.Single(Cards(panel));
        var rows = Rows(card);

        // Two bars, not three: the piece is not a bar of its own, it is what the direction is moving.
        Assert.Equal(2, rows.Count);

        Assert.Equal(RolaI18N.Get("rorolala_vcs.progress.sync"), Name(rows[0]));
        Assert.Equal("0%", Share(rows[0]));
        Assert.Equal(4, Bar(rows[0]).Maximum);
        Assert.Equal(0, Bar(rows[0]).Value);

        Assert.Equal("↑ " + RolaI18N.Get("rorolala_vcs.progress.row_up"), Name(rows[1]));
        Assert.Equal("30%", Share(rows[1]));
        Assert.Equal(10, Bar(rows[1]).Maximum);
        Assert.Equal(3, Bar(rows[1]).Value);
        Assert.Equal("a/file.psd", Moving(rows[1]));

        // A task that has not said how long it is moves rather than sits at nothing done, and one the command has
        // not finished makes the whole of the command move too: a sum of lengths one of which is unknown.
        Assert.True(
            run.Records.Read(
                """{"signal":"begin","id":4,"parent":null,"what":"plan","direction":null,"total":null}"""
            )
        );
        panel.Draw();

        Assert.True(Bar(Rows(card)[0]).IsIndeterminate);

        // A second command is a card of its own, in the order the hub began it.
        var claim = hub.Begin("rorolala_vcs.progress.hold");
        Assert.True(
            claim.Records.Read(
                """{"signal":"begin","id":9,"parent":null,"what":"","direction":null,"total":2}"""
            )
        );
        panel.Draw();

        Assert.Equal(2, Cards(panel).Count);

        // A task that is over is kept and marked full, so a card still has something to show while it lingers.
        Assert.True(run.Records.Read("""{"signal":"finish","id":2}"""));
        panel.Draw();

        Assert.Equal(10, Bar(Rows(card)[1]).Value);
        Assert.Equal("100%", Share(rows[1]));
    }

    /// <summary>The plugin's identity, which is what its settings are kept under.</summary>
    private const string Identity = "rorolala.vcs";

    /// <summary>The strip, drawn inside a window with the look over it, as the program draws it.</summary>
    /// <returns>The strip, its window, and the look's resources.</returns>
    private static (SyncStrip Strip, Window Window, ResourceDictionary Resources) Drawn()
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

        var strip = new SyncStrip(
            host,
            new SyncWatcher(host, host.Log, new ProgressHub()),
            () => null,
            refresh: () => { }
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
        var group = Assert.IsType<Border>(strip.Children[1]);
        var inside = Assert.IsType<StackPanel>(group.Child);

        return inside
            .Children.OfType<Button>()
            .Select(button => Assert.IsType<Border>(button.Content))
            .ToArray();
    }

    /// <summary>The cards a panel has drawn, in the order the commands began.</summary>
    /// <param name="panel">The panel.</param>
    /// <returns>The cards.</returns>
    private static IReadOnlyList<Border> Cards(ProgressPanel panel)
    {
        var scroll = Assert.IsType<ScrollViewer>(panel.Child);
        var cards = Assert.IsType<StackPanel>(scroll.Content);

        return cards.Children.OfType<Border>().ToArray();
    }

    /// <summary>The bars of a card, the whole of the command first and its tasks under it.</summary>
    /// <param name="card">The card.</param>
    /// <returns>The bars.</returns>
    private static IReadOnlyList<Grid> Rows(Border card)
    {
        var rows = Assert.IsType<StackPanel>(card.Child);

        return rows.Children.OfType<Grid>().ToArray();
    }

    /// <summary>What a bar is captioned with.</summary>
    /// <param name="row">The bar's line.</param>
    /// <returns>The words at the start of the line.</returns>
    private static string Name(Grid row) =>
        Assert.IsType<TextBlock>(row.Children[0]).Text ?? string.Empty;

    /// <summary>What a bar says of how far its task has got.</summary>
    /// <param name="row">The bar's line.</param>
    /// <returns>The words at the end of the line.</returns>
    private static string Share(Grid row) =>
        Assert.IsType<TextBlock>(row.Children[2]).Text ?? string.Empty;

    /// <summary>The bar of a bar's line, which is drawn inside it with what is being moved.</summary>
    /// <param name="row">The bar's line.</param>
    /// <returns>The bar.</returns>
    private static ProgressBar Bar(Grid row) =>
        Assert.IsType<Grid>(row.Children[1]).Children.OfType<ProgressBar>().Single();

    /// <summary>What a bar says it is moving, which is drawn inside the bar rather than beside it.</summary>
    /// <param name="row">The bar's line.</param>
    /// <returns>What is being moved.</returns>
    private static string? Moving(Grid row) =>
        Assert.IsType<Grid>(row.Children[1]).Children.OfType<TextBlock>().Single().Text;

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
