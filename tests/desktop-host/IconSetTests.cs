using FileSystemPlugin;
using RorolalaDesktop.Contract;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The icon sets packaged beside the program: what they say about themselves, and what their cache records.
/// </summary>
/// <remarks>
/// Drawing a set is exercised rather than stood in for: the cache is the whole point of the feature, and a
/// test that only read the theme file would not meet the renderer, the SHA-1 check or the manifest. The files
/// are made in a scratch directory, so the test does not depend on what this machine packages.
/// </remarks>
public sealed class IconSetTests
{
    /// <summary>A set states its name, authors, description, sizes, fallback pictures and rules.</summary>
    [Fact]
    public void ASetIsReadAsItDescribesItself()
    {
        var set = OneSet();

        Assert.Equal("sample", set.Id);
        Assert.Equal("Sample set", set.Name);
        Assert.Equal(new[] { "A Person", "Another" }, set.Authors);
        Assert.Equal("A description of the set.", set.Description);
        Assert.Equal(new[] { 16, 32 }, set.Sizes);
        Assert.Equal(
            new[] { IconCategory.Directory, IconCategory.Text },
            set.Sources.Select(source => source.Category)
        );
        Assert.Equal(3, set.Rules.Count);
    }

    /// <summary>Rules are tried in the order the set writes them, and only for the kinds they name.</summary>
    /// <remarks>
    /// The first rule is the one for <c>.ma</c> alone, and the second is for <c>.ma</c> and <c>.mb</c>: a
    /// <c>.ma</c> file must be drawn by the first, which is what "the earlier one wins" means. The last rule
    /// is a directory rule, and a file named like its pattern must not take it.
    /// </remarks>
    [Fact]
    public void RulesAreTriedInOrderAndByKind()
    {
        var set = OneSet();

        Assert.Equal("./icon_maya.svg", First(set, "scene.ma", EntryKind.File)?.RelativePath);
        Assert.Equal("./icon_maya.svg", First(set, "SCENE.MA", EntryKind.File)?.RelativePath);
        Assert.Equal("./icon_other.svg", First(set, "scene.mb", EntryKind.File)?.RelativePath);
        Assert.Equal("./icon_cache.svg", First(set, "cache", EntryKind.Directory)?.RelativePath);
        Assert.Null(First(set, "notes.txt", EntryKind.File));
        Assert.Null(First(set, "scene.ma", EntryKind.Directory));
        Assert.Null(First(set, "cache", EntryKind.File));
    }

    /// <summary>The fallback rules tell the kinds of entry apart by name.</summary>
    [Fact]
    public void FallbackRulesClassifyByName()
    {
        Assert.Equal(IconCategory.Directory, IconRules.Of(new Entry("somewhere", EntryKind.Directory)));
        Assert.Equal(IconCategory.Directory, IconRules.Of(new Entry("somewhere.txt", EntryKind.Directory)));
        Assert.Equal(IconCategory.Text, IconRules.Of(new Entry("notes.md", EntryKind.File)));
        Assert.Equal(IconCategory.Text, IconRules.Of(new Entry("Dockerfile", EntryKind.File)));
        Assert.Equal(IconCategory.Text, IconRules.Of(new Entry("main.rs", EntryKind.File)));
        Assert.Equal(IconCategory.Audio, IconRules.Of(new Entry("take.flac", EntryKind.File)));
        Assert.Equal(IconCategory.Media, IconRules.Of(new Entry("poster.psd", EntryKind.File)));
        Assert.Equal(IconCategory.Media, IconRules.Of(new Entry("clip.mp4", EntryKind.File)));
        Assert.Equal(IconCategory.Binary, IconRules.Of(new Entry("archive.zip", EntryKind.File)));
        Assert.Equal(IconCategory.Binary, IconRules.Of(new Entry("no-extension", EntryKind.File)));
    }

    /// <summary>A rule's picture is what the listing draws, and the fallback still answers a name no rule wants.</summary>
    /// <remarks>
    /// The set in this test has no binary fallback, so a <c>.ma</c> file that drew nothing would be a null
    /// picture rather than a picture of another kind: what is checked is that the rule is the thing that
    /// answered it, not merely that something did.
    /// </remarks>
    [Fact]
    public void ARulePictureIsWhatTheListingDraws()
    {
        var set = OneSet();
        var cache = Path.Combine(IconSets.CacheRoot, set.Id);
        var ready = new ManualResetEventSlim();

        if (Directory.Exists(cache))
        {
            Directory.Delete(cache, recursive: true);
        }

        try
        {
            IconSets.Adopt([set], _ => { }, ready.Set);
            IconSets.Choose(set.Id);

            Assert.True(ready.Wait(TimeSpan.FromSeconds(10)), "the set was not drawn");

            using var ruled = IconSets.Picture(new Entry("scene.ma", EntryKind.File), 16);
            using var fallback = IconSets.Picture(new Entry("notes.txt", EntryKind.File), 16);

            Assert.NotNull(ruled);
            Assert.NotNull(fallback);
        }
        finally
        {
            if (Directory.Exists(cache))
            {
                Directory.Delete(cache, recursive: true);
            }
        }
    }

