using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>The four commands one preset carries the file operations out with.</summary>
/// <remarks>
/// Each is a template rather than a fixed argument list: where a path goes is named in it, so a command
/// may put the two wherever it needs them — a pair the runner hands each of over as one argument.
/// </remarks>
/// <param name="Copy">What copies a source to a target.</param>
/// <param name="Move">What moves a source to a target.</param>
/// <param name="RemoveDirs">What removes a directory and everything under it.</param>
/// <param name="RemoveFiles">What removes a file.</param>
public sealed record FileOperationCommands(string Copy, string Move, string RemoveDirs, string RemoveFiles);

/// <summary>
/// A named set of file-operation commands a plugin contributes beside this plugin's own.
/// </summary>
/// <remarks>
/// This is the extension point, and it is reached by depending on this plugin: a manifest that names
/// <c>rorolala.file_system</c> in its dependencies is started after it, and its
/// <c>Initialize</c> may hand one of these to <see cref="FileOperationPresets.Add"/>. What is
/// contributed becomes a way to fill the four command settings, not a fifth setting of its own.
/// </remarks>
public interface IFileOperationPreset
{
    /// <summary>The stable value the preset is chosen by, unique among the presets offered.</summary>
    string Id { get; }

    /// <summary>An i18n key naming the preset.</summary>
    string LabelKey { get; }

    /// <summary>The four command templates the preset writes.</summary>
    FileOperationCommands Commands { get; }
}

/// <summary>
/// The presets the File System plugin's settings offer, and where a dependent plugin adds its own.
/// </summary>
/// <remarks>
/// The catalogue holds the very options list the declared setting carries, and a dependent plugin
/// initializes after this one, so what it adds is there by the time the preference panel — which is drawn
/// after every plugin has started — reads the options. The list is rebuilt on every start, so a plugin
/// that was there last run and is gone this one leaves nothing behind.
/// </remarks>
public static class FileOperationPresets
{
    /// <summary>The value of the option that writes nothing: the way to set each command by hand.</summary>
    public const string Custom = "custom";

    /// <summary>
    /// What Rorolala's own plugin offers as its preset, which is also what a file operation is done with until
    /// the user says otherwise.
    /// </summary>
    /// <remarks>
    /// Written here rather than in the plugin that offers them, because the default and the preset have to be one
    /// text: the panel reads the preset in force back from what the four settings are worth, so a copy of these
    /// that drifted would leave the preset reading as one that had been changed by hand.
    /// <para>
    /// It is public because the plugin that offers them reads them, and it names Rorolala rather than the command
    /// line it calls because that is what the option offering them is called.
    /// </para>
    /// </remarks>
    public static readonly FileOperationCommands Rorolala = new(
        "{{env:ROLA_EXE}} fs-ops cp {{from}} {{to}}",
        "{{env:ROLA_EXE}} fs-ops mv {{from}} {{to}}",
        "{{env:ROLA_EXE}} fs-ops rm {{from}}",
        "{{env:ROLA_EXE}} fs-ops rm {{from}}"
    );

    /// <summary>Every preset on offer, in the order they are shown, the custom one last.</summary>
    private static readonly List<SettingOption> Offer = [];

    /// <summary>The options list the setting carries, which a dependent plugin appends to.</summary>
    internal static IReadOnlyList<SettingOption> Options => Offer;

    /// <summary>Fills the catalogue with this plugin's own presets, before any dependent plugin starts.</summary>
    internal static void BuiltIn()
    {
        Offer.Clear();
        Offer.Add(Written("windows", "rorolala_file_system.setting.preset.windows", FileOps.Windows));
        Offer.Add(Written("unix", "rorolala_file_system.setting.preset.unix", FileOps.Unix));
        Offer.Add(new SettingOption(Custom, "rorolala_file_system.setting.preset.custom"));
    }

    /// <summary>Adds a preset a dependent plugin contributes, beside the built-in ones.</summary>
    /// <remarks>
    /// A preset of an identity already offered is ignored rather than replacing it, so that two plugins
    /// claiming one name cannot have the panel show the same value twice. The custom option stays last,
    /// where the panel expects the fallback to be.
    /// </remarks>
    /// <param name="preset">The preset to add.</param>
    public static void Add(IFileOperationPreset preset)
    {
        ArgumentNullException.ThrowIfNull(preset);

        if (Offer.Any(option => string.Equals(option.Value, preset.Id, StringComparison.Ordinal)))
        {
            return;
        }

        var option = Written(preset.Id, preset.LabelKey, preset.Commands);
        var custom = Offer.FindIndex(entry => string.Equals(entry.Value, Custom, StringComparison.Ordinal));

        if (custom < 0)
        {
            Offer.Add(option);
        }
        else
        {
            Offer.Insert(custom, option);
        }
    }

    /// <summary>One option, carrying what its commands write into the four settings.</summary>
    /// <param name="id">The value the preset is chosen by.</param>
    /// <param name="labelKey">An i18n key naming it.</param>
    /// <param name="commands">The commands it writes.</param>
    private static SettingOption Written(string id, string labelKey, FileOperationCommands commands) =>
        new(
            id,
            labelKey,
            new Dictionary<string, string>(StringComparer.Ordinal)
            {
                [FileOps.CopySetting] = commands.Copy,
                [FileOps.MoveSetting] = commands.Move,
                [FileOps.RemoveDirsSetting] = commands.RemoveDirs,
                [FileOps.RemoveFilesSetting] = commands.RemoveFiles,
            }
        );
}
