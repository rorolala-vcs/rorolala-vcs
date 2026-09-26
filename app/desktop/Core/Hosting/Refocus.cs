using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// The host's windows being come back to, as one thing every plugin hears.
/// </summary>
/// <remarks>
/// One instance for the whole host rather than one per plugin, because what it says is not about any plugin: the
/// windows are the host's, and every plugin that reads something outside the program is waiting for the same
/// moment.
/// </remarks>
internal sealed class Refocus : IRefocus
{
    /// <inheritdoc />
    public event Action? Regained;

    /// <summary>
    /// Says that a window of the host's has been come back to.
    /// </summary>
    /// <remarks>
    /// Raised by whoever is told of the activation: the main window, and each window a dock was floated in — a
    /// dock in a window of its own is come back to as much as the main one is.
    /// </remarks>
    public void Regain() => Regained?.Invoke();
}
