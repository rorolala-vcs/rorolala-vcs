using Avalonia;
using Avalonia.Controls;
using Avalonia.Data;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Media.Imaging;

namespace RorolalaDesktop;

/// <summary>
/// A picture from the icon library, drawn as a mark in the ink of what holds it.
/// </summary>
/// <remarks>
/// The library's pictures are drawn white on nothing, so a mark is the picture cut out and painted through rather
/// than the picture itself: what colour it takes is stated by the holder (a menu item's ink, a tab's) and a mark
/// that carried its own would be one per look. The holder is named rather than guessed at, because what stands
/// between the two, the presenter a menu item or a button puts its content in, states nothing of its own.
/// </remarks>
internal static class Marks
{
    /// <summary>How many pixels wide and tall a mark in a menu item is.</summary>
    /// <remarks>
    /// The square a menu item reserves for a picture of its own, so that an item with a mark and one without keep
    /// their words in the same column rather than stepping sideways when a picture appears.
    /// </remarks>
    public const double MenuSize = 16.0;

    /// <summary>How many pixels wide and tall a mark in a dock's tab is.</summary>
    /// <remarks>
    /// The size a menu's mark is drawn at, and a whole share of the picture: what is drawn is ninety-six pixels across, so
    /// a mark of sixteen is a sixth of it and its strokes land on whole pixels rather than between them. It is the size the
    /// tab keeps its room on the left for (see <c>DockArea.Make</c>): the room before the picture, the picture, and the
    /// little room between the picture and the word.
    /// </remarks>
    public const double TabSize = 16.0;

    /// <summary>A mark for a menu item, which is what a dock's picture is in the Window menu.</summary>
    /// <param name="picture">What to stamp.</param>
    /// <returns>What to put in the item.</returns>
    public static Control Menu(Bitmap picture) => Mark(picture, MenuSize, typeof(MenuItem), null);

    /// <summary>A mark for a dock's tab, drawn in the ink of the tab it is put in.</summary>
    /// <param name="picture">What to stamp.</param>
    /// <param name="classes">What the mark is named by, where a reader looks for it.</param>
    /// <returns>What to put in the tab.</returns>
    public static Control Tab(Bitmap picture, string classes)
    {
        ArgumentNullException.ThrowIfNull(classes);

        return Mark(picture, TabSize, typeof(Button), classes);
    }

    /// <summary>
    /// One mark: a picture cut out, painted through with the ink its holder states.
    /// </summary>
    /// <remarks>
    /// The interpolation is set high because what is scaled is a picture drawn at several times the size it is shown
    /// at, see the manifest's own answer for what is drawn, so a filter that samples a few of its pixels leaves a
    /// stroke broken, and the whole of what it covers is taken instead.
    /// </remarks>
    /// <param name="picture">What to stamp.</param>
    /// <param name="extent">How many pixels wide and tall it is.</param>
    /// <param name="holder">What the ink is read from.</param>
    /// <param name="classes">What the mark is named by, or nothing for one nothing looks for.</param>
    /// <returns>What to draw.</returns>
    private static Border Mark(Bitmap picture, double extent, Type holder, string? classes)
    {
        ArgumentNullException.ThrowIfNull(picture);
        ArgumentNullException.ThrowIfNull(holder);

        var mark = new Border
        {
            Width = extent,
            Height = extent,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            OpacityMask = new ImageBrush(picture) { Stretch = Stretch.Uniform },
        };

        if (classes is not null)
        {
            mark.Classes.Add(classes);
        }

        RenderOptions.SetBitmapInterpolationMode(
            mark,
            Avalonia.Media.Imaging.BitmapInterpolationMode.HighQuality
        );

        mark.Bind(
            Border.BackgroundProperty,
            new Binding("Foreground")
            {
                RelativeSource = new RelativeSource
                {
                    Mode = RelativeSourceMode.FindAncestor,
                    AncestorType = holder,
                },
            }
        );

        return mark;
    }
}
