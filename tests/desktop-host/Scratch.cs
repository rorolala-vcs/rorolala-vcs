namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>Scratch directories for one run, under the system temporary directory.</summary>
internal static class Scratch
{
    /// <summary>Where every scratch directory of this run sits.</summary>
    private static string Root { get; } = Path.Combine(
        Path.GetTempPath(),
        $"rorolala-scratch-{Guid.NewGuid():N}"
    );

    /// <summary>A fresh, empty directory under the run's scratch root.</summary>
    /// <param name="name">What the directory is for, for a person reading the path.</param>
    /// <returns>The directory, which exists and holds nothing.</returns>
    public static string New(string name)
    {
        var directory = Path.Combine(Root, name, Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(directory);

        return directory;
    }

    /// <summary>A directory holding one translation file.</summary>
    /// <param name="file">The file's name, without the extension.</param>
    /// <param name="content">What the file states, as the reader reads it.</param>
    /// <returns>The directory the file sits in.</returns>
    public static string Translations(string file, string content)
    {
        var directory = New("i18n");
        File.WriteAllText(Path.Combine(directory, $"{file}.yml"), content);

        return directory;
    }
}
