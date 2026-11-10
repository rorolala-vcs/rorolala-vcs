using Avalonia.Threading;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// The other kind of folder this plugin lets a listing make: one that is a tree.
/// </summary>
/// <remarks>
/// The directory is the file system's to make and name, since that is what its new-folder flow does. What
/// is added here is the mark that makes the directory a tree, and it is put there by the command that owns
/// the format rather than by a second writer of it.
/// </remarks>
internal static class RorolalaTreeCreate
{
    /// <summary>What the menu item is called.</summary>
    public const string LabelKey = "rorolala_vcs.new_tree";

    /// <summary>What a tree made this way is called until it is named.</summary>
    public const string NameKey = "rorolala_vcs.new_tree_name";

    /// <summary>Offers a tree as a kind of folder the file system's menus may make.</summary>
    /// <param name="host">The host the plugin was given.</param>
    public static void Declare(IPluginHost host)
    {
        ArgumentNullException.ThrowIfNull(host);

        host.FolderVariants.Add(
            new FolderVariant(LabelKey, Icons.Park, NameKey, made => Make(host, made))
        );
    }

    /// <summary>Makes the directory a tree, off the window's thread.</summary>
    /// <remarks>
    /// The run is the command line's rather than a copy of its rules here: what a tree is, and what it
    /// refuses, is stated once in the command, and a second implementation would be a second answer that
    /// disagrees the first time the command is fixed. The folder is already made, so a run that fails
    /// leaves a plain folder rather than taking the new row away.
    /// </remarks>
    /// <param name="host">The host the plugin was given.</param>
    /// <param name="directory">The directory that is to become a tree.</param>
    private static void Make(IPluginHost host, string directory)
    {
        var program = Environment.GetEnvironmentVariable(Sync.Program);

        if (string.IsNullOrEmpty(program))
        {
            host.Log.Error(RolaI18N.Get("rorolala_vcs.sync.no_program"));

            return;
        }

        _ = Task.Run(() =>
        {
            var records = new ProgressRecords();

            var (done, said, _) = CommandLine.Asked(
                program,
                ["tree", "init", directory, "--force"],
                Path.GetDirectoryName(directory) ?? directory,
                records,
                () => { }
            );

            if (!done)
            {
                host.Log.Error(said);
            }

            // The listing is read again for the mark, which is what the new row's tree column and badges
            // are drawn from: the directory existed before it was a tree, and a row drawn from that earlier
            // answer would say it is a plain folder.
            Dispatcher.UIThread.Post(host.Files.Touch);
        });
    }
}
