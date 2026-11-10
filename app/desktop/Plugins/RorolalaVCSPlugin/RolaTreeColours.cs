using System.Text;
using Avalonia.Media;
using FileSystemPlugin;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// The colour a rola-tree is marked with: the hue of the hash of the id it keeps in its mark.
/// </summary>
/// <remarks>
/// A tree is a directory, and a directory is the one entry the file manager can mark for everything under it:
/// the colour a tree's own row carries is the colour its whole work is read in, and a tree inside a tree is a
/// second colour on its own rows. The id is the Layout entry's, written into `.rola-tree/id`, so the colour
/// travels with the directory when it is moved and two copies of one tree are the same colour.
/// <para>
/// The hash is FNV-1a over the id's canonical hex, read as three channels and taken as a hue, which is the
/// reading the Launcher's Workspace mark uses for a path. The saturation and lightness are fixed rather than
/// hashed, so that the hue the hash names is the whole of what the colour varies by.
/// </para>
/// </remarks>
internal sealed class RolaTreeColours : IEntryColourProvider
{
    /// <summary>How far the colour is from grey.</summary>
    private const double Saturation = 0.80;

    /// <summary>How light the colour is.</summary>
    private const double Lightness = 0.58;

    /// <summary>What was read from a tree's mark, so that one directory is not read once per row.</summary>
    private readonly Dictionary<string, Color?> _colours = new(StringComparer.Ordinal);

    /// <inheritdoc />
    public string Id => "rorolala.vcs.tree";

    /// <inheritdoc />
    public Color? Colour(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        // A tree is a directory: a file has no mark of its own to read, and what it inherits is the answer of
        // the directory it is in rather than a second question about the file. The platform's own root, the
        // computer, has no path to hold a mark, so it is not asked of the filesystem.
        if (entry.Kind is not EntryKind.Directory || string.IsNullOrEmpty(entry.Path))
        {
            return null;
        }

        if (_colours.TryGetValue(entry.Path, out var known))
        {
            return known;
        }

        var colour = TreeColour(entry.Path);
        _colours[entry.Path] = colour;

        return colour;
    }

    /// <inheritdoc />
    public void Forget() => _colours.Clear();

    /// <summary>The colour a directory's mark names, or nothing when it is not a recorded tree.</summary>
    /// <param name="directory">The directory to read.</param>
    /// <returns>The colour, or nothing.</returns>
    private static Color? TreeColour(string directory)
    {
        var file = Path.Combine(directory, ".rola-tree", "id");
        if (!File.Exists(file))
        {
            return null;
        }

        string text;
        try
        {
            text = File.ReadAllText(file).Trim();
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return null;
        }

        if (!Guid.TryParse(text, out var id))
        {
            return null;
        }

        var hue = Hue(Hashed(id.ToString("N")));

        return Hsl(hue, Saturation, Lightness);
    }

    /// <summary>The colour a name hashes to, which is the colour's hue that is kept.</summary>
    /// <remarks>
    /// FNV-1a over the name's own bytes: it is a hash of a name rather than a digest, and what it is wanted for
    /// is that two names land far apart rather than that a name cannot be found from its colour.
    /// </remarks>
    /// <param name="name">The name.</param>
    /// <returns>The colour the hash names.</returns>
    private static Color Hashed(string name)
    {
        var hash = 2166136261u;

        foreach (var octet in Encoding.UTF8.GetBytes(name))
        {
            unchecked
            {
                hash ^= octet;
                hash *= 16777619u;
            }
        }

        return Color.FromRgb(
            (byte)((hash >> 16) & 0xFF),
            (byte)((hash >> 8) & 0xFF),
            (byte)(hash & 0xFF)
        );
    }

    /// <summary>The hue of a colour, in degrees.</summary>
    /// <param name="colour">The colour.</param>
    /// <returns>Its hue, from 0 to 360.</returns>
    private static double Hue(Color colour)
    {
        var (red, green, blue) = (colour.R / 255.0, colour.G / 255.0, colour.B / 255.0);
        var most = Math.Max(red, Math.Max(green, blue));
        var least = Math.Min(red, Math.Min(green, blue));
        var span = most - least;

        if (span == 0)
        {
            return 0;
        }

        var hue = most == red
            ? 60 * (((green - blue) / span) % 6)
            : most == green
                ? 60 * (((blue - red) / span) + 2)
                : 60 * (((red - green) / span) + 4);

        return hue < 0 ? hue + 360 : hue;
    }

    /// <summary>One colour from a hue, a saturation and a lightness.</summary>
    /// <param name="hue">The hue, in degrees.</param>
    /// <param name="saturation">How far it is from grey.</param>
    /// <param name="lightness">How far it is from black.</param>
    /// <returns>The colour.</returns>
    private static Color Hsl(double hue, double saturation, double lightness)
    {
        var chroma = (1 - Math.Abs((2 * lightness) - 1)) * saturation;
        var second = chroma * (1 - Math.Abs(((hue / 60) % 2) - 1));
        var least = lightness - (chroma / 2);

        var (red, green, blue) = hue switch
        {
            < 60 => (chroma, second, 0.0),
            < 120 => (second, chroma, 0.0),
            < 180 => (0.0, chroma, second),
            < 240 => (0.0, second, chroma),
            < 300 => (second, 0.0, chroma),
            _ => (chroma, 0.0, second),
        };

        return Color.FromRgb(
            (byte)((red + least) * 255),
            (byte)((green + least) * 255),
            (byte)((blue + least) * 255)
        );
    }
}
