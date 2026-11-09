using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Media;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaVCSPlugin;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What a marked folder does to the directory dock and to the tree: the ground of a listing inside it, the
/// row of an entry under it, and the icon of an entry that is a tree of its own.
/// </summary>
/// <remarks>
/// The colour is a rule a plugin registers, and the File System plugin is what lays it out. What is checked
/// here is the layering the two agree on: an entry inherits the nearest marked directory, a row of it under a
/// marked directory is drawn darker than the directory itself, and a listing inside one wears the colour in the
/// bright ground it is read on.
/// </remarks>
public sealed class FolderColourTests
{
    /// <summary>Begins each test from no provider and no configuration of its own.</summary>
    public FolderColourTests()
    {
        DataHome.Clean();
        EntryColourProviders.Clear();
    }

    /// <summary>A listing inside a marked folder wears the colour in its ground and in the row of an entry.</summary>
    [Fact]
    public void AListingInsideAMarkedFolderWearsItsColour()
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
            control.OpenAt(marked);
            Pump();

            var view = window.GetVisualDescendants().OfType<EntryView<Entry>>().First();
            var ground = Assert.IsType<Grid>(view.Content);
            var surface = Assert.IsType<SolidColorBrush>(ground.Background).Color;

            Assert.Equal(FolderColours.Surface(marked), surface);

            var list = view.GetVisualDescendants().OfType<ListBox>().First();
            var entry = Assert.Single(list.Items.OfType<Entry>(), item => item.Path == file);
            var container = Assert.IsType<ListBoxItem>(list.ContainerFromItem(entry));
            var tile = container.GetVisualDescendants()
                .OfType<Border>()
                .FirstOrDefault(border => border.Child is StackPanel && border.Padding.Left > 0);

            Assert.NotNull(tile);
            Assert.Equal(
                FolderColours.Wash(entry),
                Assert.IsType<SolidColorBrush>(tile!.Background).Color
            );
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
