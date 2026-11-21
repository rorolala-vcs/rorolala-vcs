using RorolalaDesktop.Docking;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The layout presets a run offers: what a preset is called, and which files are presets.
/// </summary>
public sealed class LayoutPresetTests
{
    /// <summary>A preset is named by its file, with the underscores turned into spaces.</summary>
    [Fact]
    public void APresetIsNamedByItsFile()
    {
        var folder = Scratch.New("layout-presets");
        File.WriteAllText(Path.Combine(folder, "Version_Control.json"), "{}");
        File.WriteAllText(Path.Combine(folder, "Terminal.json"), "{}");

        var presets = LayoutPresets.In(folder);

        Assert.Equal(["Terminal", "Version Control"], presets.Select(preset => preset.Name));
        Assert.All(presets, preset => Assert.EndsWith(".json", preset.Path, StringComparison.Ordinal));
    }

    /// <summary>The shipped presets are shown in the order a person moves through the work.</summary>
    /// <remarks>
    /// The order is the one the row's tabs read in, and it is not what the file names sort to: alphabetically the
    /// version control preset stands last, after the terminal. A preset the program does not know follows the five.
    /// </remarks>
    [Fact]
    public void TheShippedPresetsAreShownInTheirOwnOrder()
    {
        var folder = Scratch.New("layout-presets-order");

        foreach (var name in new[] { "Terminal", "VCS", "Settings", "Git", "File", "Other" })
        {
            File.WriteAllText(Path.Combine(folder, $"{name}.json"), "{}");
        }

        Assert.Equal(
            ["File", "VCS", "Git", "Settings", "Terminal", "Other"],
            LayoutPresets.In(folder).Select(preset => preset.Name)
        );
    }

    /// <summary>A directory under the presets is not one of them.</summary>
    /// <remarks>
    /// The raw set a preset was adjusted from is a directory beside the presets, and a preset is one file: what
    /// is under a directory has no name the menu could show and no meaning as one of them.
    /// </remarks>
    [Fact]
    public void ADirectoryUnderThePresetsIsNotAPreset()
    {
        var folder = Scratch.New("layout-presets-raw");
        File.WriteAllText(Path.Combine(folder, "Terminal.json"), "{}");

        var raw = Directory.CreateDirectory(Path.Combine(folder, "__raw")).FullName;
        File.WriteAllText(Path.Combine(raw, "Hidden.json"), "{}");

        var presets = LayoutPresets.In(folder);

        Assert.Equal(["Terminal"], presets.Select(preset => preset.Name));
    }

    /// <summary>A directory with no preset in it offers none.</summary>
    [Fact]
    public void ADirectoryWithNoPresetOffersNone() =>
        Assert.Empty(LayoutPresets.In(Scratch.New("layout-presets-empty")));
}
