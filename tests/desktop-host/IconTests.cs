using FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The pictures a plugin draws for its own furniture: that what the manifest names reached the assembly.
/// </summary>
/// <remarks>
/// What is checked is the link between the three things a picture passes through — the manifest, the run that
/// draws it, and the assembly the plugin is built as — because a break in any of them is a window that opens
/// with a hole in its toolbar and nothing said. What is not checked is the drawing: that needs a renderer, and
/// it is <c>./run.sh icons</c> that runs one.
/// </remarks>
public sealed class IconTests
{
    /// <summary>Every picture the manifest names is embedded in this plugin's own assembly.</summary>
    [Fact]
    public void EveryPictureTheManifestNamesIsEmbedded()
    {
        Assert.NotEmpty(Icons.Pictures);

        foreach (var (key, name) in Icons.Pictures)
        {
            var resource = $"FileSystemPlugin.icons.{name}.png";

            using var stream = typeof(Icons).Assembly.GetManifestResourceStream(resource);

            Assert.NotNull(stream);
            Assert.StartsWith("rorolala_file_system.", key, StringComparison.Ordinal);
            Assert.EndsWith(name, key, StringComparison.Ordinal);
        }
    }
}
