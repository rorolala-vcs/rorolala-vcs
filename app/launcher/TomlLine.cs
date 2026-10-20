using System.Text.Json;

namespace RorolalaLauncher;

/// <summary>
/// The little of TOML this program reads and writes, spread over lines.
/// </summary>
/// <remarks>
/// The files this program keeps are two keys wide — a list of paths and one style name — so they are read
/// a line at a time rather than by a parser of the whole language. What is written is written by
/// <see cref="Quote"/>, which asks the JSON serializer for the spelling, since a JSON string literal is
/// also a TOML basic string: one escape rule serves both, and no character a path may hold is lost.
/// </remarks>
internal static class TomlLine
{
    /// <summary>
    /// Splits a line into the key it states and the text of the value, without that text's comment.
    /// </summary>
    /// <param name="line">The line.</param>
    /// <returns>The key and the value's text, or nothing where the line states neither.</returns>
    public static (string? Key, string? Value) Split(string line)
    {
        var text = WithoutComment(line);
        var assign = OutsideQuotes(text, '=');

        if (assign < 0)
        {
            return (null, null);
        }

        var key = text[..assign].Trim().Trim('"', '\'');
        var value = text[(assign + 1)..].Trim();

        return key.Length == 0 || value.Length == 0 ? (null, null) : (key, value);
    }

    /// <summary>
    /// The text a written string stands for, or the text itself where it was written as one.
    /// </summary>
    /// <param name="value">What a line states.</param>
    /// <returns>The string it states.</returns>
    public static string String(string value)
    {
        var text = value.Trim();

        if (text.Length >= 2 && text[0] == '\'' && text[^1] == '\'')
        {
            // A literal string is taken as written: TOML has no escapes in one, which is what makes it
            // the spelling a path is least likely to be mangled by.
            return text[1..^1];
        }

        if (text.Length >= 2 && text[0] == '"' && text[^1] == '"')
        {
            try
            {
                return JsonSerializer.Deserialize<string>(text) ?? text;
            }
            catch (JsonException)
            {
                return text[1..^1];
            }
        }

        return text;
    }

    /// <summary>The text a string is written as, which is one TOML basic string.</summary>
    /// <param name="text">The string.</param>
    /// <returns>The quoted text.</returns>
    public static string Quote(string text) => JsonSerializer.Serialize(text);

    /// <summary>
    /// The line without the comment it carries, if any.
    /// </summary>
    /// <remarks>
    /// A `#` inside a quoted string is part of the string, so only one outside quotes begins a comment.
    /// </remarks>
    /// <param name="line">The line.</param>
    /// <returns>What is left of it.</returns>
    private static string WithoutComment(string line)
    {
        var comment = OutsideQuotes(line, '#');

        return comment < 0 ? line : line[..comment];
    }

    /// <summary>The first place a character stands outside any quoted string, or -1.</summary>
    /// <param name="text">The text.</param>
    /// <param name="wanted">The character.</param>
    /// <returns>Where it stands, or -1.</returns>
    private static int OutsideQuotes(string text, char wanted)
    {
        var quote = '\0';

        for (var at = 0; at < text.Length; at++)
        {
            var character = text[at];

            if (quote != '\0')
            {
                if (character == '\\' && quote == '"')
                {
                    at++;
                }
                else if (character == quote)
                {
                    quote = '\0';
                }

                continue;
            }

            if (character is '"' or '\'')
            {
                quote = character;
                continue;
            }

            if (character == wanted)
            {
                return at;
            }
        }

        return -1;
    }
}
