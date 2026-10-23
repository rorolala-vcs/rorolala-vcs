using System.Diagnostics.CodeAnalysis;
using Avalonia.Media.Imaging;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop;

/// <summary>
/// The pictures the host itself contributes to the icon library.
/// </summary>
/// <remarks>
/// The window's own buttons, and nothing else: everything a dock or a menu is drawn with belongs to the
/// plugin that draws it, and what is here is the part of the window the host owns. They are drawn from the
/// pinned Material Design set by <c>cargo dev icons</c> — the manifest is <c>app/desktop/icons.toml</c> —
/// and the keys below are generated from that manifest rather than written by hand.
/// <para>
/// They travel embedded in the host's own assembly rather than beside it, for the reason a plugin's do:
/// what is read at runtime is what the program is, and a picture in a directory is a second thing to keep
/// in step with the program it is drawn by.
/// </para>
/// </remarks>
internal static partial class Icons
{
    /// <summary>
    /// Contributes the host's pictures to the library every reader looks them up in.
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

    /// <summary>A picture of the host's own, read from the assembly it travels in.</summary>
    /// <param name="name">The name the set knows it by.</param>
    /// <returns>The picture.</returns>
    private static Bitmap Embedded(string name)
    {
        var resource = $"RorolalaDesktop.icons.{name}.png";

        using var stream =
            typeof(Icons).Assembly.GetManifestResourceStream(resource)
            ?? throw new InvalidOperationException($"the icon `{resource}` is not embedded");

        return new Bitmap(stream);
    }
}
