using RorolalaDesktop.Theming;

namespace RorolalaLauncher;

/// <summary>
/// What the user chose about how this program is drawn, kept in <c>settings.toml</c>.
/// </summary>
/// <remarks>
/// The day/night choice and the colours are not here: those are the Desktop program's own file, and the
/// two programs read and write the same one. What is here is the one choice only this program makes —
/// how the window's own frame is drawn — and what this program does when the user has not made it.
/// <para>
/// A style that is not written is the machine's own answer, which is the answer a run asks the command
/// line for (see <see cref="Rola.RolaRunner.ProbeWindowStyle"/>). Writing one down is the user's choice,
/// and it is what this program hands to the Desktop program when it starts it.
/// </para>
/// </remarks>
internal sealed class LauncherSettings
{
    /// <summary>The key a window style is written under.</summary>
    private const string FrameKey = "windowStyle";

    /// <summary>The style chosen, or nothing when the user has chosen none.</summary>
    public WindowStyle? Frame { get; set; }

    /// <summary>
    /// Reads the settings a file states, or the empty settings when there is no file.
    /// </summary>
    /// <remarks>
    /// A file this program cannot read is answered as no choice rather than as a failure: the choice is
    /// cosmetic, and a program that refused to open over one would be worse than one drawn the machine's
    /// way.
    /// </remarks>
    /// <param name="path">The file.</param>
    /// <returns>What it states.</returns>
    public static LauncherSettings Read(string path)
    {
        var settings = new LauncherSettings();

        if (!File.Exists(path))
        {
            return settings;
        }

        string text;

        try
        {
            text = File.ReadAllText(path);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return settings;
        }

        foreach (var line in text.Split('\n'))
        {
            var (key, value) = TomlLine.Split(line);

            if (key != FrameKey || value is null)
            {
                continue;
            }

            if (WindowStyles.Named(TomlLine.String(value)) is { } style)
            {
                settings.Frame = style;
            }
        }

        return settings;
    }

    /// <summary>
    /// Writes the settings, or takes the file away when nothing is chosen.
    /// </summary>
    /// <remarks>
    /// A choice taken back is the absence of a key, and a file with no key in it is the file the program
    /// reads as "the machine decides": writing the machine's answer down instead would be a choice the
    /// user did not make, kept forever.
    /// </remarks>
    /// <param name="path">The file.</param>
    /// <exception cref="IOException">The file could not be written.</exception>
    public void Write(string path)
    {
        if (Frame is not { } style)
        {
            if (File.Exists(path))
            {
                File.Delete(path);
            }

            return;
        }

        var directory = Path.GetDirectoryName(path);

        if (!string.IsNullOrEmpty(directory))
        {
            Directory.CreateDirectory(directory);
        }

        File.WriteAllText(path, $"{FrameKey} = {TomlLine.Quote(style.ToString())}\n");
    }
}
