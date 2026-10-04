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

    /// <inheritdoc />
    public bool Hides(Entry entry, string root)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return GitIgnores.Hides(entry.Path, root);
    }
}

/// <summary>
/// What Git ignores, asked of the program and remembered per directory.
/// </summary>
/// <remarks>
/// The answers are kept by the directory asked about and the root it was asked from, because what a
/// listing stages is one directory's entries and what a tree reads is one directory's directories: the
/// ones being looked at are asked about together, however many there are, and moving away and back is a
/// second ask rather than a second process per entry. The cache lives for the run because a rule changed
/// in a file is a change the next run picks up — this is a browser, not a monitor of the repository, and
/// asking again on every frame would be a process per frame.
/// <para>
/// A path is asked about as one entry of the directory holding it, which is what Git answers about a
/// list of paths at once — and a directory is not asked about its own contents. Whether a directory is
/// ignored is its parent's list to answer, and reading the directory itself to find that out would both
/// read a directory nobody asked about and answer the wrong question.
/// </para>
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

    /// <summary>The ignored paths of each directory already asked about, by the root it was asked from.</summary>
    private static readonly Dictionary<(string Root, string Directory), HashSet<string>> Asked = [];

    /// <summary>The repository each root was found to be in, so the walk up is made once per root.</summary>
    /// <remarks>
    /// Nothing is kept for the repository's own sake: it is what says whether there is a repository at
    /// all, which is the answer a root outside every one gives — and the answer a tree rooted outside
    /// every one is read by, so the walk is worth making once rather than per directory walked into.
    /// </remarks>
    private static readonly Dictionary<string, string?> Repositories = new(StringComparer.Ordinal);

    /// <summary>
    /// Whether a path is one Git ignores.
    /// </summary>
    /// <remarks>
    /// Read against the directory holding it, which is where a rule about one entry is stated — the
    /// answer a listing asks for about what it is showing. A view rooted elsewhere asks the other
    /// overload.
    /// </remarks>
    /// <param name="path">The path to ask about, which may be a file or a directory.</param>
    /// <returns>Whether Git ignores it.</returns>
    public static bool Hides(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        return Hides(path, Path.GetDirectoryName(path) is { Length: > 0 } parent ? parent : path);
    }

    /// <summary>
    /// Whether a path is one Git ignores, read against the place a view is rooted at.
    /// </summary>
    /// <remarks>
    /// The repository is looked for upwards from `root` and not from the path: a tree is rooted at a
    /// base, and a base no repository holds is a view under which nothing is ignored — whatever the
    /// directories under it happen to hold. The other way, a repository a directory under the base is
    /// inside would answer for itself, and one tree would be read by two sets of rules.
    /// </remarks>
    /// <param name="path">The path to ask about, which may be a file or a directory.</param>
    /// <param name="root">The directory the view reading the path is rooted at.</param>
    /// <returns>Whether Git ignores it.</returns>
    public static bool Hides(string path, string root)
    {
        ArgumentNullException.ThrowIfNull(path);
        ArgumentNullException.ThrowIfNull(root);

        if (Path.GetDirectoryName(path) is not { Length: > 0 } directory)
        {
            return false;
        }

        var key = (root, directory);

        if (!Asked.TryGetValue(key, out var ignored))
        {
            ignored = Read(directory, root);
            Asked[key] = ignored;
        }

        return ignored.Contains(path);
    }

    /// <summary>Asks Git what of a directory it ignores, from the repository a root is inside.</summary>
    /// <param name="directory">The directory whose entries are asked about.</param>
    /// <param name="root">The directory the view reading them is rooted at.</param>
    /// <returns>The paths Git named, which may be none.</returns>
    private static HashSet<string> Read(string directory, string root)
    {
        var ignored = new HashSet<string>(StringComparer.Ordinal);
        var repository = Repository(root);

        if (repository is null)
        {
            return ignored;
        }

        var entries = Entries(directory);

        if (entries.Count == 0)
        {
            return ignored;
        }

        try
        {
            var asking = new ProcessStartInfo("git")
            {
                WorkingDirectory = repository,
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
                // Asked about as the repository names it, and only when it is the repository's to name:
                // a path beside the repository rather than under it is not one its rules are about, and
                // a program handed one answers nothing at all rather than nothing about that path.
                if (Relative(repository, entry) is not { } relative)
                {
                    continue;
                }

                git.StandardInput.WriteLine(relative);
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
                ignored.Add(Path.GetFullPath(Path.Combine(repository, line.Trim())));
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
    /// How the repository names a path, or nothing when the path is not under it.
    /// </summary>
    /// <param name="repository">The repository root.</param>
    /// <param name="path">The path to name.</param>
    /// <returns>The relative path Git is asked about, or nothing.</returns>
    private static string? Relative(string repository, string path)
    {
        var relative = Path.GetRelativePath(repository, path);

        return relative == ".." || relative.StartsWith($"..{Path.DirectorySeparatorChar}", StringComparison.Ordinal)
            ? null
            : relative;
    }

    /// <summary>
    /// The repository a directory belongs to, or nothing when it belongs to none.
    /// </summary>
    /// <remarks>
    /// Walking up from the directory rather than running <c>git rev-parse</c>: this is asked of every
    /// root a view is read from, and a file check the platform answers immediately is cheaper than a
    /// process. The step that finds a repository is the directory that holds one, which is where the
    /// program is run from, so what it says is relative to a known place.
    /// <para>
    /// What this finds is where to start looking, not the final word: <c>git check-ignore</c> is what
    /// answers, and a directory this takes for a repository that the program does not is a directory
    /// whose entries are all shown.
    /// </para>
    /// </remarks>
    /// <param name="root">The directory to start from.</param>
    /// <returns>The repository's own directory, or nothing.</returns>
    private static string? Repository(string root)
    {
        if (Repositories.TryGetValue(root, out var known))
        {
            return known;
        }

        var at = new DirectoryInfo(root);

        while (at is not null)
        {
            if (Directory.Exists(Path.Combine(at.FullName, ".git")) || File.Exists(Path.Combine(at.FullName, ".git")))
            {
                Repositories[root] = at.FullName;

                return at.FullName;
            }

            at = at.Parent;
        }

        Repositories[root] = null;

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