    /// <summary>The cache is drawn once, and drawn again when a source it was drawn from changes.</summary>
    /// <remarks>
    /// What is compared is the manifest's SHA-1 and the picture it describes: a cache that was served from
    /// the old source after the source changed is the failure this feature exists to prevent, and a picture
    /// whose bytes did not change is not evidence either way.
    /// </remarks>
    [Fact]
    public void AChangedSourceIsDrawnAgain()
    {
        var folder = Scratch.New("icon-set-source");
        var set = OneSet(folder, "#000000");
        var cache = Path.Combine(Scratch.New("icon-set-cache"), "root");
        var warnings = new List<string>();

        Assert.True(IconSets.Build(set, cache, warnings.Add));
        Assert.Empty(warnings);

        var picture = Path.Combine(cache, set.Id, IconSets.NameOf("./fallback-directory.svg", 16));
        var manifest = Path.Combine(cache, set.Id, "manifest.json");

        Assert.True(File.Exists(picture));
        Assert.Contains("sha1", File.ReadAllText(manifest), StringComparison.OrdinalIgnoreCase);
        Assert.Equal(10, Directory.GetFiles(Path.Combine(cache, set.Id), "*.png").Length);

        // What a rule names is drawn too, even though it is not one of the fallback pictures.
        Assert.True(
            File.Exists(
                Path.Combine(cache, set.Id, IconSets.NameOf("./icon_maya.svg", 16))
            )
        );
        Assert.True(
            File.Exists(
                Path.Combine(cache, set.Id, IconSets.NameOf("./icon_cache.svg", 32))
            )
        );

        var drawn = File.ReadAllBytes(picture);

        // Agreeing with the manifest draws nothing: the second run is the one a start-up makes every time.
        Assert.True(IconSets.Build(set, cache, warnings.Add));
        Assert.Equal(drawn, File.ReadAllBytes(picture));

        File.WriteAllText(Path.Combine(set.Folder, "fallback-directory.svg"), Square("#FFFFFF"));

        Assert.True(IconSets.Build(set, cache, warnings.Add));
        Assert.NotEqual(drawn, File.ReadAllBytes(picture));
    }

    /// <summary>
    /// A set in a scratch directory, drawn with a square of the colour given.
    /// </summary>
    /// <param name="root">Where to put the set's directory, or nothing for a fresh scratch directory.</param>
    /// <param name="colour">What the directory picture is filled with.</param>
    /// <returns>The set that was written.</returns>
    private static IconSet OneSet(string? root = null, string colour = "#000000")
    {
        root ??= Scratch.New("icon-set-source");

        var folder = Path.Combine(root, "sample");
        Directory.CreateDirectory(folder);

        File.WriteAllText(
            Path.Combine(folder, "icon-theme.toml"),
            """
            [metadata]
            name = "Sample set"
            authors = [ "A Person", "Another" ]
            description = "A description of the set."

            [generation]
            mode = "svg"
            sizes = [ 16, 32 ]

            [fallback]
            directory-format = "./fallback-directory.svg"
            text-format = "./fallback-text.svg"

            [[rules]]
            regex = [ ".ma$" ]
            type = [ "file" ]
            icon = "./icon_maya.svg"

            [[rules]]
            regex = ".ma$|.mb$"
            type = "file"
            icon = "./icon_other.svg"

            [[rules]]
            regex = [ "^cache$" ]
            type = [ "directory" ]
            icon = "./icon_cache.svg"
            """
        );

        File.WriteAllText(Path.Combine(folder, "fallback-directory.svg"), Square(colour));
        File.WriteAllText(Path.Combine(folder, "fallback-text.svg"), Square("#336699"));
        File.WriteAllText(Path.Combine(folder, "icon_maya.svg"), Square("#CC3300"));
        File.WriteAllText(Path.Combine(folder, "icon_other.svg"), Square("#33CC33"));
        File.WriteAllText(Path.Combine(folder, "icon_cache.svg"), Square("#3333CC"));

        return Assert.Single(IconSets.Discover(root, _ => { }));
    }

    /// <summary>The first rule that matches an entry, as the set would try them.</summary>
    /// <param name="set">The set whose rules are tried.</param>
    /// <param name="path">The entry's path.</param>
    /// <param name="kind">Whether the entry is a file or a directory.</param>
    /// <returns>The first matching rule, or nothing.</returns>
    private static IconRule? First(IconSet set, string path, EntryKind kind) =>
        set.Rules.FirstOrDefault(rule => rule.Matches(new Entry(path, kind)));

    /// <summary>An SVG that fills its square with one colour.</summary>
    /// <param name="colour">The colour to fill with.</param>
    /// <returns>The SVG document.</returns>
    private static string Square(string colour) =>
        $"""
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
          <rect x="0" y="0" width="24" height="24" fill="{colour}"/>
        </svg>
        """;
}
