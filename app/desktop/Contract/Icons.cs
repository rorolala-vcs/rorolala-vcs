using Avalonia.Media.Imaging;

namespace RorolalaDesktop.Contract;

/// <summary>
/// The pictures the program draws, keyed by name.
/// </summary>
/// <remarks>
/// A key, not a picture, is what the rest of the contract names an icon by: a menu item, a dock's
/// header command and an entry's badge each say what they want drawn rather than carrying the
/// image. A picture contributed once under a key is therefore the one every reader of that key
/// sees, wherever it is read from.
/// <para>
/// One key names one picture, and adding another under a key replaces it — the picture is the
/// contributor's to correct, and a reader that had to choose between two would be choosing by load
/// order. A key nothing names answers nothing, and the reader draws what it would have drawn
/// without a picture.
/// </para>
/// </remarks>
public interface IIconLibrary
{
    /// <summary>Adds a picture, or replaces the one a key already names.</summary>
    /// <param name="key">The key the picture answers to.</param>
    /// <param name="picture">What to draw.</param>
    void Add(string key, Bitmap picture);

    /// <summary>The picture a key names, or nothing when it names none.</summary>
    /// <param name="key">The key to look up.</param>
    /// <returns>What to draw, or nothing.</returns>
    Bitmap? Find(string key);
}
