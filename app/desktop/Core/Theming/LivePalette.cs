using Avalonia.Media;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Theming;

/// <summary>
/// A palette that answers for whichever of the two looks is in force when it is asked.
/// </summary>
/// <remarks>
/// It holds how to find the colours rather than a copy of them, which is the whole of its reason to
/// exist: the user may change the primary, the accent or any of the ten grounds while the program runs,
/// and a plugin that drew itself from a copy taken at start-up would be drawn in colours the window
/// stopped using. Reading it again is what a change reaches, and that is also why nothing here is
/// cached: the answer is two lookups and a comparison.
/// </remarks>
internal sealed class LivePalette : IPalette
{
    /// <summary>How the colours in force are found, which is the service's own business.</summary>
    private readonly Func<ThemePalette> _inForce;

    /// <summary>Makes the palette over the way its colours are found.</summary>
    /// <param name="inForce">How the colours in force are found.</param>
    public LivePalette(Func<ThemePalette> inForce) => _inForce = inForce;

    /// <inheritdoc />
    public Color Bright => _inForce().Bright;

    /// <inheritdoc />
    public Color Mid => _inForce().Mid;

    /// <inheritdoc />
    public Color Dark => _inForce().Dark;

    /// <inheritdoc />
    public Color Front => _inForce().Front;

    /// <inheritdoc />
    public Color Splitter => _inForce().Splitter;
}
