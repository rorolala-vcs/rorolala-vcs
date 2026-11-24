using Avalonia;
using Avalonia.Animation;
using Avalonia.Animation.Easings;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Theming;

/// <summary>How the program moves, as a plugin sees it.</summary>
/// <remarks>
/// A view rather than a second statement of the motion: every member answers the one <see cref="Motion"/> the
/// program is drawn from, so a plugin and the host cannot disagree about a duration or about the level in force.
/// It is stateless, so one instance serves every plugin.
/// </remarks>
internal sealed class MotionService : IMotion
{
    /// <inheritdoc />
    public MotionLevel Level => Motion.Level;

    /// <inheritdoc />
    public TimeSpan Instant => Motion.Instant;

    /// <inheritdoc />
    public TimeSpan Feedback => Motion.Feedback;

    /// <inheritdoc />
    public TimeSpan State => Motion.State;

    /// <inheritdoc />
    public TimeSpan Panel => Motion.Panel;

    /// <inheritdoc />
    public TimeSpan Bar => Motion.Bar;

    /// <inheritdoc />
    public double Near => Motion.Near;

    /// <inheritdoc />
    public double Far => Motion.Far;

    /// <inheritdoc />
    public Easing Ease => Motion.Ease;

    /// <inheritdoc />
    public event EventHandler? Changed
    {
        add => Motion.Changed += value;
        remove => Motion.Changed -= value;
    }

    /// <inheritdoc />
    public DoubleTransition Fade(AvaloniaProperty property, TimeSpan nominal) =>
        Motion.Fade(property, nominal);

    /// <inheritdoc />
    public DoubleTransition Travel(AvaloniaProperty property, TimeSpan nominal) =>
        Motion.Travel(property, nominal);

    /// <inheritdoc />
    public BrushTransition Brush(AvaloniaProperty property, TimeSpan nominal) =>
        Motion.Brush(property, nominal);

    /// <inheritdoc />
    public TransformOperationsTransition Transform(AvaloniaProperty property, TimeSpan nominal) =>
        Motion.Transform(property, nominal);

    /// <inheritdoc />
    public TimeSpan FadeTime(TimeSpan nominal) => Motion.FadeTime(nominal);

    /// <inheritdoc />
    public TimeSpan TravelTime(TimeSpan nominal) => Motion.TravelTime(nominal);
}
