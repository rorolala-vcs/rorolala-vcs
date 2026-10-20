using RorolalaDesktop.Theming;
using RorolalaLauncher;

namespace RolalalaLauncher.IntegrationTests;

/// <summary>
/// What the launcher keeps of its own: the Workspaces it has opened, and what it was told about its frame.
/// </summary>
/// <remarks>
/// Both files are written by this program and read back by it, which is the whole of what they are for: a
/// file that this program writes and cannot read again is a list of Workspaces that is lost every time the
/// program is closed.
/// </remarks>
public sealed class LauncherFilesTests
{
    /// <summary>A list of Workspaces survives being written and read.</summary>
    [Fact]
    public void TheListOfWorkspacesSurvivesARoundTrip()
    {
        var directory = Scratch.New("workspaces");
        var path = Path.Combine(directory, "workspaces.toml");
        var file = new WorkspacesFile();

        file.Add(Path.Combine(directory, "one"));
        file.Add(Path.Combine(directory, "two with a space"));
        file.Write(path);

        var read = WorkspacesFile.Read(path);

        Assert.Equal(file.Paths, read.Paths);
    }

    /// <summary>The file is written as a person reads it, one path a line, and is read back the same way.</summary>
    [Fact]
    public void TheFileIsWrittenTheWayThePlanStatesIt()
    {
        var directory = Scratch.New("workspaces-shape");
        var path = Path.Combine(directory, "workspaces.toml");
        var file = new WorkspacesFile();

        file.Add(Path.Combine(directory, "one"));
        file.Write(path);

        var text = File.ReadAllText(path);

        Assert.StartsWith("workspaces = [\n", text, StringComparison.Ordinal);
        Assert.EndsWith("]\n", text, StringComparison.Ordinal);
        Assert.Contains("\t\"", text, StringComparison.Ordinal);
    }

    /// <summary>A directory named twice, in two spellings, is one entry.</summary>
    [Fact]
    public void AWorkspaceIsOnTheListOnce()
    {
        var directory = Scratch.New("workspaces-once");
        var file = new WorkspacesFile();

        Assert.True(file.Add(directory));
        Assert.False(file.Add(directory + Path.DirectorySeparatorChar));
        Assert.False(file.Add(directory + Path.DirectorySeparatorChar + "."));

        Assert.Single(file.Paths);
    }

    /// <summary>A file a person wrote — with a comment, and the list on one line — is read.</summary>
    [Fact]
    public void AHandWrittenFileIsRead()
    {
        var directory = Scratch.New("workspaces-hand");
        var path = Path.Combine(directory, "workspaces.toml");

        File.WriteAllText(
            path,
            "# Where the work is\nworkspaces = [ \"/home/one\", '/home/two' ] # the list\n"
        );

        var read = WorkspacesFile.Read(path);

        Assert.Equal(["/home/one", "/home/two"], read.Paths);
    }

    /// <summary>A file that is not there is an empty list.</summary>
    [Fact]
    public void NoFileIsNoList()
    {
        var directory = Scratch.New("workspaces-none");

        Assert.Empty(WorkspacesFile.Read(Path.Combine(directory, "workspaces.toml")).Paths);
    }

    /// <summary>A comment between the entries, and a bracket inside one, do not end the list.</summary>
    [Fact]
    public void TheListEndsWhereTheFileSaysItDoes()
    {
        var directory = Scratch.New("workspaces-brackets");
        var path = Path.Combine(directory, "workspaces.toml");

        File.WriteAllText(
            path,
            "workspaces = [\n\t\"/home/one[0]\", # the first\n\t\"/home/two\",\n]\n# \"not a path\"\n"
        );

        Assert.Equal(["/home/one[0]", "/home/two"], WorkspacesFile.Read(path).Paths);
    }

    /// <summary>What this program was told about its frame survives being written and read.</summary>
    [Fact]
    public void TheFrameSurvivesARoundTrip()
    {
        var directory = Scratch.New("settings");
        var path = Path.Combine(directory, "settings.toml");

        foreach (var style in new[] { WindowStyle.Windows, WindowStyle.Mac, WindowStyle.TillingWm })
        {
            new LauncherSettings { Frame = style }.Write(path);

            Assert.Equal(style, LauncherSettings.Read(path).Frame);
        }
    }

    /// <summary>A frame that was not chosen is the absence of the key, and of the file.</summary>
    [Fact]
    public void NoChoiceIsNoFile()
    {
        var directory = Scratch.New("settings-none");
        var path = Path.Combine(directory, "settings.toml");

        new LauncherSettings { Frame = WindowStyle.Mac }.Write(path);

        Assert.True(File.Exists(path));

        new LauncherSettings().Write(path);

        Assert.False(File.Exists(path));
        Assert.Null(LauncherSettings.Read(path).Frame);
    }

    /// <summary>A style this program does not know is not a choice it can keep.</summary>
    [Fact]
    public void AnotherStyleIsNoChoice()
    {
        var directory = Scratch.New("settings-other");
        var path = Path.Combine(directory, "settings.toml");

        File.WriteAllText(path, "windowStyle = \"gnome\"\n");

        Assert.Null(LauncherSettings.Read(path).Frame);
    }

    /// <summary>A path is written as one string, whatever it carries.</summary>
    [Fact]
    public void APathIsWrittenAsOneString()
    {
        var text = TomlLine.Quote("C:\\work\\one \"two\"");

        Assert.Equal("C:\\work\\one \"two\"", TomlLine.String(text));
    }

    /// <summary>A comment after a value is not part of it, and a hash inside one is.</summary>
    [Fact]
    public void ACommentIsNotTheValue()
    {
        var (key, value) = TomlLine.Split("windowStyle = \"a # b\" # what was chosen");

        Assert.Equal("windowStyle", key);
        Assert.Equal("a # b", TomlLine.String(value!));
    }
}
