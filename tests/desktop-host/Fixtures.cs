namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// Where the fixture plugin assemblies are, and a place to lay a few of them out.
/// </summary>
/// <remarks>
/// The fixtures are copied beside this suite by its own build, under <c>plugins/</c>. A host is
/// pointed at a directory holding only the fixtures a test is about, so a test reads the plugins it
/// named and nothing else.
/// </remarks>
internal static class Fixtures
{
    /// <summary>Where every fixture assembly was laid out beside this suite.</summary>
    private static string Source { get; } = Path.Combine(AppContext.BaseDirectory, "plugins");

    /// <summary>
    /// A directory holding only the named fixtures.
    /// </summary>
    /// <param name="names">The fixture assembly names, without the extension.</param>
    /// <returns>The directory the copies sit in.</returns>
    /// <exception cref="FileNotFoundException">A named fixture was not built.</exception>
    public static string Only(params string[] names)
    {
        var directory = Scratch.New("plugins");

        foreach (var name in names)
        {
            File.Copy(Path.Combine(Source, $"{name}.dll"), Path.Combine(directory, $"{name}.dll"));
        }

        return directory;
    }
}
