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
        File.WriteAllText(Path.Combine(folder, "File_Manager.json"), "{}");
        File.WriteAllText(Path.Combine(folder, "Terminal.json"), "{}");

        var presets = LayoutPresets.In(folder);

        Assert.Equal(["File Manager", "Terminal"], presets.Select(preset => preset.Name));
        Assert.All(presets, preset => Assert.EndsWith(".json", preset.Path, StringComparison.Ordinal));
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
