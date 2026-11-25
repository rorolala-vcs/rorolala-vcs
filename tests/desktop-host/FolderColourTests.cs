using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaVCSPlugin;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What a marked folder does to the directory dock and to the tree: the icon of an entry that is the colour's
/// own, and the wash a tree row is read on.
/// </summary>
/// <remarks>
/// The colour is a rule a plugin registers, and the File System plugin is what lays it out. What is checked
/// here is the layering the two agree on: an entry inherits the nearest marked directory, a marked entry wears
/// its colour on its own icon, and a row under it is washed more lightly than the row the colour belongs to.
/// </remarks>
public sealed class FolderColourTests
{
    /// <summary>Begins each test from no provider and no configuration of its own.</summary>
    public FolderColourTests()
    {
        DataHome.Clean();
        EntryColourProviders.Clear();
        EntryMarkProviders.Clear();
        ChosenEntry.Clear();
    }

    /// <summary>A marked folder wears its colour on its own icon, and its children are not tinted.</summary>
    [Fact]
    public void AMarkedFolderWearsItsColourOnItsIconAndNotOnItsChildren()
    {
        var root = Scratch.New("folder-colour-listing");
        var marked = Directory.CreateDirectory(Path.Combine(root, "marked")).FullName;
        var file = Path.Combine(marked, "hero.psd");

        File.WriteAllText(file, "x");

        EntryColourProviders.Register(new FixedColour(marked));

        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(root, new HideRegistry(host.Config), host.Config, host.Files);
        var browser = new Browser(host.Log, shared, root, host.LogicalViews);
        var control = new DirectoryControl(
            host,
            shared,
            browser,
            new Clip(host.Log),
            new LogicalClip()
        );
        var window = new Window
        {
            Width = 900,
            Height = 600,
            Content = control,
        };

        window.Show();
        window.Activate();
        Pump();

        try
        {
            control.OpenAt(root);
            Pump();

            var view = window.GetVisualDescendants().OfType<EntryView<Entry>>().First();
            var ground = Assert.IsType<Grid>(view.Content);

            Assert.Equal(0, Assert.IsAssignableFrom<ISolidColorBrush>(ground.Background).Color.A);

            var list = view.GetVisualDescendants().OfType<ListBox>().First();
            var markedEntry = Assert.Single(list.Items.OfType<Entry>(), item => item.Path == marked);
            var markedTile = Assert.IsType<ListBoxItem>(list.ContainerFromItem(markedEntry));

            // The colour is the marked entry's own answer: the hue of it over the entry's picture, or, on a
            // machine whose theme holds no picture for the entry, the colour of the square that stands in
            // for it. Both are the icon saying the folder's own colour rather than a colour under it.
            var icon = markedTile.GetVisualDescendants().ToArray();
            var wearing =
                icon.OfType<Image>().Any(image => image.BlendMode == BitmapBlendingMode.Hue)
                || icon
                    .OfType<Border>()
                    .Any(border =>
                        border.Background is ISolidColorBrush brush && brush.Color == Colors.Red
                    );

            Assert.True(wearing, "the marked folder wears no colour on its icon");

            // A row under it inherits the colour, and an inherited row carries neither a tint nor a line under
            // its name: the colour belongs to the folder it was given to.
            control.OpenAt(marked);
            Pump();

            list = window.GetVisualDescendants().OfType<ListBox>().First();
            var hero = Assert.Single(list.Items.OfType<Entry>(), item => item.Path == file);
            var heroTile = Assert.IsType<ListBoxItem>(list.ContainerFromItem(hero));

            Assert.DoesNotContain(
                heroTile.GetVisualDescendants().OfType<Image>(),
                image => image.BlendMode == BitmapBlendingMode.Hue
            );
            Assert.DoesNotContain(
                heroTile.GetVisualDescendants().OfType<Border>(),
                border => border.BorderThickness.Bottom > 0 && border.BorderThickness.Top == 0
            );
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A showing that names a directory chooses it rather than entering it.</summary>
    [Fact]
    public void AShowingChoosesADirectory()
    {
        var root = Scratch.New("folder-colour-showing");
        var tree = Directory.CreateDirectory(Path.Combine(root, "tree")).FullName;

        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(root, new HideRegistry(host.Config), host.Config, host.Files);
        var browser = new Browser(host.Log, shared, root, host.LogicalViews);
        var control = new DirectoryControl(
            host,
            shared,
            browser,
            new Clip(host.Log),
            new LogicalClip()
        );
        var window = new Window
        {
            Width = 900,
            Height = 600,
            Content = control,
        };

        window.Show();
        window.Activate();
        Pump();

        try
        {
            control.OpenAt(root);
            Pump();
            control.RevealSelected(tree);
            Pump();

            Assert.Equal(tree, ChosenDirectory.Current?.Path);

            var view = window.GetVisualDescendants().OfType<EntryView<Entry>>().First();
            var list = view.GetVisualDescendants().OfType<ListBox>().First();
            var entry = Assert.Single(list.Items.OfType<Entry>(), item => item.Path == tree);

            Assert.Same(entry, list.SelectedItem);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A tree's park is worn on its folder tile, in the look's own ground.</summary>
    [Fact]
    public void ATreeMarkIsWornOnTheFolderTile()
    {
        var root = Scratch.New("folder-colour-park");
        var tree = Directory.CreateDirectory(Path.Combine(root, "tree")).FullName;

        Mark(tree, "11111111-1111-1111-1111-111111111111");

        var colours = new RolaTreeColours();

        EntryMarkProviders.Register(new RolaTreeIcon(colours));

        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(root, new HideRegistry(host.Config), host.Config, host.Files);
        var browser = new Browser(host.Log, shared, root, host.LogicalViews);

        RorolalaVCSPlugin.Icons.Declare(host.Icons);

        var control = new DirectoryControl(
            host,
            shared,
            browser,
            new Clip(host.Log),
            new LogicalClip()
        );
        var window = new Window
        {
            Width = 900,
            Height = 600,
            Content = control,
        };

        window.Show();
        window.Activate();
        Pump();

        try
        {
            var view = window.GetVisualDescendants().OfType<EntryView<Entry>>().First();
            var list = view.GetVisualDescendants().OfType<ListBox>().First();
            var entry = Assert.Single(list.Items.OfType<Entry>(), item => item.Path == tree);
            var container = Assert.IsType<ListBoxItem>(list.ContainerFromItem(entry));
            var row = Assert.Single(container.GetVisualDescendants().OfType<BadgeRow>());
            var badge = Assert.Single(row.Children);
            var expected = RorolalaColours.Mid;
            var fill = badge.GetSelfAndVisualDescendants()
                .OfType<Border>()
                .FirstOrDefault(border =>
                    border.Background is ISolidColorBrush brush && brush.Color == expected
                );

            Assert.NotNull(fill);

            // The bead is the look's ground, ringed rather than shadowed, and the park itself is the look's ink:
            // the colour marks where the tree reaches, and the badge says what the folder is.
            Assert.Contains(
                badge.GetSelfAndVisualDescendants().OfType<Border>(),
                border =>
                    border.Background is ISolidColorBrush brush
                    && brush.Color == RorolalaColours.Front
            );

            Assert.Equal(1, fill.BorderThickness.Left);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A tree's colour is the hue of its id, so a copy of the tree is the same colour.</summary>
    [Fact]
    public void ATreeColourComesFromTheIdInItsMark()
    {
        var root = Scratch.New("folder-colour-tree");
        var one = Directory.CreateDirectory(Path.Combine(root, "one")).FullName;
        var same = Directory.CreateDirectory(Path.Combine(root, "same")).FullName;
        var other = Directory.CreateDirectory(Path.Combine(root, "other")).FullName;
        var file = Path.Combine(one, "hero.psd");

        Mark(one, "11111111-1111-1111-1111-111111111111");
        Mark(same, "11111111-1111-1111-1111-111111111111");
        Mark(other, "22222222-2222-2222-2222-222222222222");
        File.WriteAllText(file, "x");

        var colours = new RolaTreeColours();
        var first = colours.Colour(new Entry(one, EntryKind.Directory));

        Assert.NotNull(first);
        Assert.Equal(first, colours.Colour(new Entry(same, EntryKind.Directory)));
        Assert.NotEqual(first, colours.Colour(new Entry(other, EntryKind.Directory)));
        Assert.Null(colours.Colour(new Entry(file, EntryKind.File)));
    }

    /// <summary>Writes the id a tree's mark holds.</summary>
    /// <param name="directory">The tree's directory.</param>
    /// <param name="id">The Layout entry's id.</param>
    private static void Mark(string directory, string id)
    {
        var mark = Directory.CreateDirectory(Path.Combine(directory, ".rola-tree")).FullName;

        File.WriteAllText(Path.Combine(mark, "id"), id);
    }

    /// <summary>Lets the window run a few turns of its loop, which is what lays a dock out and draws it.</summary>
    private static void Pump()
    {
        for (var turn = 0; turn < 30; turn++)
        {
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
            Dispatcher.UIThread.RunJobs();
            Thread.Sleep(10);
        }
    }

    /// <summary>A colour provider that marks one directory red and nothing else.</summary>
    private sealed class FixedColour : IEntryColourProvider
    {
        /// <summary>The directory the provider marks.</summary>
        private readonly string _marked;

        /// <summary>Makes a provider that marks one directory.</summary>
        /// <param name="marked">The directory to mark.</param>
        public FixedColour(string marked) => _marked = marked;

        /// <inheritdoc />
        public string Id => "it.folder.colour";

        /// <inheritdoc />
        public Color? Colour(Entry entry) =>
            entry.Kind is EntryKind.Directory
            && string.Equals(entry.Path, _marked, StringComparison.Ordinal)
                ? Colors.Red
                : null;
    }
}
