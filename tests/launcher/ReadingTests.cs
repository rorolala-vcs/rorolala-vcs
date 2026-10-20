using RorolalaDesktop.Theming;
using RorolalaLauncher;
using RorolalaLauncher.Rola;
using RorolalaLauncher.Theming;

namespace RolalalaLauncher.IntegrationTests;

/// <summary>
/// What the launcher reads about the machine and the work: an address, a language, a directory.
/// </summary>
/// <remarks>
/// Each of these is a reading the program makes before it can do anything, and each is a reading that the
/// command line makes too: what is checked is that the launcher reads what the command line would read, so
/// that a window accepting something is a window whose command accepts it.
/// </remarks>
public sealed class ReadingTests
{
    /// <summary>An address is read the way the protocol reads one.</summary>
    /// <param name="written">What is written.</param>
    [Theory]
    [InlineData("127.0.0.1")]
    [InlineData("localhost")]
    [InlineData("example.com")]
    [InlineData("7717")]
    [InlineData("127.0.0.1:7717")]
    [InlineData("localhost:7717")]
    [InlineData("[::1]:7717")]
    [InlineData("rola://10.0.0.1/ns")]
    [InlineData("rola://host/ns/sub")]
    [InlineData("host/ns/sub")]
    [InlineData("host:7717/ns")]
    public void AnAddressIsReadAsOne(string written) => Assert.True(VaultAddressSyntax.Valid(written));

    /// <summary>And what does not name a daemon is not one.</summary>
    /// <param name="written">What is written.</param>
    [Theory]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData("not a host")]
    [InlineData("host:portx")]
    [InlineData("99999")]
    [InlineData("a:b:c")]
    [InlineData("rola://")]
    [InlineData("[::1]")]
    [InlineData("host name")]
    public void AnotherThingIsNotAnAddress(string written) =>
        Assert.False(VaultAddressSyntax.Valid(written));

    /// <summary>The language a run speaks is what it was told, then what was chosen, then the machine's.</summary>
    [Fact]
    public void ALanguageIsChosen()
    {
        Assert.Equal("zh-CN", LauncherLanguage.Choose("zh_CN.UTF-8", "en", "en"));
        Assert.Equal("en", LauncherLanguage.Choose(null, "en", "zh-CN"));
        Assert.Equal("zh-CN", LauncherLanguage.Choose(null, null, "zh-CN"));
        Assert.Equal("en", LauncherLanguage.Choose(null, null, "en-GB"));
        Assert.Equal("en", LauncherLanguage.Choose(null, null, null));
    }

    /// <summary>A directory is a Workspace when it holds one, and says which of the two it is otherwise.</summary>
    [Fact]
    public void ADirectoryIsReadAsWhatItIs()
    {
        var directory = Scratch.New("state");

        Assert.Equal(WorkspaceState.Missing, Workspaces.Of(Path.Combine(directory, "gone")));
        Assert.Equal(WorkspaceState.NotWorkspace, Workspaces.Of(directory));

        Directory.CreateDirectory(Path.Combine(directory, Workspaces.DataDirectory));

        Assert.Equal(WorkspaceState.Workspace, Workspaces.Of(directory));
    }

    /// <summary>A Workspace is called what its directory is called, however the path is written.</summary>
    [Fact]
    public void AWorkspaceIsNamedForItsDirectory()
    {
        Assert.Equal("work", Workspaces.Named("/home/one/work"));
        Assert.Equal("work", Workspaces.Named("/home/one/work/"));
        Assert.Equal("work", Workspaces.Named("C:\\work"));
    }

    /// <summary>How a style is drawn is a fact about the colours, not a guess.</summary>
    [Fact]
    public void AnInkIsReadOnItsFill()
    {
        Assert.True(Skin.IsLight(Avalonia.Media.Colors.White));
        Assert.False(Skin.IsLight(Avalonia.Media.Colors.Black));
        Assert.Equal(Avalonia.Media.Colors.Black, Skin.Readable(Avalonia.Media.Colors.White));
        Assert.Equal(Avalonia.Media.Colors.White, Skin.Readable(Avalonia.Media.Colors.Black));
    }

    /// <summary>A style is handed to the command line under the name a run writes.</summary>
    [Fact]
    public void AStyleIsWrittenAsARunWritesIt()
    {
        Assert.Equal("windows", RolaRunner.Written(WindowStyle.Windows));
        Assert.Equal("mac", RolaRunner.Written(WindowStyle.Mac));
        Assert.Equal("tillingwm", RolaRunner.Written(WindowStyle.TillingWm));
    }
}
