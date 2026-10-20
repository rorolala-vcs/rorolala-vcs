namespace RolalalaLauncher.IntegrationTests;

/// <summary>
/// Scratch directories for one run.
/// </summary>
/// <remarks>
/// They sit under the suite's own directory rather than the system's temporary one, because what they hold
/// is a launcher's files and a file system that is not the suite's own is a file system with somebody
/// else's leftovers in it — and because a run leaves nothing behind where it did not have to write.
/// </remarks>
internal static class Scratch
{
    /// <summary>A fresh directory, under this suite's own.</summary>
    /// <param name="name">What it is for, for a person reading the path.</param>
    /// <returns>The directory, which exists and holds nothing.</returns>
    public static string New(string name)
    {
        var directory = Path.Combine(
            AppContext.BaseDirectory,
            "__scratch",
            name,
            Guid.NewGuid().ToString("N")
        );

        Directory.CreateDirectory(directory);

        return directory;
    }

    /// <summary>A file holding bytes, under a directory of this run's.</summary>
    /// <param name="name">What the directory is for.</param>
    /// <param name="file">The file's name.</param>
    /// <param name="content">What it holds.</param>
    /// <returns>The file's path.</returns>
    public static string Write(string name, string file, byte[] content)
    {
        var directory = New(name);
        var path = Path.Combine(directory, file);

        File.WriteAllBytes(path, content);

        return path;
    }
}
