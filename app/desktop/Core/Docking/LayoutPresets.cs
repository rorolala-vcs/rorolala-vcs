using System.IO;

namespace RorolalaDesktop.Docking;

/// <summary>One layout preset: what it is called, and the file it is read from.</summary>
/// <param name="Name">What the menu names it by, which is its file name with the underscores spaced.</param>
/// <param name="Path">The preset's file.</param>
internal sealed record LayoutPreset(string Name, string Path);

/// <summary>
/// The layout presets laid beside the program.
/// </summary>
/// <remarks>
/// A preset is a layout file like the one the program saves, in a directory beside the program rather than
/// in the configuration directory: it is a part of the program's own furniture, replaced by an export, and
/// not something a run writes. The name the menu shows is the file's name with the underscores turned into
/// spaces, which is why the presets are not translated: a preset is a file a person puts there, and the file
/// name is the one name it is given.
/// </remarks>
internal static class LayoutPresets
{
    /// <summary>The directory the presets sit in, beside the program.</summary>
    public const string FolderName = "layout-presets";

    /// <summary>The directory the presets are read from.</summary>
    public static string Folder => System.IO.Path.Combine(AppContext.BaseDirectory, FolderName);

    /// <summary>Every preset the directory holds, in the order the menu lists them.</summary>
    /// <returns>The presets, or nothing when the directory holds none.</returns>
    public static IReadOnlyList<LayoutPreset> All() => In(Folder);

    /// <summary>Every preset one directory holds, in the order the menu lists them.</summary>
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

        var files = System.IO.Directory
            .EnumerateFiles(folder, "*.json", SearchOption.TopDirectoryOnly)
            .OrderBy(file => file, StringComparer.OrdinalIgnoreCase)
            .ThenBy(file => file, StringComparer.Ordinal);

        var presets = new List<LayoutPreset>();

        foreach (var file in files)
        {
            var name = System.IO.Path.GetFileNameWithoutExtension(file).Replace('_', ' ');

            if (name.Length > 0)
            {
                presets.Add(new LayoutPreset(name, file));
            }
        }

        return presets;
    }
}
