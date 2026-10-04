using System.Diagnostics.CodeAnalysis;
using Avalonia.Media.Imaging;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// The pictures this plugin contributes to the icon library.
/// </summary>
/// <remarks>
/// Two, and neither is drawn here: the File System plugin draws an entry's mark in the ink the theme gives
/// it, and a picture contributed under a key is what it draws. What is here is the reading of them out of the
/// assembly they travel in — a plugin is discovered as one file, so a picture beside it would be a second
/// thing to keep in step — and the keys are the manifest's (`app/desktop/icons.toml`), written out by
/// <c>./run.sh icons</c> rather than by hand.
/// </remarks>
internal static partial class Icons
{
    /// <summary>
    /// Contributes this plugin's pictures to the library the host hands on.
    /// </summary>
    /// <param name="library">The library to add them to.</param>
    // The pictures are handed to the library, which holds them for as long as the program runs: they are not
    // lost at the end of this scope, so there is nothing here to dispose.
    [SuppressMessage(
        "Reliability",
        "CA2000:Dispose objects before losing scope",
        Justification = "the icon library holds each picture for the life of the program, so handing one over is not losing it"
    )]
    public static void Declare(IIconLibrary library)
    {
        ArgumentNullException.ThrowIfNull(library);

        foreach (var (key, name) in Pictures)
        {
            library.Add(key, Embedded(name));
        }
    }

    /// <summary>A picture of this plugin's own, read from the assembly it travels in.</summary>
    /// <param name="name">The name the set knows it by.</param>
    /// <returns>The picture.</returns>
    private static Bitmap Embedded(string name)
    {
        var resource = $"RorolalaVCSPlugin.icons.{name}.png";

        using var stream =
            typeof(Icons).Assembly.GetManifestResourceStream(resource)
            ?? throw new InvalidOperationException($"the icon `{resource}` is not embedded");

        return new Bitmap(stream);
    }
}
