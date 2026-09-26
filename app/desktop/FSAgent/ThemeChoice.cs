using System.Text.Json;
using Avalonia.Media;
using Avalonia.Styling;

namespace RorolalaFSAgent;

/// <summary>
/// The two look settings this program reads: the variant it is drawn in and its accent.
/// </summary>
/// <remarks>
/// Read from the file the Desktop keeps them in, so that a window opened by the agent looks like the
/// Desktop that opened it. Only these two things are read, and a file that is missing, unreadable or
/// says something else is not an error: a conflict still has to be answered with whatever look is
/// available, so the defaults are used instead.
/// </remarks>
/// <param name="Variant">The variant to draw in.</param>
/// <param name="Accent">The colour anything accented is drawn in.</param>
internal sealed record ThemeChoice(ThemeVariant Variant, Color Accent)
{
    /// <summary>The accent used when the file names none: the lemon the Desktop was drawn in.</summary>
    private static readonly Color DefaultAccent = Color.FromRgb(0xBF, 0xFF, 0x00);

    /// <summary>Reads the look from the Desktop's data directory, defaulting where it cannot.</summary>
    public static ThemeChoice Load()
    {
        try
        {
            var path = Path.Combine(Root(), "theme.json");

            if (!File.Exists(path))
            {
                return Default();
            }

            using var document = JsonDocument.Parse(File.ReadAllText(path));

            if (document.RootElement.ValueKind != JsonValueKind.Object)
            {
                return Default();
            }

            var root = document.RootElement;
            var choice = Default();

            if (
                root.TryGetProperty("mode", out var mode)
                && mode.ValueKind == JsonValueKind.String
            )
            {
                choice = choice with { Variant = Mode(mode.GetString()!, choice.Variant) };
            }

            if (
                root.TryGetProperty("accent", out var accent)
                && accent.ValueKind == JsonValueKind.String
                && ParseAccent(accent.GetString()!, out var colour)
            )
            {
                choice = choice with { Accent = colour };
            }

            return choice;
        }
        catch (Exception)
        {
            // An unreadable file is no different from no file: the conflict window is still drawn,
            // in the look the program has by default.
            return Default();
        }
    }

    /// <summary>The look used when the file names one, or nothing usable.</summary>
    private static ThemeChoice Default() => new(ThemeVariant.Default, DefaultAccent);

    /// <summary>The variant a name stands for, falling back to the one already chosen.</summary>
    private static ThemeVariant Mode(string name, ThemeVariant fallback) =>
        name.ToLowerInvariant() switch
        {
            "light" => ThemeVariant.Light,
            "dark" => ThemeVariant.Dark,
            "system" => ThemeVariant.Default,
            _ => fallback,
        };

    /// <summary>The colour <c>#RRGGBB</c> stands for.</summary>
    private static bool ParseAccent(string text, out Color colour)
    {
        colour = DefaultAccent;

        if (text.Length != 7 || text[0] != '#' || !IsHex(text.AsSpan(1)))
        {
            return false;
        }

        colour = Color.FromRgb(
            Convert.ToByte(text[1..3], 16),
            Convert.ToByte(text[3..5], 16),
            Convert.ToByte(text[5..7], 16)
        );

        return true;
    }

    /// <summary>Whether every character is a hexadecimal digit.</summary>
    private static bool IsHex(ReadOnlySpan<char> text)
    {
        foreach (var character in text)
        {
            if (!Uri.IsHexDigit(character))
            {
                return false;
            }
        }

        return true;
    }

    /// <summary>
    /// The directory the Desktop keeps its configuration in, resolved the way the Desktop resolves
    /// it so that both read the same file.
    /// </summary>
    private static string Root()
    {
        var data = Environment.GetFolderPath(
            Environment.SpecialFolder.LocalApplicationData,
            Environment.SpecialFolderOption.DoNotVerify
        );

        // A platform that answers with nothing has no data directory; the working directory keeps
        // the program runnable rather than unwritable, as the Desktop does in the same place.
        var root = string.IsNullOrEmpty(data) ? "." : data;

        return Path.Combine(root, "rola", "desktop");
    }
}
