using RorolalaDesktop.Configuration;

namespace RorolalaLauncher;

/// <summary>
/// Where this program keeps its own files, and where the programs it starts from are.
/// </summary>
/// <remarks>
/// Everything is named relative to one of two places and nothing is read from the environment, because
/// an environment is the machine's and not this program's: what sits beside the program is where an
/// export lays it out, and what it keeps of its own is under the same data directory the Desktop program
/// keeps its files under, in a directory of its own.
/// <para>
/// The two roots are given rather than worked out here, so that a test can point them at scratch ones.
/// </para>
/// </remarks>
/// <param name="Root">The directory the launcher keeps its own files under.</param>
/// <param name="Beside">The directory the program itself sits in, which the programs it starts sit beside.</param>
internal sealed record LauncherFiles(string Root, string Beside)
{
    /// <summary>The launcher's own directory for this machine.</summary>
    private const string Own = "launcher";

    /// <summary>The directory a run on this machine keeps its files under.</summary>
    public static string Home => Path.Combine(ConfigPaths.Root, Own);

    /// <summary>Where this run's files are.</summary>
    public static LauncherFiles ForThisRun() => new(Home, AppContext.BaseDirectory);

    /// <summary>The list of Workspaces this machine has opened, in the order it opened them.</summary>
    public string Workspaces => Path.Combine(Root, "workspaces.toml");

    /// <summary>What the user chose about this program, as opposed to what the machine is drawn in.</summary>
    public string Settings => Path.Combine(Root, "settings.toml");

    /// <summary>The picture the banner plays, which is the user's own once it is there.</summary>
    public string Banner => Path.Combine(Root, "banner.aseprite");

    /// <summary>
    /// The Workspace a connection is checked from.
    /// </summary>
    /// <remarks>
    /// The command line speaks a handshake from inside a Workspace, and a creation is checked before
    /// there is one, so a Workspace of this program's own is what the check is made from. It holds no
    /// work and nothing in it is ever read; it is a place to stand rather than a place to keep.
    /// </remarks>
    public string Probe => Path.Combine(Root, "probe");

    /// <summary>The pictures an aseprite replacement may be chosen from, beside the program.</summary>
    public string Banners => Path.Combine(Beside, "launcher-banners");

    /// <summary>The command line program, beside this one in a published layout.</summary>
    public string Rola => Path.Combine(Beside, "..", "rola" + Suffix);

    /// <summary>The Desktop program, in the same directory as this one.</summary>
    public string Desktop => Path.Combine(Beside, "RorolalaDesktop" + Suffix);

    /// <summary>
    /// The lines the launcher says about itself, beside the program.
    /// </summary>
    /// <remarks>
    /// Beside the program rather than under the user's data directory: they are the release's own copy, not
    /// a choice of the person reading it, and a build carries them the way it carries the banners.
    /// </remarks>
    public string Slogans => Path.Combine(Beside, "slogan.txt");

    /// <summary>What a platform puts after a program's name, if anything.</summary>
    private static string Suffix => OperatingSystem.IsWindows() ? ".exe" : string.Empty;

    /// <summary>Makes the directory this program keeps its files in.</summary>
    /// <returns>The same files, with the directory in place.</returns>
    /// <exception cref="IOException">The directory could not be made.</exception>
    public LauncherFiles Made()
    {
        Directory.CreateDirectory(Root);

        return this;
    }
}
