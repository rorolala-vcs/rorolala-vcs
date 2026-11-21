using System.IO;

namespace RorolalaDesktop.Docking;

/// <summary>One layout preset: what it is called, and the file it is read from.</summary>
/// <param name="Name">What the tab names it by, which is its file name with the underscores spaced.</param>
/// <param name="Path">The preset's file.</param>
internal sealed record LayoutPreset(string Name, string Path);

/// <summary>
/// The layout presets laid beside the program.
/// </summary>
/// <remarks>
/// A preset is a layout file like the one the program saves, in a directory beside the program rather than
/// in the configuration directory: it is a part of the program's own furniture, replaced by an export, and
/// not something a run writes. The name the tab shows is the file's name with the underscores turned into
/// spaces, which is why the presets are not translated: a preset is a file a person puts there, and the file
/// name is the one name it is given.
/// <para>
/// The five the export lays down stand in the order a person moves through the work, which is written here
/// rather than read from the directory: the file names still say what each arrangement is, and a run that
/// sorts them alphabetically puts the version control one after the terminal. A preset that is not one of
/// the five follows them in alphabetical order, so adding a file still adds a tab without a code change.
/// </para>
/// </remarks>
internal static class LayoutPresets
{
    /// <summary>The directory the presets sit in, beside the program.</summary>
    public const string FolderName = "layout-presets";

    /// <summary>The order the shipped presets are shown in, by the name their files give them.</summary>
    private static readonly string[] Order = ["File", "VCS", "Git", "Settings", "Terminal"];

    /// <summary>The directory the presets are read from.</summary>
    public static string Folder => System.IO.Path.Combine(AppContext.BaseDirectory, FolderName);

    /// <summary>Every preset the directory holds, in the order the tabs show them.</summary>
    /// <returns>The presets, or nothing when the directory holds none.</returns>
    public static IReadOnlyList<LayoutPreset> All() => In(Folder);

    /// <summary>Every preset one directory holds, in the order the tabs show them.</summary>
    /// <remarks>
    /// Read from a named directory rather than only the one beside the program, because that directory is the
    /// program's own output and a test that wrote to it would be a test that changed what a run reads.
    /// </remarks>
    /// <param name="folder">The directory to read.</param>
    /// <returns>The presets, or nothing when the directory holds none.</returns>
    internal static IReadOnlyList<LayoutPreset> In(string folder)
    {
        ArgumentException.ThrowIfNullOrEmpty(folder);

        if (!System.IO.Directory.Exists(folder))
        {
            return [];
        }

        var files = System.IO.Directory.EnumerateFiles(
            folder,
            "*.json",
            SearchOption.TopDirectoryOnly
        );

        var presets = new List<LayoutPreset>();

        foreach (var file in files)
        {
            var name = System.IO.Path.GetFileNameWithoutExtension(file).Replace('_', ' ');

            if (name.Length > 0)
            {
                presets.Add(new LayoutPreset(name, file));
            }
        }

        return
        [
            .. presets
                .OrderBy(preset => Rank(preset.Name))
                .ThenBy(preset => preset.Name, StringComparer.OrdinalIgnoreCase)
                .ThenBy(preset => preset.Name, StringComparer.Ordinal),
        ];
    }

    /// <summary>Where a name stands among the shipped presets, or after all of them.</summary>
    /// <param name="name">The name the file gives.</param>
    /// <returns>The position, or the number of shipped presets for a name that is not one.</returns>
    private static int Rank(string name)
    {
        var at = Array.FindIndex(
            Order,
            known => string.Equals(known, name, StringComparison.OrdinalIgnoreCase)
        );

        return at < 0 ? Order.Length : at;
    }
}
