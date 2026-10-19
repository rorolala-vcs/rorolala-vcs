using Avalonia;
using Avalonia.Animation;
using Avalonia.Animation.Easings;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Threading;

namespace RorolalaDesktop.Notifications;

/// <summary>
/// The notifications on screen, standing one above the next in the bottom right of a window.
/// </summary>
/// <remarks>
/// A panel of its own rather than a stack of items, because the movement is the point: a new notification
/// arrives at the bottom and every one already there is pushed up, and that push has to be animated rather
/// than jumping. The layout places every card where it belongs, and a card whose place has changed is drawn
/// at its old place for one frame and then slides to the new one — the same arrangement the plugin manager
/// uses for its cards, for the same reason.
/// <para>
/// The cards are laid out from the bottom up: the newest is the lowest, and the stack grows towards the top
/// of the window, which is where a reader looks for the newest thing.
/// </para>
/// </remarks>
internal sealed class NotificationStack : Panel
{
    /// <summary>How wide a notification is.</summary>
    private const double StackWidth = 380;

    /// <summary>How far one notification stands from the next.</summary>
    private const double Gap = 10;

    /// <summary>How far a notification slides while it arrives or leaves.</summary>
    private const double Slide = 28;

    /// <summary>Where the layout put each card last time, so a move can be animated from there.</summary>
    private readonly Dictionary<Control, double> _tops = new(ReferenceEqualityComparer.Instance);

    /// <summary>What a card's own movement is drawn with.</summary>
    private readonly Transitions _slide;

    /// <summary>Makes an empty stack.</summary>
    public NotificationStack()
    {
        Width = StackWidth;

        _slide =
            new Transitions
            {
                new DoubleTransition
                {
                    Property = TranslateTransform.YProperty,
                    Duration = TimeSpan.FromMilliseconds(180),
                    Easing = new CubicEaseOut(),
                },
            };
    }

    /// <summary>Puts a notification at the bottom of the stack, pushing the others up.</summary>
    /// <param name="card">What to show.</param>
    public void Push(NotificationCard card)
    {
        ArgumentNullException.ThrowIfNull(card);

        var shift = new TranslateTransform { Y = Slide, Transitions = _slide };

        card.RenderTransform = shift;
        card.Opacity = 0;
        card.Dismissed += Dismissed;

        Children.Add(card);

        // The card is measured before it can be slid into place, and the entry runs after the first pass
        // that places it: moving before that would animate it towards a position that is about to change.
        Dispatcher.UIThread.Post(() =>
        {
            shift.Y = 0;
            card.Opacity = 1;
            card.Start();
        });
    }

    /// <inheritdoc />
    protected override Size MeasureOverride(Size availableSize)
    {
        _ = availableSize;

        var height = 0.0;

        foreach (var child in Children)
        {
            child.Measure(new Size(StackWidth, double.PositiveInfinity));
            height += child.DesiredSize.Height;
        }

        if (Children.Count > 1)
        {
            height += Gap * (Children.Count - 1);
        }

        return new Size(StackWidth, height);
    }

    /// <inheritdoc />
    protected override Size ArrangeOverride(Size finalSize)
    {
        var bottom = finalSize.Height;

        for (var index = Children.Count - 1; index >= 0; index--)
        {
            var child = Children[index];
            var height = child.DesiredSize.Height;

            bottom -= height;

            child.Arrange(new Rect(0, bottom, finalSize.Width, height));

            if (_tops.TryGetValue(child, out var previous) && Math.Abs(previous - bottom) > 0.5)
            {
                Shift(child, previous - bottom);
            }

            _tops[child] = bottom;
            bottom -= Gap;
        }

        return finalSize;
    }

    /// <summary>Starts a card on its way out, and takes it away once it has gone.</summary>
    /// <param name="sender">The card.</param>
    /// <param name="e">Not read.</param>
    private void Dismissed(object? sender, EventArgs e)
    {
        if (sender is not NotificationCard card || !Children.Contains(card))
        {
            return;
        }

        card.Dismissed -= Dismissed;
        card.Stop();
        card.Opacity = 0;

        if (card.RenderTransform is TranslateTransform shift)
        {
            shift.Y = Slide;
        }

        // The room is given back after the card has finished leaving it, so that the cards below do not
        // move at the same time as the one that is going: two movements at once read as a shuffle.
        var settle = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(180) };
        settle.Tick += (_, _) =>
        {
            settle.Stop();

            Children.Remove(card);
            _tops.Remove(card);
            InvalidateMeasure();
        };
        settle.Start();
    }

    /// <summary>Draws a card at its old place for a frame, then slides it to where the layout put it.</summary>
    /// <param name="card">The card that moved.</param>
    /// <param name="delta">How far it moved, positive being upwards.</param>
    private void Shift(Control card, double delta)
    {
        if (card.RenderTransform is not TranslateTransform shift)
        {
            return;
        }

        // The jump back is not a movement the reader asked to see: it puts the card where it was before
        // the layout moved it, and only the following move to zero is animated.
        shift.Transitions = null;
        shift.Y = delta;
        shift.Transitions = _slide;

        Dispatcher.UIThread.Post(() => shift.Y = 0);
    }
}
