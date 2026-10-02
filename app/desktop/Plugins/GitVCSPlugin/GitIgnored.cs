using System.Diagnostics;
using System.Text;
using FileSystemPlugin;
using RorolalaDesktop.Contract;

namespace GitVCSPlugin;

/// <summary>
/// Hides what Git ignores: the entries a repository's <c>.gitignore</c> rules leave out.
/// </summary>
/// <remarks>
/// The rules are Git's to read and are not reimplemented here: <c>.gitignore</c> is a small language with
/// a history of its own (negations, directory-only rules, the repository's own exclusion file, the
/// global one), and a second implementation would be a second answer to what Git does. So the answer is
/// asked of the <c>git</c> program, once per directory rather than once per entry — the listing stages
/// every entry of a directory together, and a process per entry would be a listing that took a second
/// per file.
/// </remarks>
public sealed class GitIgnored : IEntryHideProvider
{
    /// <summary>What this provider is switched by, which is written into the user's preference file.</summary>
    public const string Value = "git_ignored";

    /// <summary>
    /// The label is stated by this plugin rather than by the File System plugin, though its key says
    /// otherwise.
    /// </summary>
    /// <remarks>
    /// The key is namespaced as the File System plugin's because it names an option of that plugin's
    /// setting, which is the only setting a hide provider can be shown under. A key is what a call site
    /// names and any registered file may state it, so the option is offered without the File System plugin
    /// having to know that Git exists.
    /// </remarks>
    public const string Label = "rorolala_file_system.hides.git_ignored";

    /// <inheritdoc />
    public string Id => Value;

    /// <inheritdoc />
    public string LabelKey => Label;

    /// <inheritdoc />
    public bool Hides(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return GitIgnores.Hides(entry.Path);
    }
}

/// <summary>
/// What Git ignores, asked of the program and remembered per directory.
/// </summary>
/// <remarks>
/// The answers are kept by directory because what a listing stages is one directory's entries: the
/// directory a user is looking at is asked about once, however many of its entries are drawn, and moving
/// away and back is a second ask rather than a second process per entry. The cache lives for the run
/// because a rule changed in a file is a change the next run picks up — this is a browser, not a monitor
/// of the repository, and asking again on every frame would be a process per frame.
/// </remarks>
public static class GitIgnores
{
    /// <summary>How long the program is given before it is treated as having nothing to say.</summary>
    /// <remarks>
    /// A directory being listed is on screen and waits for this, so a repository that makes the program
    /// hang — a filesystem that stopped answering, a rule that asks about a network share — must not hang
    /// the browser with it. Nothing ignored is the answer that shows the entries.
    /// </remarks>
    private static readonly TimeSpan Patience = TimeSpan.FromSeconds(5);

    /// <summary>The ignored paths of each directory already asked about.</summary>
    private static readonly Dictionary<string, HashSet<string>> Asked = new(StringComparer.Ordinal);

    /// <summary>Whether a path is one Git ignores.</summary>
    /// <param name="path">The path to ask about, which may be a file or a directory.</param>
    /// <returns>Whether Git ignores it.</returns>
    public static bool Hides(string path)
    {
        // A directory's own name is ignored or not, and asking Git about the directory asks that. A file
        // is asked about along with the rest of the directory holding it.
        var directory = Directory.Exists(path) ? path : Path.GetDirectoryName(path);

        if (directory is not { Length: > 0 })
        {
            return false;
        }

        if (!Asked.TryGetValue(directory, out var ignored))
        {
            ignored = Read(directory);
            Asked[directory] = ignored;
        }

        return ignored.Contains(path);
    }

    /// <summary>Asks Git what of a directory it ignores.</summary>
    /// <remarks>
    /// The directory itself is asked about along with what it holds, because the answer about a directory is
    /// not derivable from the answers about its entries: an empty directory can be ignored by a rule naming
    /// it, and a directory the program is run in is not an entry of itself.
    /// </remarks>
    /// <param name="directory">The directory to ask about.</param>
    /// <returns>The paths Git named, which may be none.</returns>
    private static HashSet<string> Read(string directory)
    {
        var ignored = new HashSet<string>(StringComparer.Ordinal);
        var root = Repository(directory);

        if (root is null)
        {
            return ignored;
        }

        var entries = Entries(directory);

        if (entries.Count == 0)
        {
            entries.Add(directory);
        }

        try
        {
            var asking = new ProcessStartInfo("git")
            {
                WorkingDirectory = root,
                RedirectStandardInput = true,
                RedirectStandardOutput = true,
                UseShellExecute = false,
                StandardOutputEncoding = Encoding.UTF8,
            };

            asking.ArgumentList.Add("check-ignore");
            asking.ArgumentList.Add("--stdin");

            using var git = Process.Start(asking);

            if (git is null)
            {
                return ignored;
            }

            foreach (var entry in entries)
            {
                git.StandardInput.WriteLine(Path.GetRelativePath(root, entry));
            }

            git.StandardInput.Close();

            // Read before waiting rather than after: a program that filled the pipe would be waiting for
            // this side to read it while this side waited for the program to finish.
            var said = git.StandardOutput.ReadToEnd();

            if (!git.WaitForExit((int)Patience.TotalMilliseconds))
            {
                // WORKAROUND: the program is still holding the directory, so it is killed rather than waited
                // on: it has not answered, and an answer half-read is worse than none.
                git.Kill(entireProcessTree: true);

                return ignored;
            }

            // A program that failed answers nothing: what it said first is a partial list, and hiding by that
            // would hide by rules nobody stated. A repository the program cannot read is one whose whole
            // listing is shown, which is the same answer as no program at all.
            if (git.ExitCode != 0)
            {
                return ignored;
            }

            foreach (var line in said.Split('\n', StringSplitOptions.RemoveEmptyEntries))
            {
                // An answer is the path it was asked about, relative to the repository root — which is where
                // it was written from and what it is read back against.
                ignored.Add(Path.GetFullPath(Path.Combine(root, line.Trim())));
            }
        }
        catch (Exception error)
            when (error is System.ComponentModel.Win32Exception or InvalidOperationException or IOException)
        {
            // No `git` installed is not a reason to fail a listing: nothing is hidden by a program that is
            // not there, which is the same answer as a directory no repository holds.
            return ignored;
        }

        return ignored;
    }

    /// <summary>
    /// The repository a directory belongs to, or nothing when it belongs to none.
    /// </summary>
    /// <remarks>
    /// Walking up from the directory rather than running <c>git rev-parse</c>: this is asked of every
    /// directory a user opens, including ones on the far side of a mount, and a file check the platform
    /// answers immediately is cheaper than a process. The step that finds a repository is the directory that
    /// holds one, which is where the program is run from, so what it says is relative to a known place.
    /// </remarks>
    /// <param name="directory">The directory to start from.</param>
    /// <returns>The repository's own directory, or nothing.</returns>
    private static string? Repository(string directory)
    {
        var at = new DirectoryInfo(directory);

        while (at is not null)
        {
            if (Directory.Exists(Path.Combine(at.FullName, ".git")) || File.Exists(Path.Combine(at.FullName, ".git")))
            {
                return at.FullName;
            }

            at = at.Parent;
        }

        return null;
    }

    /// <summary>What a directory holds, which is nothing when it cannot be read.</summary>
    /// <param name="directory">The directory to read.</param>
    private static List<string> Entries(string directory)
    {
        try
        {
            return [.. Directory.EnumerateFileSystemEntries(directory)];
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return [];
        }
    }
}
