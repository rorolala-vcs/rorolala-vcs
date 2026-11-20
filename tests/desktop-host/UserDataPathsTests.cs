using RolaSharp;
using Rorolala;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// Where a run's user data is, answered once for every program rather than once per program.
/// </summary>
public sealed class UserDataPathsTests
{
    /// <summary>A name is one directory under the base the caller hands in.</summary>
    [Fact]
    public void AnExplicitNameIsOneDirectoryUnderTheBase()
    {
        Assert.Equal(
            Path.Combine("data", "rola", "develop"),
            UserDataPaths.RootIn("data", "develop")
        );
    }

    /// <summary>The keys of a build's name sit under that name.</summary>
    [Fact]
    public void TheKeysSitUnderTheName()
    {
        Assert.Equal(Path.Combine(UserDataPaths.Root, "keys"), UserDataPaths.Keys);
    }

    /// <summary>A name that is not one directory segment is refused rather than joined.</summary>
    [Fact]
    public void AnInvalidNameIsRejected()
    {
        Assert.Throws<ArgumentException>(() => new UserScope("../escape"));
        Assert.Throws<ArgumentException>(() => new UserScope("a/b"));
    }

    /// <summary>The default name is the one the build was given, or the released name.</summary>
    [Fact]
    public void TheDefaultNameIsTheOneTheBuildWasGiven()
    {
        var named = Environment.GetEnvironmentVariable("ROLA_USERDATA_NAME");
        var expected = string.IsNullOrEmpty(named) ? "release" : named;

        Assert.Equal(expected, UserDataPaths.Name);
    }
}
