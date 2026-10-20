using System.Text;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;

namespace RorolalaLauncher.Controls;

/// <summary>
/// The mark a Workspace wears on the list: its first letter on a colour of its own.
/// </summary>
/// <remarks>
/// The colour is the directory's own rather than the look's: a hash of the whole path is read as a colour,
/// its hue is taken, and that hue is drawn twice at two lightnesses — so a Workspace looks the same on every
/// machine it is listed on, and two Workspaces are told apart by colour before either name is read. The two
/// are laid as a gradient from the top-left of the mark to its bottom-right, which is what stops it reading
/// as a flat rectangle of some colour.
/// <para>
/// What is written on it is the directory's own first character: a Latin letter is uppercased and a Chinese
/// one is kept as it is, because the two are not the same thing and uppercasing the wrong one is how a mark
/// comes out with nothing on it.
/// </para>
/// </remarks>
internal sealed class WorkspaceBadge : Border
{
    /// <summary>How wide and tall the mark is.</summary>
    private const double Extent = 40;

    /// <summary>How far the corner is rounded, which is what makes the mark a mark rather than a swatch.</summary>
    private const double Rounding = 10;

    /// <summary>How far the colour of a mark is pulled towards grey, so that no two of them shout.</summary>
    private const double Saturation = 0.55;

    /// <summary>How light the top-left end of the mark is.</summary>
    private const double Lit = 0.52;

    /// <summary>How light the bottom-right end of it is.</summary>
    private const double Dim = 0.32;

    /// <summary>Makes the mark for one Workspace.</summary>
    /// <param name="path">The Workspace's whole directory, which the colour is read from.</param>
    public WorkspaceBadge(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        var hue = Hue(Hashed(path));
        var (lit, dim) = (Hsl(hue, Saturation, Lit), Hsl(hue, Saturation, Dim));

        Width = Extent;
        Height = Extent;
        CornerRadius = new CornerRadius(Rounding);
        Margin = new Thickness(0, 0, 12, 0);
        VerticalAlignment = VerticalAlignment.Center;
        Background = new LinearGradientBrush
        {
            StartPoint = new RelativePoint(0, 0, RelativeUnit.Relative),
            EndPoint = new RelativePoint(1, 1, RelativeUnit.Relative),
            GradientStops =
            {
                new GradientStop(lit, 0),
                new GradientStop(dim, 1),
            },
        };

        Child = new TextBlock
        {
            Text = Initial(Workspaces.Named(path)),
            FontSize = 17,
            FontWeight = FontWeight.SemiBold,
            Foreground = new SolidColorBrush(Colors.White),
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };
    }

    /// <summary>
    /// The colour a path hashes to, which is the colour's hue that is kept.
    /// </summary>
    /// <remarks>
    /// FNV-1a over the path's own bytes: it is a hash of a name rather than a digest, and what it is wanted
    /// for is that two paths land far apart rather than that a path cannot be found from its colour.
    /// </remarks>
    /// <param name="path">The path.</param>
    /// <returns>The colour the hash names.</returns>
    private static Color Hashed(string path)
    {
        var hash = 2166136261u;

        foreach (var octet in Encoding.UTF8.GetBytes(path))
        {
            hash ^= octet;
            hash *= 16777619u;
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

    /// <summary>
    /// What the mark is written with: the name's own first character, or one past the first rune where that
    /// rune is only half of one.
    /// </summary>
    /// <param name="name">The name.</param>
    /// <returns>The character, or a question mark where the name has none.</returns>
    private static string Initial(string name)
    {
        foreach (var rune in name.EnumerateRunes())
        {
            return Rune.ToUpperInvariant(rune).ToString();
        }

        return "?";
    }
}
