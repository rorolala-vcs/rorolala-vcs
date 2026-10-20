namespace RorolalaLauncher;

/// <summary>
/// The lines the launcher says about itself, read from beside the program.
/// </summary>
/// <remarks>
/// The lines are a file rather than a table in the program because they are copy rather than behaviour: a
/// sentence written by someone who writes sentences belongs in a file that person can edit, and a build
/// carries it beside the program the way it carries the banners.
/// <para>
/// One line is one line: a blank line is not one, and a line longer than the room the window has is cut
/// where the room ends rather than wrapped onto a second line, because a second line would move everything
/// under it every time a longer line was drawn.
/// </para>
/// </remarks>
internal static class Slogans
{
    /// <summary>The most of a line the window has room for, which is where a longer one is cut.</summary>
    private const int Most = 96;

    /// <summary>
    /// The lines a file states.
    /// </summary>
    /// <param name="path">The file.</param>
    /// <returns>Its lines, cut to the room the window has.</returns>
    public static IReadOnlyList<string> Read(string path)
    {
        if (!File.Exists(path))
        {
            return [];
        }

        string[] lines;

        try
        {
            lines = File.ReadAllLines(path);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return [];
        }

        var said = new List<string>(lines.Length);

        foreach (var line in lines)
        {
            var text = line.Trim();

            if (text.Length == 0)
            {
                continue;
            }

            said.Add(text.Length <= Most ? text : $"{text[..Most].TrimEnd()}…");
        }

        return said;
    }
}
