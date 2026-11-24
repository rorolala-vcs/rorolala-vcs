using Avalonia;
using Avalonia.Animation;
using Avalonia.Animation.Easings;

namespace RorolalaDesktop.Contract;

/// <summary>
/// How the program moves: the durations, the distances and the transitions every surface is drawn with.
/// </summary>
/// <remarks>
/// A plugin draws its own surfaces with the host's motion rather than with numbers of its own: the user's choice
/// reaches every window and every dock, and a duration written into a plugin would be a place that choice does not
/// reach. A transition made here is retuned when the level changes, so a surface already on screen follows the new
/// level without being rebuilt.
/// <para>
/// A plugin with an animation that is not a transition on a property, a drawing that eases itself frame by frame,
/// listens to <see cref="Changed"/> and reads <see cref="Level"/> in its own loop.
/// </para>
/// </remarks>
public interface IMotion
{
    /// <summary>The level in force.</summary>
    MotionLevel Level { get; }

    /// <summary>Immediate feedback, such as a press that has to land with the pointer.</summary>
    TimeSpan Instant { get; }

    /// <summary>A surface answering the pointer.</summary>
    TimeSpan Feedback { get; }

    /// <summary>A change of state.</summary>
    TimeSpan State { get; }

    /// <summary>A panel arriving or leaving.</summary>
    TimeSpan Panel { get; }

    /// <summary>A bar reaching a new step.</summary>
    TimeSpan Bar { get; }

    /// <summary>The small of the two distances a surface travels.</summary>
    double Near { get; }

    /// <summary>The far one.</summary>
    double Far { get; }

    /// <summary>The standard easing.</summary>
    Easing Ease { get; }

    /// <summary>Raised when the level changes.</summary>
    /// <remarks>
    /// The transitions made here are retuned before this is raised, so a plugin that only holds transitions has
    /// nothing to do. It is for an animation the plugin runs itself.
    /// </remarks>
    event EventHandler? Changed;

    /// <summary>A transition on an opacity, which is kept when motion is reduced.</summary>
    /// <param name="property">The property it animates.</param>
    /// <param name="nominal">How long it takes when motion is full.</param>
    /// <returns>The transition.</returns>
    DoubleTransition Fade(AvaloniaProperty property, TimeSpan nominal);

    /// <summary>A transition on a position or a size, which is removed when motion is reduced.</summary>
    /// <param name="property">The property it animates.</param>
    /// <param name="nominal">How long it takes when motion is full.</param>
    /// <returns>The transition.</returns>
    DoubleTransition Travel(AvaloniaProperty property, TimeSpan nominal);

    /// <summary>A transition on a brush, which is kept when motion is reduced.</summary>
    /// <param name="property">The property it animates.</param>
    /// <param name="nominal">How long it takes when motion is full.</param>
    /// <returns>The transition.</returns>
    BrushTransition Brush(AvaloniaProperty property, TimeSpan nominal);

    /// <summary>A transition on a transform, which is removed when motion is reduced.</summary>
    /// <param name="property">The property it animates.</param>
    /// <param name="nominal">How long it takes when motion is full.</param>
    /// <returns>The transition.</returns>
    TransformOperationsTransition Transform(AvaloniaProperty property, TimeSpan nominal);

    /// <summary>How long a fade takes in the level in force.</summary>
    /// <param name="nominal">How long it takes when motion is full.</param>
    /// <returns>The duration in force.</returns>
    TimeSpan FadeTime(TimeSpan nominal);

    /// <summary>How long a travel takes in the level in force.</summary>
    /// <param name="nominal">How long it takes when motion is full.</param>
    /// <returns>The duration in force.</returns>
    TimeSpan TravelTime(TimeSpan nominal);
}
