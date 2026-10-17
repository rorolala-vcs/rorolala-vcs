using Avalonia.Controls;
using FileSystemPlugin;
using RorolalaDesktop.Contract;

namespace TerminalPlugin;

/// <summary>
/// The Terminal dock: one shell, in the directory the tree is rooted at.
/// </summary>
/// <remarks>
/// A dock per activation rather than one shared view, because a terminal is a process: two docks are two
/// shells, and closing one ends that one's shell and no other. Where it starts is the base the File System
/// plugin publishes, since that is where the user is working; it is read once, when the shell is started, so
/// that rooting the tree elsewhere afterwards does not pull a running shell out of its own directory.
/// </remarks>
internal sealed class TerminalDock : IDockView
{
    /// <summary>The control the dock shows.</summary>
    private readonly TerminalView _view;

    /// <summary>Makes the dock, and the shell in it.</summary>
    /// <param name="log">Where a shell that could not be started is said.</param>
    public TerminalDock(ILog log)
    {
        var at = BaseDirectory.Current is { Length: > 0 } directory && Directory.Exists(directory)
            ? directory
            : Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);

        _view = new TerminalView(
            log,
            TerminalSettings.Program(),
            TerminalSettings.Chosen(),
            at,
            TerminalSettings.Family(),
            TerminalSettings.Size()
        );
    }

    /// <inheritdoc />
    public Control View => _view;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderItem> Header => [];
}
