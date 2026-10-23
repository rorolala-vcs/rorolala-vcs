using System.Diagnostics.CodeAnalysis;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Data;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// The pictures this plugin contributes to the icon library.
/// </summary>
/// <remarks>
/// None is drawn here: a picture contributed under a key is what the views stamp as a mask, and what
/// this class holds is the reading of them out of the assembly they travel in — a plugin is discovered as
/// one file, so a picture beside it would be a second thing to keep in step. The keys are the manifest's
/// (`app/desktop/icons.toml`), written out by <c>cargo dev icons</c> rather than by hand.
/// </remarks>
internal static partial class Icons
{
    /// <summary>How many pixels wide and tall a picture in a menu item is.</summary>
    internal const double Menu = 16;

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

    /// <summary>A picture drawn as a mask over the ink of the menu item it is in.</summary>
    /// <remarks>
    /// The ink is the item's own <c>Foreground</c> rather than a colour picked here, because the two looks
    /// state it differently and a mark that picked its own would be a mark the item could not be read
    /// beside. It is named through the item rather than read from whatever view holds it: a menu item's
    /// template states nothing of its own, so an inherited answer would come from the menu.
    /// </remarks>
    /// <param name="picture">The picture to stamp.</param>
    /// <returns>What to draw, whose background is the item's ink.</returns>
    internal static Border Inked(Bitmap picture)
    {
        var glyph = new Border
        {
            Width = Menu,
            Height = Menu,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            OpacityMask = new ImageBrush(picture) { Stretch = Stretch.Uniform },
        };

        glyph.Bind(
            Border.BackgroundProperty,
            new Binding("Foreground")
            {
                RelativeSource = new RelativeSource
                {
                    Mode = RelativeSourceMode.FindAncestor,
                    AncestorType = typeof(Avalonia.Controls.MenuItem),
                },
            }
        );

        // Every picture is read at several times the size it is shown at, so what the mask does is a
        // reduction: the high filter keeps a thin stroke whole where a cheaper one leaves it broken.
        RenderOptions.SetBitmapInterpolationMode(
            glyph,
            BitmapInterpolationMode.HighQuality
        );

        return glyph;
    }
}
