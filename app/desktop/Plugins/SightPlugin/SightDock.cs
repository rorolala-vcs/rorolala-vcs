using Avalonia.Controls;
using RorolalaDesktop.Contract;

namespace SightPlugin;

/// <summary>The preview dock: the Sight view, whose corner holds the previewer and info tools.</summary>
/// <remarks>
/// The dock carries no header: the two things a reader reaches for, switching the previewer and
/// asking what is being shown, stand in the preview's own corner, as the Version Graph's tools do.
/// What the dock draws is the whole of it.
/// </remarks>
internal sealed class SightDock : IDockView
{
    /// <summary>The view the dock shows.</summary>
    private readonly SightView _view;

    /// <summary>Makes the dock.</summary>
    /// <param name="host">The host.</param>
    public SightDock(IPluginHost host) => _view = new SightView(host);

    /// <inheritdoc />
    public Control View => _view;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderItem> Header => [];
}
