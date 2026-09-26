using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// The card that follows the pointer while this program is carrying entries.
/// </summary>
/// <remarks>
/// The toolkit floats no picture of its own — a drag is handed data and nothing else — and on X11 the source of
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
    /// <summary>How faded the card is, so that it reads as a carrying, not as a thing.</summary>
    private const double Opacity = 0.7;

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
    public Ghost(Canvas over)
    {
        _over = over;

        _card = new Border
        {
            IsVisible = false,
            IsHitTestVisible = false,
            Opacity = Opacity,
            Width = Size,
            Height = Size,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8),
            Child = _face,
        };

        // Read from the look's own keys rather than written in colours, like the band a frame is drawn with:
        // colours of its own here would be a second answer to what this program looks like.
        _card[!Border.BorderBrushProperty] = new DynamicResourceExtension("rorolala.primary");
        _card[!Border.BackgroundProperty] = new DynamicResourceExtension("rorolala.bg.elevated");
        _card[!Border.BoxShadowProperty] = new DynamicResourceExtension("rorolala.shadow");

        over.Children.Add(_card);
    }

    /// <summary>
    /// What a drag of this program's is carrying, for the card's sake.
    /// </summary>
    /// <remarks>
    /// Said by the view that started the drag — before it is handed over, and nowhere after — and read by every
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
        _card.IsVisible = true;
    }

    /// <summary>Takes the card off, which is what every end of a drag does.</summary>
    public void Unghost() => _card.IsVisible = false;
}
