using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// The files having changed, as the plugins say so.
/// </summary>
/// <remarks>
/// A plugin that changed something by running a command line of its own says so here, and every listing that
/// draws the files reads again for it. Nothing about the filesystem tells the program, so the one that made
/// the change is the one that says.
/// </remarks>
internal sealed class FileChanges : IFileChanges
{
    /// <inheritdoc />
    public event Action? Changed;

    /// <inheritdoc />
    public event Action? Redrawn;

    /// <inheritdoc />
    public void Touch() => Changed?.Invoke();

    /// <inheritdoc />
    public void Redraw() => Redrawn?.Invoke();
}
