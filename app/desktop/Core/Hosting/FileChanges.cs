using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// The files having changed, as the plugins say so.
/// </summary>
/// <remarks>
/// A plugin that changed something by running a command line of its own says so here, and every listing that
/// draws the files reads again for it. Nothing about the filesystem tells the program, so the one that made
/// the change is the one that says.
/// <para>
/// A change is said across the board or for one directory, and the two are one answer at two scales: a command
/// whose reach is not known says the first, and a change another program made in one directory says the second,
/// which is what a reader of the work can act on without reading all of it.
/// </para>
/// </remarks>
internal sealed class FileChanges : IFileChanges
{
    /// <inheritdoc />
    public event Action? Changed;

    /// <inheritdoc />
    public event Action<string>? ChangedIn;

    /// <inheritdoc />
    public event Action? Redrawn;

    /// <inheritdoc />
    public void Touch() => Changed?.Invoke();

    /// <inheritdoc />
    public void TouchIn(string directory)
    {
        ArgumentException.ThrowIfNullOrEmpty(directory);

        ChangedIn?.Invoke(directory);
    }

    /// <inheritdoc />
    public void Redraw() => Redrawn?.Invoke();
}
