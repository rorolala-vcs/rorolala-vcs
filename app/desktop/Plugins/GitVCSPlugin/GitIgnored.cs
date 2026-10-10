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
/// asked of the <c>git</c> program, once per repository rather than once per directory: the rules are the
/// repository's, and a view that reads a tree a step at a time asks about one directory per step — which,
/// asked per directory, is one program run per directory walked into.
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

    /// <inheritdoc />
    public bool Ready(string root)
    {
        ArgumentNullException.ThrowIfNull(root);

        return GitIgnores.Ready(root);
    }

    /// <inheritdoc />
    public void Prepare(string root)
    {
        ArgumentNullException.ThrowIfNull(root);

        GitIgnores.Prepare(root);
    }

    /// <inheritdoc />
    public void Forget() => GitIgnores.Forget();
}

/// <summary>
/// What Git ignores in a repository, asked of the program once and kept.
/// </summary>
/// <remarks>
/// One answer per repository rather than one per directory, because the rules are the repository's:
/// <c>git ls-files --others --ignored --exclude-standard --directory</c> names every path they leave out,
/// with a whole ignored directory named once instead of walked into, and one program answers for the whole
/// of what any view is showing. The listing that used to ask per directory cost one program per directory —
/// which a directory's own listing paid once, and a tree paid once per step under the one being opened.
/// <para>
/// The answer is read when a repository is first asked about and kept until the files may have changed
/// (<see cref="Forget"/>), because reading it walks the working tree: what is not a file of the repository
/// is asked about once for a run, not once per row.
/// </para>
/// <para>
/// What the answer says is what the program says about an entry that is not in the index. A file that is
/// both tracked and matched by a rule is not named by <c>ls-files --others</c> and is therefore shown, where
/// <c>check-ignore</c> would have said it is ignored: being in the repository is the better answer of the two
/// for a browser to give.
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

    /// <summary>
    /// The ignored paths of each repository already asked about, by the repository's own directory.
    /// </summary>
    /// <remarks>
    /// Kept as a <see cref="Lazy{T}"/> rather than as the set itself, because the read is a program run and
    /// two threads may ask at once — a view priming a repository off the window's thread while the window's
    /// thread asks about a row. One of them reads and the other waits for that reading rather than starting a
    /// second program of its own, and the lock is held for the lookup alone so that the waiting is on the
    /// answer rather than on this class.
    /// </remarks>
    private static readonly Dictionary<string, Lazy<HashSet<string>>> Ignored = new(StringComparer.Ordinal);

    /// <summary>The repository each root was found to be in, so the walk up is made once per root.</summary>
    /// <remarks>
    /// Nothing is kept for the repository's own sake: it is what says whether there is a repository at
    /// all, which is the answer a root outside every one gives — and the answer a tree rooted outside
    /// every one is read by, so the walk is worth making once rather than per directory walked into.
    /// </remarks>
    private static readonly Dictionary<string, string?> Repositories = new(StringComparer.Ordinal);

    /// <summary>What both dictionaries are guarded by, and nothing else.</summary>
    private static readonly object Gate = new();

    /// <summary>
    /// How a path is spelled for comparison.
    /// </summary>
    /// <remarks>
    /// Windows does not tell two paths differing only in case apart, and one of the two sides of a
    /// comparison here is written by the program rather than read from the directory the entry came from:
    /// comparing them case-sensitively there would be a rule that hides nothing on one machine and
    /// everything on another.
    /// </remarks>
    private static readonly StringComparer Spelling =
        OperatingSystem.IsWindows() ? StringComparer.OrdinalIgnoreCase : StringComparer.Ordinal;

    /// <summary>
    /// Lets go of every answer, so the next one is asked of the program again.
    /// </summary>
    /// <remarks>
    /// Said when the files may have changed, and the rules are files: a <c>.gitignore</c> edited, a
    /// repository set up in a directory that had none, a copy of one fetched. A kept answer outliving that
    /// is a listing hiding by rules nobody states any more — and the cache is what makes asking once per
    /// repository worth it, so it is cleared rather than never kept.
    /// </remarks>
    public static void Forget()
    {
        lock (Gate)
        {
            Ignored.Clear();
            Repositories.Clear();
        }
    }

    /// <summary>
    /// Whether what this provider answers with is already at hand for a root.
    /// </summary>
    /// <remarks>
    /// Asked by a view that reads a whole level of a tree before deciding whether that reading can be made
    /// on the thread its rows are drawn on: a repository that has not been read is one worth reading off
    /// that thread first (<see cref="Prepare"/>).
    /// </remarks>
    /// <param name="root">The directory the view reading the entries is rooted at.</param>
    /// <returns>Whether an answer about the root's repository is held.</returns>
    public static bool Ready(string root)
    {
        ArgumentNullException.ThrowIfNull(root);

        lock (Gate)
        {
            return Repository(root) is not { } repository
                || (Ignored.TryGetValue(repository, out var read) && read.IsValueCreated);
        }
    }

    /// <summary>
    /// Reads what this provider answers with for a root, unless it has been read already.
    /// </summary>
    /// <remarks>
    /// For a caller that is about to ask about a whole level and would rather the walk happened off the
    /// thread the rows are drawn on: what is asked next finds the answer here, so a step that would have
    /// waited for a program does not.
    /// </remarks>
    /// <param name="root">The directory the view reading the entries is rooted at.</param>
    public static void Prepare(string root)
    {
        ArgumentNullException.ThrowIfNull(root);

        _ = Answer(root);
    }

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
    /// The repository is looked for upwards from <c>root</c> and not from the path: a tree is rooted at a
    /// base, and a base no repository holds is a view under which nothing is ignored — whatever the
    /// directories under it happen to hold. The other way, a repository a directory under the base is
    /// inside would answer for itself, and one tree would be read by two sets of rules.
    /// <para>
    /// This reads the repository if it has not been read, so a caller that is about to ask about many paths
    /// wants <see cref="Prepare"/> first: the reading is a program run, and this waits for it.
    /// </para>
    /// </remarks>
    /// <param name="path">The path to ask about, which may be a file or a directory.</param>
    /// <param name="root">The directory the view reading the path is rooted at.</param>
    /// <returns>Whether Git ignores it.</returns>
    public static bool Hides(string path, string root)
    {
        ArgumentNullException.ThrowIfNull(path);
        ArgumentNullException.ThrowIfNull(root);

        return Answer(root) is { } ignored && Hidden(ignored, path);
    }

    /// <summary>
    /// Whether a path is one the rules leave out, or is inside a directory they do.
    /// </summary>
    /// <remarks>
    /// The directories holding it are asked about as well, because the program is asked for whole ignored
    /// directories rather than for every file under them: what is inside one is not named on its own, and a
    /// path that was typed rather than listed — or one drawn while the hidden entries are shown — is still
    /// inside a directory the rules leave out.
    /// </remarks>
    /// <param name="ignored">What the repository's rules leave out.</param>
    /// <param name="path">The path to ask about.</param>
    /// <returns>Whether it is hidden.</returns>
    private static bool Hidden(HashSet<string> ignored, string path)
    {
        for (var at = Path.TrimEndingDirectorySeparator(path); at.Length > 0; at = Above(at))
        {
            if (ignored.Contains(at))
            {
                return true;
            }
        }

        return false;
    }

    /// <summary>The directory holding a path, or nothing when there is nowhere further up.</summary>
    /// <param name="path">The path to step out of.</param>
    /// <returns>The directory holding it, or an empty path at the top.</returns>
    private static string Above(string path) =>
        Path.GetDirectoryName(path) is { Length: > 0 } parent
        && !string.Equals(parent, path, StringComparison.Ordinal)
            ? Path.TrimEndingDirectorySeparator(parent)
            : string.Empty;

    /// <summary>
    /// What a root's repository ignores, reading it if it has not been read.
    /// </summary>
    /// <param name="root">The directory the view reading the entries is rooted at.</param>
    /// <returns>The ignored paths, or nothing when the root is inside no repository.</returns>
    private static HashSet<string>? Answer(string root)
    {
        Lazy<HashSet<string>> reading;

        lock (Gate)
        {
            if (Repository(root) is not { } repository)
            {
                return null;
            }

            if (!Ignored.TryGetValue(repository, out var kept))
            {
                kept = new Lazy<HashSet<string>>(
                    () => Read(repository),
                    LazyThreadSafetyMode.ExecutionAndPublication
                );
                Ignored[repository] = kept;
            }

            reading = kept;
        }

        // The reading is waited for outside the lock: it is a program run, and a view asking about a row
        // while another is reading must not hold this class against everything else that asks.
        return reading.Value;
    }

    /// <summary>Asks Git what of a repository it ignores.</summary>
    /// <param name="repository">The repository's own directory, which is where the program is run from.</param>
    /// <returns>The paths the rules leave out, which may be none.</returns>
    private static HashSet<string> Read(string repository)
    {
        var ignored = new HashSet<string>(Spelling);

        try
        {
            var asking = new ProcessStartInfo("git")
            {
                WorkingDirectory = repository,
                RedirectStandardOutput = true,
                UseShellExecute = false,
                CreateNoWindow = true,
                StandardOutputEncoding = Encoding.UTF8,
            };

            asking.ArgumentList.Add("ls-files");
            asking.ArgumentList.Add("--others");
            asking.ArgumentList.Add("--ignored");
            asking.ArgumentList.Add("--exclude-standard");

            // A whole ignored directory is named once rather than every file under it: one word hides the
            // directory, and a build directory of a hundred thousand objects is a walk this does not make.
            asking.ArgumentList.Add("--directory");

            // NUL-terminated rather than one path per line, because a name that is not plain ASCII is
            // escaped and quoted in the line-separated form — which would be a set of paths that match
            // nothing, and a rule that silently hides nothing at all in a work named in another script.
            asking.ArgumentList.Add("-z");

            using var git = Process.Start(asking);

            if (git is null)
            {
                return ignored;
            }

            // Read before waiting rather than after: a repository with many ignored paths would fill the pipe
            // and wait for this side to read it while this side waited for the program to finish.
            var said = git.StandardOutput.ReadToEnd();

            if (!git.WaitForExit((int)Patience.TotalMilliseconds))
            {
                // WORKAROUND: the program is still holding the directory, so it is killed rather than waited
                // on: it has not answered, and an answer half-read is worse than none.
                git.Kill(entireProcessTree: true);

                return ignored;
            }

            // A program that failed answers nothing: hiding by what it said first would hide by rules nobody
            // stated. A repository the program cannot read is one whose whole listing is shown, which is the
            // same answer as no program at all.
            if (git.ExitCode != 0)
            {
                return ignored;
            }

            foreach (var name in said.Split('\0', StringSplitOptions.RemoveEmptyEntries))
            {
                // A path is answered as the repository names it, which is where the program was run from and
                // what it is read back against; the separator a directory is named with is not part of what
                // the path is.
                ignored.Add(
                    Path.TrimEndingDirectorySeparator(
                        Path.GetFullPath(Path.Combine(repository, name))
                    )
                );
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
    /// root a view is read from, and a file check the platform answers immediately is cheaper than a
    /// process. The step that finds a repository is the directory that holds one, which is where the
    /// program is run from, so what it says is relative to a known place.
    /// <para>
    /// What this finds is where to start looking, not the final word: what the program answers is what
    /// hides anything, and a directory this takes for a repository that the program does not is a
    /// directory whose entries are all shown.
    /// </para>
    /// </remarks>
    /// <param name="root">The directory to start from.</param>
    /// <returns>The repository's own directory, or nothing when it belongs to none.</returns>
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
}
