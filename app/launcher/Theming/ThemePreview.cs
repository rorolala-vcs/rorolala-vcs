using System.Diagnostics.CodeAnalysis;
using System.Runtime.InteropServices;
using Avalonia;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using RorolalaDesktop.Contract;

namespace RorolalaLauncher.Theming;

/// <summary>
/// The picture a day-or-night choice is made by: one small window, drawn once per variant.
/// </summary>
/// <remarks>
/// The artwork is a wireframe of the program's own window — a border, a row, some lines of text, a panel
/// beside them — drawn in six flat colours that stand for the six a look is made of. So it is recoloured
/// rather than redrawn: each colour in the file is replaced by the colour the variant gives that part, and
/// what comes out is a window in the look the choice would put the program in.
/// <para>
/// The choice that follows the system is the same picture twice, split along the diagonal from its top-right
/// corner to its bottom-left: the part above the line is the light look and the part below it is the dark
/// one, which is what the choice means rather than the name of it written out.
/// </para>
/// <para>
/// Every picture is made once per palette and kept: a palette is a value, so two palettes that are the same
/// colours are one picture, and a dialog drawn again does not have to recolour anything.
/// </para>
/// </remarks>
internal static class ThemePreview
{
    /// <summary>Where the artwork travels inside the assembly.</summary>
    private const string Shelf = "RorolalaLauncher.theme-preview.png";

    /// <summary>The colour of a part that is what tells one surface from another.</summary>
    private const int Splitter = 0xFF0060;

    /// <summary>The colour of a part that is written on rather than read: ink mixed halfway to the ground.</summary>
    private const int MutedInk = 0x005CFF;

    /// <summary>The colour of a part that is written on.</summary>
    private const int Ink = 0xFFF100;

    /// <summary>The colour of a part that is a face standing out from the ground.</summary>
    private const int Bright = 0x00FF08;

    /// <summary>The colour of a part that is the ground.</summary>
    private const int Mid = 0xFF7800;

    /// <summary>The colour of a part that is a hollow in the ground.</summary>
    private const int Dark = 0x00EDFF;

    /// <summary>The artwork, read once and kept.</summary>
    private static readonly Lazy<Bitmap> Artwork = new(Load);

    /// <summary>The pictures made so far, by the palette they are drawn in.</summary>
    private static readonly Dictionary<ThemePalette, Bitmap> Variants = [];

    /// <summary>The pictures made so far, by the two palettes they are split between.</summary>
    private static readonly Dictionary<(ThemePalette Light, ThemePalette Dark), Bitmap> Splits = [];

    /// <summary>
    /// One palette's picture.
    /// </summary>
    /// <param name="palette">The colours the picture is drawn in.</param>
    /// <returns>The picture.</returns>
    public static Bitmap Of(ThemePalette palette)
    {
        if (Variants.TryGetValue(palette, out var held))
        {
            return held;
        }

        var picture = Painted(palette, null);

        Variants[palette] = picture;

        return picture;
    }

    /// <summary>
    /// The picture that is one palette above a line and another below it, which is the choice that follows
    /// the system.
    /// </summary>
    /// <param name="light">The colours above the line.</param>
    /// <param name="dark">The colours below it.</param>
    /// <returns>The picture.</returns>
    public static Bitmap HalfAndHalf(ThemePalette light, ThemePalette dark)
    {
        if (Splits.TryGetValue((light, dark), out var held))
        {
            return held;
        }

        var picture = Painted(light, dark);

        Splits[(light, dark)] = picture;

        return picture;
    }

