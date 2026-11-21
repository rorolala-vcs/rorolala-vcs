using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What a content fetch is asked to do: the command it runs, and when its two items may be pressed.
/// </summary>
public sealed class BlobFetchTests
{
    /// <summary>
    /// A fetch names the theme every run of the window's is asked for, then the node's variant as the
    /// word of `--with`, and `--recursive` when the versions it stands on come with it.
    /// </summary>
    [Fact]
    public void AFetchNamesTheWordAndTheDepth()
    {
        Assert.Equal(
            new[] { "--theme-choice=desktop-friendly", "fetch", "--with", "abc" },
            BlobFetch.Arguments("abc", recursive: false)
        );
        Assert.Equal(
            new[] { "--theme-choice=desktop-friendly", "fetch", "--recursive", "--with", "abc" },
            BlobFetch.Arguments("abc", recursive: true)
        );
    }

    /// <summary>
    /// A recursive fetch of a node carries something when the node or any version below it is missing
    /// content, and nothing when the whole chain is here.
    /// </summary>
    [Fact]
    public void AChainIsMissingWhenAnyVersionOfItIs()
    {
        var chain = new Dictionary<string, (string? Base, bool Offline)>(StringComparer.Ordinal)
        {
            ["top"] = ("mid", true),
            ["mid"] = ("old", false),
            ["old"] = (null, true),
        };

        Assert.True(BlobFetch.AnyMissing(chain, "top"));

        var here = new Dictionary<string, (string? Base, bool Offline)>(StringComparer.Ordinal)
        {
            ["top"] = ("old", true),
            ["old"] = (null, true),
        };

        Assert.False(BlobFetch.AnyMissing(here, "top"));
    }
}
