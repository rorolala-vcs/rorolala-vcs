using System.Diagnostics.CodeAnalysis;
using Avalonia.Media.Imaging;
using RorolalaDesktop.Contract;

namespace GitVCSPlugin;

/// <summary>The pictures this plugin contributes to the icon library.</summary>
/// <remarks>
/// None is drawn here: a picture contributed under a key is what the views stamp as a mask, and what
/// this class holds is the reading of it out of the assembly it travels in, since a plugin is
/// discovered as one file and a picture beside it would be a second thing to keep in step. The keys
/// are the manifest's (`app/desktop/icons.toml`), written out by `cargo dev icons`.
/// </remarks>
internal static partial class Icons
{
    /// <summary>Contributes this plugin's pictures to the library the host hands on.</summary>
    /// <param name="library">The library to add them to.</param>
    // The pictures are handed to the library, which holds them for as long as the program runs: they
    // are not lost at the end of this scope, so there is nothing here to dispose.
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
            library.Add(key, From($"GitVCSPlugin.icons.{name}.png"));
        }

        // The category this plugin's entries sit under wears the Git logomark: the renderer looks a
        // category's picture up by the category's key, and this plugin's key is its own. The
        // logomark is not from the set, so it is vendored under `vendor/git` and drawn there.
        library.Add(
            GitRolaActions.Category,
            From("GitVCSPlugin.vendor.git.Git-Icon-White.png")
        );
    }

    /// <summary>A picture of this plugin's own, read from the assembly it travels in.</summary>
    /// <param name="resource">The resource's name in the assembly.</param>
    /// <returns>The picture.</returns>
    private static Bitmap From(string resource)
    {
        using var stream =
            typeof(Icons).Assembly.GetManifestResourceStream(resource)
            ?? throw new InvalidOperationException($"the icon `{resource}` is not embedded");

        return new Bitmap(stream);
    }
}