    /// <summary>
    /// Recolours the artwork, one palette or two.
    /// </summary>
    /// <remarks>
    /// The split is the diagonal from the picture's top-right corner to its bottom-left one, and the part
    /// above it — the part whose pixels satisfy <c>x/width + y/height &lt; 1</c> — is the first palette. The
    /// two corners the line passes through belong to neither side in particular; they are drawn in the second
    /// palette, which is what makes the line read as one rather than as a step.
    /// </remarks>
    /// <param name="upper">The palette the part above the line is drawn in.</param>
    /// <param name="lower">The palette below it, or nothing when there is one palette.</param>
    /// <returns>The picture.</returns>
    private static Bitmap Painted(ThemePalette upper, ThemePalette? lower)
    {
        var artwork = Artwork.Value;
        var size = artwork.PixelSize;

        var above = Map(upper);
        var below = lower is { } palette ? Map(palette) : above;

        var painted = new WriteableBitmap(
            size,
            new Vector(96, 96),
            PixelFormat.Bgra8888,
            AlphaFormat.Unpremul
        );

        using (var frame = painted.Lock())
        {
            // The artwork is copied into the bitmap being made rather than read a pixel at a time: what it
            // is to become is the same picture with six colours replaced, and the toolkit's own copy is the
            // one statement of how a bitmap's pixels are laid out.
            artwork.CopyPixels(frame);

            var stride = frame.RowBytes;
            var pixels = new byte[stride * size.Height];

            Marshal.Copy(frame.Address, pixels, 0, pixels.Length);

            var whole = size.Width * size.Height;

            for (var y = 0; y < size.Height; y++)
            {
                for (var x = 0; x < size.Width; x++)
                {
                    var at = (y * stride) + (x * 4);
                    var colour = (pixels[at + 2] << 16) | (pixels[at + 1] << 8) | pixels[at];

                    var side = lower is null
                        || (((x + 1) * size.Height) + ((y + 1) * size.Width)) <= whole;

                    if ((side ? above : below).TryGetValue(colour, out var paintedColour))
                    {
                        pixels[at + 0] = paintedColour.B;
                        pixels[at + 1] = paintedColour.G;
                        pixels[at + 2] = paintedColour.R;
                    }
                }
            }

            Marshal.Copy(pixels, 0, frame.Address, pixels.Length);
        }

        return painted;
    }

    /// <summary>What each colour of the artwork is drawn in under one palette.</summary>
    /// <param name="palette">The colours in force.</param>
    /// <returns>The colours, by the artwork's own.</returns>
    private static Dictionary<int, Color> Map(ThemePalette palette) =>
        new()
        {
            [Splitter] = palette.Splitter,
            [MutedInk] = Mix(palette.Front, palette.Mid),
            [Ink] = palette.Front,
            [Bright] = palette.Bright,
            [Mid] = palette.Mid,
            [Dark] = palette.Dark,
        };

    /// <summary>
    /// The colour halfway between two, which is what the artwork's muted ink stands for.
    /// </summary>
    /// <remarks>
    /// Halfway is enough for the one use it has: what is written on a surface is the surface's ink, and what
    /// is written beside it is that ink given up halfway to the surface, so that it is read as being there
    /// without being read as the thing itself.
    /// </remarks>
    /// <param name="ink">The ink.</param>
    /// <param name="ground">What it stands on.</param>
    /// <returns>The colour between them.</returns>
    private static Color Mix(Color ink, Color ground) =>
        Color.FromRgb(
            (byte)((ink.R + ground.R) / 2),
            (byte)((ink.G + ground.G) / 2),
            (byte)((ink.B + ground.B) / 2)
        );

    /// <summary>
    /// The artwork itself, read from the assembly it travels in.
    /// </summary>
    /// <returns>The picture.</returns>
    // The artwork is kept for as long as the program runs — it is the one lazy value — so it is handed on
    // rather than lost at the end of this scope.
    [SuppressMessage(
        "Reliability",
        "CA2000:Dispose objects before losing scope",
        Justification = "the artwork is held for the life of the program by the lazy value that made it"
    )]
    private static Bitmap Load()
    {
        using var stream =
            typeof(ThemePreview).Assembly.GetManifestResourceStream(Shelf)
            ?? throw new InvalidOperationException($"the preview `{Shelf}` is not embedded");

        return new Bitmap(stream);
    }
}
