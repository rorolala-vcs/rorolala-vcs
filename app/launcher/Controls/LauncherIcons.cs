using System.Diagnostics.CodeAnalysis;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Media.Imaging;

namespace RorolalaLauncher;

/// <summary>
/// The pictures this program is drawn with, by the key the manifest names each under.
/// </summary>
/// <remarks>
/// The three the toolbar wears are this project's own set, drawn from the pinned Material Design set by
/// <c>./run.sh icons</c> and named by the generated <see cref="Icons"/>; the two a window's frame carries are
/// the Desktop program's own files, because the two programs draw one frame.
/// <para>
/// They are read once and kept: a picture is stamped every time its button is drawn, and a file opened once
/// per hover would be the wrong price for a mark.
/// </para>
/// </remarks>
internal static class LauncherIcons
{
    /// <summary>What a picture is held under inside the assembly, which is where the manifest's files travel.</summary>
    private const string Shelf = "RorolalaLauncher.icons.";

    /// <summary>Every picture read so far, by the name the set knows it by.</summary>
    private static readonly Dictionary<string, Bitmap> Read = new(StringComparer.Ordinal);

    /// <summary>The picture the minimize button of a frame carries.</summary>
    public static Bitmap Minimize => Named("minimize");

    /// <summary>The picture the close button of a frame carries.</summary>
    public static Bitmap Close => Named("close");

    /// <summary>
    /// One of this program's own pictures, by the key the generated keys name it under.
    /// </summary>
    /// <param name="key">The key, from <see cref="Icons"/>.</param>
    /// <returns>The picture.</returns>
    public static Bitmap Of(string key)
    {
        foreach (var (named, name) in Icons.Pictures)
        {
            if (string.Equals(named, key, StringComparison.Ordinal))
            {
                return Named(name);
            }
        }

        throw new ArgumentOutOfRangeException(nameof(key), key, "no picture of this program is held under that key");
    }

    /// <summary>
    /// One of the pictures stamped in an ink, which is how the Desktop program draws the same file.
    /// </summary>
    /// <remarks>
    /// The picture is an opacity mask over a fill rather than an image: what a button under the pointer, and
    /// the one that closes the window, are filled with is the look's answer, and a picture that carried its
    /// own colour would be a mark that vanished into the fill the day the look changed.
    /// </remarks>
    /// <param name="picture">The picture.</param>
    /// <param name="extent">How wide and tall it is drawn.</param>
    /// <param name="ink">The colour it is stamped in.</param>
    /// <returns>What to draw.</returns>
    public static Border Glyph(Bitmap picture, double extent, IBrush ink)
    {
        ArgumentNullException.ThrowIfNull(picture);
        ArgumentNullException.ThrowIfNull(ink);

        var glyph = new Border
        {
            Width = extent,
            Height = extent,
            Background = ink,
            OpacityMask = new ImageBrush(picture) { Stretch = Stretch.Uniform },
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            IsHitTestVisible = false,
        };

        // The file is drawn at several times the size it is shown at — see the icon set the Desktop program
        // reads the same file from — so what is scaled here is a large bitmap being reduced: a filter that
        // samples a few of its pixels leaves a thin stroke broken.
        RenderOptions.SetBitmapInterpolationMode(glyph, BitmapInterpolationMode.HighQuality);

        return glyph;
    }

    /// <summary>The picture the set knows by a name, read the first time it is asked for.</summary>
    /// <param name="name">The name the set knows it by.</param>
    /// <returns>The picture.</returns>
    private static Bitmap Named(string name)
    {
        if (Read.TryGetValue(name, out var held))
        {
            return held;
        }

        var picture = Embedded($"{Shelf}{name}.png");

        Read[name] = picture;

        return picture;
    }

    /// <summary>
    /// A picture of this program's own, read from the assembly it travels in.
    /// </summary>
    /// <param name="resource">The name it is embedded under.</param>
    /// <returns>The picture.</returns>
    // The picture is kept for as long as the program runs — it is cached by `Named` — so it is handed on
    // rather than lost at the end of this scope.
    [SuppressMessage(
        "Reliability",
        "CA2000:Dispose objects before losing scope",
        Justification = "the picture is held for the life of the program by the cache that made it"
    )]
    private static Bitmap Embedded(string resource)
    {
        using var stream =
            typeof(LauncherIcons).Assembly.GetManifestResourceStream(resource)
            ?? throw new InvalidOperationException($"the icon `{resource}` is not embedded");

        return new Bitmap(stream);
    }
}
