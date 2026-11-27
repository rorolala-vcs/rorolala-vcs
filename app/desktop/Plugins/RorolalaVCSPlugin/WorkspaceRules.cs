using System.Text;
using RolaSharp;

namespace RorolalaVCSPlugin;

/// <summary>The rules one Vault handed down, read only.</summary>
/// <param name="Name">The name the Workspace bound the Vault under.</param>
/// <param name="Patterns">What its rule files state, in file order, without the notes.</param>
internal sealed record VaultRules(string Name, IReadOnlyList<string> Patterns);

/// <summary>
/// The ignore rules a Workspace works by, as the files hold them.
/// </summary>
/// <remarks>
/// The Workspace's own rules are one file, the one a new Workspace is made with, and this reads and
/// writes that file's text. What a line means is the loader's and is not parsed here; the paths the
/// two files sit at are the library's, read through <see cref="RolaPaths"/>.
/// <para>
/// The rules a Vault handed down are read for showing rather than for writing: a fetch writes them,
/// and they are the Vault's. Each Vault is one directory under the Workspace's ignore rules, named
/// by the name the Workspace bound it under.
/// </para>
/// <para>
/// A rule file is small and is read when the view is drawn; nothing else in this window keeps a
/// reading of it. The writing is staged beside the file and swapped in, because a rule file read
/// halfway through a write would be a tree read by rules nobody stated.
/// </para>
/// </remarks>
internal sealed class WorkspaceRules
{
    /// <summary>The encoding a rule file is written in, which is the command line's.</summary>
    private static readonly UTF8Encoding Utf8 = new(encoderShouldEmitUTF8Identifier: false);

    /// <summary>Makes a reading of the rules holding the Workspace's root and its file.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <param name="file">The file the Workspace's own rules are kept in.</param>
    /// <param name="lines">The file's lines, in the order it holds them.</param>
    /// <param name="vaults">The rules each Vault handed down.</param>
    private WorkspaceRules(
        string root,
        string file,
        IReadOnlyList<string> lines,
        IReadOnlyList<VaultRules> vaults
    )
    {
        Root = root;
        File = file;
        Lines = lines;
        Vaults = vaults;
    }

    /// <summary>The Workspace's root.</summary>
    public string Root { get; }

    /// <summary>The file the Workspace's own rules are kept in.</summary>
    public string File { get; }

    /// <summary>The file's lines, in the order the loader applies them.</summary>
    public IReadOnlyList<string> Lines { get; }

    /// <summary>The rules each Vault handed down, in the order the directories read.</summary>
    public IReadOnlyList<VaultRules> Vaults { get; }

    /// <summary>The file's path as it reads from the Workspace's root.</summary>
    /// <remarks>
    /// Shown over the text editor rather than the whole path, which on a deep tree is mostly the
    /// directories the reader already knows. The separator is the file's own spelling, because the
    /// path is one a reader may copy into a program on this machine.
    /// </remarks>
    public string Shown => Path.GetRelativePath(Root, File);

    /// <summary>Reads the rules of a Workspace.</summary>
    /// <param name="root">The Workspace's root.</param>
    /// <returns>The rules the files hold.</returns>
    /// <exception cref="IOException">A rule file cannot be read.</exception>
    /// <exception cref="UnauthorizedAccessException">A rule file cannot be reached.</exception>
    public static WorkspaceRules Read(string root)
    {
        ArgumentNullException.ThrowIfNull(root);

        var file = RolaPaths.WorkspaceIgnoreRuleFile(root);
        var lines = System.IO.File.Exists(file) ? System.IO.File.ReadAllLines(file) : [];

        return new WorkspaceRules(root, file, lines, ReadVaults(root));
    }

    /// <summary>Writes the Workspace's own rules.</summary>
    /// <remarks>
    /// The text is written beside the rules directory and moved onto the file, so a reader never meets
    /// half of two rule sets. It is not written inside the directory: the loader reads every file under
    /// it, so a temporary there would be a rule set of its own until it was moved.
    /// <para>
    /// A list with no lines is an empty file rather than no file: the Workspace still states its rules,
    /// and it states none of them.
    /// </para>
    /// </remarks>
    /// <param name="lines">The lines to write, in the order the loader is to apply them.</param>
    /// <exception cref="IOException">The file cannot be written.</exception>
    /// <exception cref="UnauthorizedAccessException">The file cannot be reached.</exception>
    public void Write(IReadOnlyList<string> lines)
    {
        ArgumentNullException.ThrowIfNull(lines);

        var directory = Path.GetDirectoryName(File)
            ?? throw new InvalidOperationException($"`{File}` is not a path with a directory");
        var beside = Path.GetDirectoryName(directory)
            ?? throw new InvalidOperationException($"`{directory}` is not a path with a directory");
        Directory.CreateDirectory(directory);

        var text = lines.Count == 0 ? string.Empty : $"{string.Join('\n', lines)}\n";
        var temporary = Path.Combine(
            beside,
            $"__{Path.GetFileName(File)}-{Guid.NewGuid():N}.tmp"
        );

        System.IO.File.WriteAllText(temporary, text, Utf8);

        try
        {
            System.IO.File.Move(temporary, File, overwrite: true);
        }
        finally
        {
            if (System.IO.File.Exists(temporary))
            {
                System.IO.File.Delete(temporary);
            }
        }
    }

    /// <summary>
    /// Reads the rules the Vaults handed down.
    /// </summary>
    /// <remarks>
    /// A directory whose name begins with a dot is left out: a fetch stages the copy it is writing
    /// beside the one it replaces, under a dot name, and half of a copy is not a set of rules. A
    /// nested file is read where it sits, in path order, which is the order the loader reads them.
    /// </remarks>
    /// <param name="root">The Workspace's root.</param>
    /// <returns>What each Vault's directories hold.</returns>
    private static IReadOnlyList<VaultRules> ReadVaults(string root)
    {
        var held = RolaPaths.WorkspaceIgnoreRuleVaults(root);

        if (!Directory.Exists(held))
        {
            return [];
        }

        var vaults = new List<VaultRules>();

        foreach (
            var directory in Directory
                .EnumerateDirectories(held)
                .Where(at => !Path.GetFileName(at).StartsWith('.'))
                .OrderBy(at => at, StringComparer.Ordinal)
        )
        {
            var patterns = new List<string>();
            var files = Directory
                .EnumerateFiles(directory, "*", SearchOption.AllDirectories)
                .OrderBy(at => at, StringComparer.Ordinal);

            foreach (var file in files)
            {
                foreach (var line in System.IO.File.ReadAllLines(file))
                {
                    var text = line.Trim();

                    if (text.Length > 0 && !text.StartsWith('#'))
                    {
                        patterns.Add(text);
                    }
                }
            }

            vaults.Add(new VaultRules(Path.GetFileName(directory), patterns));
        }

        return vaults;
    }
}
