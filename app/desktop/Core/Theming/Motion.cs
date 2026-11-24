using Avalonia;
using Avalonia.Animation;
using Avalonia.Animation.Easings;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Theming;

/// <summary>
/// How the program moves: the durations, the distances and the easing every transition is built from.
/// </summary>
/// <remarks>
/// One place rather than a constant beside each surface, because the choice about motion is one choice: a
/// person who asks for less has to be answered by every transition the program has, and a number written in
/// the file that draws a surface is a number no switch can reach.
/// <para>
/// A transition made here is registered, and <see cref="Level"/> retunes every one of them in place when it
/// changes. That is what lets the preference apply while the program runs, including the transitions the
/// look put in its styles, which cannot be replaced after the window exists.
/// </para>
/// <para>
/// The three levels are the ones a reader of the design expects: <see cref="MotionLevel.Full"/> draws
/// everything, <see cref="MotionLevel.Reduced"/> keeps the short fades and removes travel and scaling, and
/// <see cref="MotionLevel.Off"/> makes every change arrive at once. An opacity is a fade and survives
/// reduction; a position, a size and a rotation are travel and do not.
/// </para>
/// </remarks>
internal static class Motion
{
    /// <summary>Immediate feedback, such as a press that has to land with the pointer.</summary>
    public static readonly TimeSpan Instant = TimeSpan.FromMilliseconds(100);

    /// <summary>A surface answering the pointer, which is short enough to read as the surface itself.</summary>
    public static readonly TimeSpan Feedback = TimeSpan.FromMilliseconds(120);

    /// <summary>A change of state: a selection, a card making room, a notification arriving.</summary>
    public static readonly TimeSpan State = TimeSpan.FromMilliseconds(160);

    /// <summary>A panel arriving or leaving, which is longer because the whole surface is the change.</summary>
    public static readonly TimeSpan Panel = TimeSpan.FromMilliseconds(220);

    /// <summary>A bar reaching a new step, which has to read as one slide rather than a stutter.</summary>
    public static readonly TimeSpan Bar = TimeSpan.FromMilliseconds(280);

    /// <summary>How far a surface steps aside, which is the small of the two distances.</summary>
    public const double Near = 8;

    /// <summary>How far a notification or a card travels, which is the far one.</summary>
    public const double Far = 28;

    /// <summary>The standard easing: fast at the start, settling at the end.</summary>
    public static readonly Easing Ease = new CubicEaseOut();

    /// <summary>The easing a released panel settles with: quicker off the release than the standard one.</summary>
    public static readonly Easing Expo = new ExponentialEaseOut();

    /// <summary>How many registrations may stand before the dead ones are swept.</summary>
    private const int SweepAt = 256;

    /// <summary>The transitions this class made, so that a change of level reaches each of them.</summary>
    private static readonly List<Registration> Registered = [];

    /// <summary>Raised after a change of level has retuned every transition this class made.</summary>
    /// <remarks>
    /// It is for a plugin whose animation is not a transition on a property, a drawing that eases itself frame by
    /// frame: what it holds has already been retuned by the time this arrives.
    /// </remarks>
    public static event EventHandler? Changed;

    /// <summary>How much of the motion is drawn, set from the look the user chose.</summary>
    /// <remarks>
    /// Setting the same level again does nothing, because a colour chosen in the preference panel applies
    /// the look on every frame of a dragged plate, and a sweep per frame would be a sweep for nothing.
    /// </remarks>
    public static MotionLevel Level
    {
        get => _level;
        set
        {
            if (_level == value)
            {
                return;
            }

            _level = value;
            Retune();
            Changed?.Invoke(null, EventArgs.Empty);
        }
    }

    /// <summary>How much of the motion is drawn.</summary>
    private static MotionLevel _level = MotionLevel.Full;

    /// <summary>A transition on an opacity, which is kept when motion is reduced.</summary>
    /// <param name="property">The property it animates.</param>
    /// <param name="nominal">How long it takes when motion is full.</param>
    /// <returns>The transition, already registered.</returns>
    public static DoubleTransition Fade(AvaloniaProperty property, TimeSpan nominal) =>
        Keep(
            new DoubleTransition { Property = property, Easing = Ease },
            nominal,
            MotionKind.Fade,
            Made.Double
        );

    /// <summary>A transition on a position or a size, which is removed when motion is reduced.</summary>
    /// <param name="property">The property it animates.</param>
    /// <param name="nominal">How long it takes when motion is full.</param>
    /// <returns>The transition, already registered.</returns>
    public static DoubleTransition Travel(AvaloniaProperty property, TimeSpan nominal) =>
        Keep(
            new DoubleTransition { Property = property, Easing = Ease },
            nominal,
            MotionKind.Travel,
            Made.Double
        );

    /// <summary>A transition on a brush, which is kept when motion is reduced.</summary>
    /// <param name="property">The property it animates.</param>
    /// <param name="nominal">How long it takes when motion is full.</param>
    /// <returns>The transition, already registered.</returns>
    public static BrushTransition Brush(AvaloniaProperty property, TimeSpan nominal) =>
        Keep(
            new BrushTransition { Property = property, Easing = Ease },
            nominal,
            MotionKind.Fade,
            Made.Brush
        );

