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
    /// <summary>A set states its name, authors, description, sizes and fallback pictures.</summary>
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
        Assert.Equal(4, Directory.GetFiles(Path.Combine(cache, set.Id), "*.png").Length);

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
            """
        );

        File.WriteAllText(Path.Combine(folder, "fallback-directory.svg"), Square(colour));
        File.WriteAllText(Path.Combine(folder, "fallback-text.svg"), Square("#336699"));

        return Assert.Single(IconSets.Discover(root, _ => { }));
    }

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
