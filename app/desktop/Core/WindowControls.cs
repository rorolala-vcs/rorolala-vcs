using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Data;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Theming;

namespace RorolalaDesktop;

/// <summary>
/// The window's own buttons: what they say, and what they are drawn as.
/// </summary>
/// <remarks>
/// The buttons are the markup's — a button per role, on the same row as the menu — and what the toolkit
/// makes of them is the role each carries: a press on one closes, minimizes, or maximizes the window, and
/// the state each is available in follows the window. What is left to this file is the part of a button
/// that is neither markup nor toolkit: the words a reader hears it by, which are translated, and the
/// picture it carries, which is the program's own rather than the desktop's.
/// <para>
/// The pictures are Material Design's, drawn from the pinned set and read out of the program's own
/// assembly — see <see cref="Icons"/>. Each is drawn as an opacity mask over the ink of the button it sits
/// in rather than as a picture, so that a button under the pointer, and the one that closes the window,
/// take the colour the look gives them instead of one baked into the file.
/// </para>
/// <para>
/// The maximize button is the exception to one picture per button: it is drawn as one of two, and which of
/// them is on it is the window's own state to say.
/// </para>
/// </remarks>
internal static class WindowControls
{
    /// <summary>
    /// How many pixels wide and tall a button's picture is.
    /// </summary>
    /// <remarks>
    /// Smaller than the button, and smaller than the artwork's own drawn area: the set draws on a
    /// twenty-four unit grid with two units of air on every side, so a mark drawn to the edges of its
    /// target would read as a button filled with a picture rather than as a picture with a button around
    /// it. This is the size that makes a window button read like the rest of the program, where a tool's
    /// mark is eighteen pixels in a box of twenty-eight.
    /// </remarks>
    public const double GlyphSize = 12.0;

    /// <summary>
    /// How many pixels wide and tall a button's picture is where the frame is drawn the way macOS draws one.
    /// </summary>
    /// <remarks>
    /// Smaller than the mark on a button of the design's, because the light it is drawn on is smaller than a
    /// button: a mark the size of the design's would fill a light to its edge, where what macOS draws on one
    /// is a mark that leaves the colour around it to be read.
    /// </remarks>
    public const double LightGlyphSize = 8.0;

    /// <summary>The key the minimize button's picture is held under.</summary>
    public const string MinimizeKey = Icons.Minimize;

    /// <summary>The key the maximize button's picture is held under.</summary>
    public const string MaximizeKey = Icons.CropSquare;

    /// <summary>The key the picture the maximize button wears once the window is maximized is held under.</summary>
    public const string RestoreKey = Icons.FilterNone;

    /// <summary>The key the close button's picture is held under.</summary>
    public const string CloseKey = Icons.Close;

    /// <summary>
    /// A picture read from the library, or nothing when the library holds no such key.
    /// </summary>
    /// <remarks>
    /// Nothing is answered rather than a failure raised: a picture is what a button is drawn as, and a
    /// button drawn as nothing is still a button — one that closes or minimizes the window exactly as it
    /// would have. What must not happen is the window failing to open because a file it was drawn from
    /// was not beside the program.
    /// </remarks>
    /// <param name="pictures">The library to read from.</param>
    /// <param name="key">The key the picture is held under.</param>
    /// <returns>The picture, or nothing.</returns>
    public static Bitmap? Find(IIconLibrary pictures, string key)
    {
        ArgumentNullException.ThrowIfNull(pictures);

        return pictures.Find(key);
    }

    /// <summary>A button's picture, drawn in the ink of the button it is put in.</summary>
    /// <remarks>
    /// The ink is the button's own <c>Foreground</c> rather than a colour picked here, because what a
    /// button is filled with is not this file's to know: a window button under the pointer sits on the
    /// sunken ground, the one that closes it on the failure's red wash, and what is read on either is the
    /// look's answer. A picture that picked its own ink would be a mark that vanished into the fill the
    /// day the look changed.
    /// </remarks>
    /// <param name="picture">The picture to stamp.</param>
    /// <param name="extent">How many pixels wide and tall it is.</param>
    /// <returns>What to draw, which is drawn in the ink of the button it was put in.</returns>
    public static Control Glyph(Bitmap picture, double extent)
    {
        ArgumentNullException.ThrowIfNull(picture);

        var glyph = new Border
        {
            Width = extent,
            Height = extent,
            Classes = { RorolalaTheme.CaptionGlyphClass },
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            OpacityMask = new ImageBrush(picture) { Stretch = Stretch.Uniform },
        };

        // A picture is drawn at several times the size it is shown at — see `Icons.Picture` — so what is
        // scaled here is a large bitmap being reduced rather than a small one being enlarged, and a filter
        // that samples a few of its pixels leaves a glyph's thin strokes broken. The high one takes the
        // whole of what it covers.
        Avalonia.Media.RenderOptions.SetBitmapInterpolationMode(
            glyph,
            Avalonia.Media.Imaging.BitmapInterpolationMode.HighQuality
        );

        // The holder is named rather than guessed at: what states the ink is the button, and what is
        // between the two — the presenter a button puts its content in — states nothing of its own. A
        // button is named rather than a content control, which is what everything that holds anything is.
        glyph.Bind(
            Border.BackgroundProperty,
            new Binding("Foreground")
            {
                RelativeSource = new RelativeSource
                {
                    Mode = RelativeSourceMode.FindAncestor,
                    AncestorType = typeof(Button),
                },
            }
        );

        return glyph;
    }

    /// <summary>
    /// Puts a picture in a button, or takes the one that is there away.
    /// </summary>
    /// <remarks>
    /// Written as a method rather than left to the caller setting <c>Content</c>, because the picture is
    /// the button's and nothing else is: a caption button has no text, and what says what it does is the
    /// word it is named by, which a reader hears and a window never draws.
    /// <para>
    /// A button that is nothing is left alone rather than refused, because a frame drawn for a tiling window
    /// manager has none of them and gives the window nothing to draw on.
    /// </para>
    /// </remarks>
    /// <param name="button">The button to draw on, or nothing where the frame has none.</param>
    /// <param name="picture">What to draw, or nothing to draw none.</param>
    public static void Picture(Button? button, Control? picture)
    {
        if (button is null)
        {
            return;
        }

        button.Content = picture;
    }

    /// <summary>
    /// Gives a button the words it is read and reached by.
    /// </summary>
    /// <remarks>
    /// A window's buttons carry no visible text, so the name is the only thing that says what one is —
    /// to a reader, to an automation client, and to whatever enumerates a window's controls.
    /// </remarks>
    /// <param name="button">The button to name.</param>
    /// <param name="key">The translated key the name is read from.</param>
    /// <param name="i18n">Where the words are.</param>
    public static void Describe(Button button, string key, I18nService i18n) =>
        AutomationProperties.SetName(button, i18n.Get(key));
}