    /// <summary>A transition on a transform, which is removed when motion is reduced.</summary>
    /// <param name="property">The property it animates.</param>
    /// <param name="nominal">How long it takes when motion is full.</param>
    /// <returns>The transition, already registered.</returns>
    public static TransformOperationsTransition Transform(AvaloniaProperty property, TimeSpan nominal) =>
        Keep(
            new TransformOperationsTransition { Property = property, Easing = Ease },
            nominal,
            MotionKind.Travel,
            Made.Transform
        );

    /// <summary>How long a fade takes in the level in force, for a timer that has to wait for one.</summary>
    /// <param name="nominal">How long it takes when motion is full.</param>
    /// <returns>The duration in force.</returns>
    public static TimeSpan FadeTime(TimeSpan nominal) => Resolved(nominal, MotionKind.Fade);

    /// <summary>How long a travel takes in the level in force, for a timer that has to wait for one.</summary>
    /// <param name="nominal">How long it takes when motion is full.</param>
    /// <returns>The duration in force.</returns>
    public static TimeSpan TravelTime(TimeSpan nominal) => Resolved(nominal, MotionKind.Travel);

    /// <summary>Keeps a transition, and gives it the duration the level in force asks for.</summary>
    /// <typeparam name="T">The kind of transition.</typeparam>
    /// <param name="transition">The transition to keep.</param>
    /// <param name="nominal">How long it takes when motion is full.</param>
    /// <param name="kind">What it moves.</param>
    /// <param name="made">Which transition type it is, so that a retune can set its duration again.</param>
    /// <returns>The same transition, for the caller to use.</returns>
    private static T Keep<T>(T transition, TimeSpan nominal, MotionKind kind, Made made)
        where T : class
    {
        SetDuration(transition, made, Resolved(nominal, kind));

        Sweep();

        Registered.Add(new Registration(new WeakReference<object>(transition), nominal, kind, made));

        return transition;
    }

    /// <summary>The duration a transition of one kind takes in the level in force.</summary>
    /// <param name="nominal">How long it takes when motion is full.</param>
    /// <param name="kind">What it moves.</param>
    /// <returns>The duration in force.</returns>
    private static TimeSpan Resolved(TimeSpan nominal, MotionKind kind) =>
        _level switch
        {
            MotionLevel.Off => TimeSpan.Zero,
            MotionLevel.Reduced when kind == MotionKind.Travel => TimeSpan.Zero,
            _ => nominal,
        };

    /// <summary>Gives every transition still alive the duration the level now asks for.</summary>
    private static void Retune()
    {
        for (var index = Registered.Count - 1; index >= 0; index--)
        {
            var registration = Registered[index];

            if (registration.Target.TryGetTarget(out var transition))
            {
                SetDuration(
                    transition,
                    registration.Made,
                    Resolved(registration.Nominal, registration.Kind)
                );
            }
            else
            {
                Registered.RemoveAt(index);
            }
        }
    }

    /// <summary>Drops the registrations whose transition has been collected, once there are enough to sweep.</summary>
    /// <remarks>
    /// A transition made for one notification is made again for the next, so the list would otherwise grow
    /// for as long as the program runs. The sweep is occasional rather than on every add, because the list is
    /// short in the common case and a pass per notification would be work for nothing.
    /// </remarks>
    private static void Sweep()
    {
        if (Registered.Count < SweepAt)
        {
            return;
        }

        for (var index = Registered.Count - 1; index >= 0; index--)
        {
            if (!Registered[index].Target.TryGetTarget(out _))
            {
                Registered.RemoveAt(index);
            }
        }
    }

    /// <summary>Sets the duration on one of the transition types this class makes.</summary>
    /// <param name="transition">The transition.</param>
    /// <param name="made">Which type it is.</param>
    /// <param name="duration">How long it should take.</param>
    private static void SetDuration(object transition, Made made, TimeSpan duration)
    {
        switch (made)
        {
            case Made.Double:
                ((DoubleTransition)transition).Duration = duration;
                break;
            case Made.Brush:
                ((BrushTransition)transition).Duration = duration;
                break;
            case Made.Transform:
                ((TransformOperationsTransition)transition).Duration = duration;
                break;
            default:
                break;
        }
    }

    /// <summary>What a transition moves, which is what decides whether reduction removes it.</summary>
    private enum MotionKind
    {
        /// <summary>A colour or an opacity, which is kept when motion is reduced.</summary>
        Fade,

        /// <summary>A position, a size or a rotation, which is removed when motion is reduced.</summary>
        Travel,
    }

    /// <summary>
    /// The transition types this class makes.
    /// </summary>
    /// <remarks>
    /// The types have no common non-generic base that carries <c>Duration</c>, so a retune has to know
    /// which one it is holding. Every transition this class returns is one of these three.
    /// </remarks>
    private enum Made
    {
        /// <summary>A <see cref="DoubleTransition"/>, on a number.</summary>
        Double,

        /// <summary>A <see cref="BrushTransition"/>, on a brush.</summary>
        Brush,

        /// <summary>A <see cref="TransformOperationsTransition"/>, on a transform.</summary>
        Transform,
    }

    /// <summary>One transition this class made, and what it takes to retune it.</summary>
    /// <param name="Target">The transition, held weakly because the surface that owns it decides how long it lives.</param>
    /// <param name="Nominal">How long it takes when motion is full.</param>
    /// <param name="Kind">What it moves.</param>
    /// <param name="Made">Which transition type it is.</param>
    private readonly record struct Registration(
        WeakReference<object> Target,
        TimeSpan Nominal,
        MotionKind Kind,
        Made Made
    );
}
