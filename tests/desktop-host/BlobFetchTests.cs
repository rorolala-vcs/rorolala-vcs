using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What a content fetch is asked to do: the command it runs, and which versions of a lane it names.
/// </summary>
public sealed class BlobFetchTests
{
    /// <summary>
    /// A fetch names the store's sync-hashes command and then the keys, after the theme every run of
    /// the window's is asked for.
    /// </summary>
    [Fact]
    public void AFetchNamesTheStoreAndItsKeys()
    {
        Assert.Equal(
            new[]
            {
                "--theme-choice=desktop-friendly",
                "storage",
                "sync-hashes",
                "one",
                "two",
            },
            BlobFetch.Arguments(["one", "two"])
        );
    }

    /// <summary>
    /// A lane is one file's chain, so only its versions come, only the ones whose content is not all
    /// here, and each key once.
    /// </summary>
    [Fact]
    public void OnlyTheVersionsOfTheLaneThatAreNotAllHereCome()
    {
        (int Lane, string Storage, bool Offline)[] versions =
        [
            (0, "here", true),
            (0, "missing", false),
            (1, "elsewhere", false),
            (0, "missing", false),
            (0, string.Empty, false),
        ];

        Assert.Equal(new[] { "missing" }, BlobFetch.MissingInLane(versions, 0));
    }
}
