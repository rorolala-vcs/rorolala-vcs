using Avalonia;
using Avalonia.Animation;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Threading;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// The card that follows the pointer while this program is carrying entries.
/// </summary>
/// <remarks>
/// The toolkit floats no picture of its own, a drag is handed data and nothing else, and on X11 the source of
/// a drag is told nothing about the pointer while it is on, because the XDND handler swallows the motion before
/// it reaches the tree. So what says a drag is happening is this, drawn over the view at the positions the drop
/// side is told.
/// <para>
/// It is drawn by whichever view the pointer is over, which is why what is carried is held here rather than by
/// the view that started the drag: a drag crossing into another dock is answered by that dock's view, and that
/// view knows nothing of what it did not start. Every view a drag can pass over makes one of these.
/// </para>
/// </remarks>
internal sealed class Ghost
{
    /// <summary>How faded the card is at rest, so that it reads as a carrying, not as a thing.</summary>
    private const double Opacity = 0.7;

    /// <summary>The scale the card is picked up from, so that it arrives at the hand rather than appearing.</summary>
    private const double Waking = 0.92;

    /// <summary>How large the card is, which is also what keeps it inside the view.</summary>
    private const double Size = 48.0;

    /// <summary>How far past the pointer the card is drawn, so that it never sits under it.</summary>
    private const double Step = 12.0;

    /// <summary>How large the picture the card carries is.</summary>
    private const int Icon = 32;

    /// <summary>What a drag of this program's is carrying, for the card's sake, or nothing when none is on.</summary>
    private static Entry? _carrying;

    /// <summary>What the card is drawn over, which is also what says how large the view is.</summary>
    private readonly Canvas _over;

    /// <summary>How the program moves, which the card is picked up with.</summary>
    private readonly IMotion _motion;

    /// <summary>The card's scale, which is what says it has been picked up.</summary>
    private readonly ScaleTransform _scale = new(Waking, Waking);

    /// <summary>What the card's opacity arrives with.</summary>
    private readonly Transitions _fade;

    /// <summary>What the card's scale arrives with.</summary>
    private readonly Transitions _grow;

    /// <summary>The card itself.</summary>
    private readonly Border _card;

    /// <summary>Where the picture of what is carried is put.</summary>
    private readonly ContentControl _face = new()
    {
        HorizontalAlignment = HorizontalAlignment.Center,
        VerticalAlignment = VerticalAlignment.Center,
    };

    /// <summary>What the card is showing, so that a picture is not read from the desktop for every move.</summary>
    private string _shown = string.Empty;

    /// <summary>Makes a card, drawn over the view that will answer for the pointer.</summary>
    /// <param name="over">What it is drawn over: a layer filling the view and taking no pointer of its own.</param>
    /// <param name="motion">How the program moves, which the pickup is drawn with.</param>
    public Ghost(Canvas over, IMotion motion)
    {
        _over = over;
        _motion = motion;

        _fade = new Transitions { motion.Fade(Visual.OpacityProperty, motion.Instant) };
        _grow = new Transitions
        {
            motion.Travel(ScaleTransform.ScaleXProperty, motion.Instant),
            motion.Travel(ScaleTransform.ScaleYProperty, motion.Instant),
        };

        _card = new Border
        {
            IsVisible = false,
            IsHitTestVisible = false,
            Opacity = 0,
            Width = Size,
            Height = Size,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8),
            Child = _face,
            RenderTransform = _scale,
            RenderTransformOrigin = new RelativePoint(0.5, 0.5, RelativeUnit.Relative),
            Transitions = _fade,
        };

        _scale.Transitions = _grow;

        // Read from the look's own keys rather than written in colours, like the band a frame is drawn with:
        // colours of its own here would be a second answer to what this program looks like.
        _card[!Border.BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Primary);
        _card[!Border.BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Bright);
        _card[!Border.BoxShadowProperty] = new DynamicResourceExtension(PaletteKeys.Shadow);

        over.Children.Add(_card);
    }

    /// <summary>
    /// What a drag of this program's is carrying, for the card's sake.
    /// </summary>
    /// <remarks>
    /// Said by the view that started the drag (before it is handed over, and nowhere after) and read by every
    /// view: the one the pointer is over draws the card, and a view that did not start the drag has no other way
    /// to know what it would be showing.
    /// </remarks>
    public static Entry? Carrying
    {
        get => _carrying;
        set => _carrying = value;
    }

    /// <summary>
    /// Draws the card at a point, or takes it off where no drag of this program's is on.
    /// </summary>
    /// <remarks>
    /// A drag from another program carries no card, because what it carries is that program's to draw. The card
    /// is kept inside the view, so that a pointer at the edge does not put it out of sight.
    /// </remarks>
    /// <param name="at">Where the pointer is, in the view's own coordinates.</param>
    public void Following(Point at)
    {
        if (_carrying is not { } carried)
        {
            Unghost();

            return;
        }

        // Read once per thing carried rather than once per move: a drag is told of every pixel the pointer
        // travels, and a picture asked of the desktop for each of them is a picture a pixel.
        if (!string.Equals(_shown, carried.Path, StringComparison.Ordinal))
        {
            _face.Content = Icons.For(carried, Icon);
            _shown = carried.Path;
        }

        var room = new Size(
            Math.Max(0, _over.Bounds.Width - Size),
            Math.Max(0, _over.Bounds.Height - Size));

        Canvas.SetLeft(_card, Math.Clamp(at.X + Step, 0, room.Width));
        Canvas.SetTop(_card, Math.Clamp(at.Y + Step, 0, room.Height));

        if (_card.IsVisible)
        {
            return;
        }

        _card.IsVisible = true;

        // The card is shown where it belongs and at nothing, then brought up: the position is set before the
        // first frame, so a pickup is seen at the hand rather than on the way to it. With motion off the two
        // values are set at once and the card simply appears.
        if (
            _motion.FadeTime(_motion.Instant) > TimeSpan.Zero
            || _motion.TravelTime(_motion.Instant) > TimeSpan.Zero
        )
        {
            Dispatcher.UIThread.Post(Arrived);
        }
        else
        {
            Arrived();
        }
    }

    /// <summary>Takes the card off, which is what every end of a drag does.</summary>
    public void Unghost()
    {
        if (!_card.IsVisible)
        {
            return;
        }

        _card.IsVisible = false;

        // Put back to where a pickup starts, without a transition: a card taken off while it was still arriving
        // would otherwise begin the next drag from wherever that fade had reached.
        _card.Transitions = null;
        _scale.Transitions = null;
        _card.Opacity = 0;
        _scale.ScaleX = Waking;
        _scale.ScaleY = Waking;
        _card.Transitions = _fade;
        _scale.Transitions = _grow;
    }

    /// <summary>Puts the card at the size and opacity it rests at while it is carried.</summary>
    private void Arrived()
    {
        _card.Opacity = Opacity;
        _scale.ScaleX = 1;
        _scale.ScaleY = 1;
    }
}
