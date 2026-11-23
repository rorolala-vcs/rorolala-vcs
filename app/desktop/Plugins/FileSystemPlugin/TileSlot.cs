using Avalonia;
using Avalonia.Controls;

namespace FileSystemPlugin;

/// <summary>
/// One tile whose picture is built when the tile comes near the viewport.
/// </summary>
/// <remarks>
/// A grid realizes a tile for every entry it lists, so a directory of a few thousand images would
/// otherwise ask for a few thousand previews before anything is drawn. The tile's shape and its
/// caption are made at once, because the caption is what decides the height the panel lays out; only
/// the picture is deferred, and the room it takes is held by a placeholder of the same size, so the
/// layout is the one the tile would have had.
/// <para>
/// The picture is built when the slot's effective viewport reaches it, and dropped when the viewport
/// is two of its own heights away. A tile that leaves the tree cancels its preview by itself, see
/// <see cref="PreviewTile"/>.
/// </para>
/// </remarks>
internal sealed class TileSlot : Border
{
    /// <summary>What builds the picture when the tile is near the viewport.</summary>
    private readonly Func<Control> _build;

    /// <summary>The tile's name, which is what decides its height.</summary>
    private readonly TextBlock _caption;

    /// <summary>The column the placeholder and the picture take turns in.</summary>
    private readonly StackPanel _stack;

    /// <summary>The placeholder, kept so that leaving the viewport puts it back without building one.</summary>
    private readonly Panel _placeholder;

    /// <summary>The picture on screen, or nothing while the placeholder is.</summary>
    private Control? _content;

    /// <summary>The last effective viewport, so that a size that arrives after it is answered too.</summary>
    private Rect _viewport;

    /// <summary>Makes a slot for one tile.</summary>
    /// <param name="build">What builds the picture.</param>
    /// <param name="caption">The tile's name, which is what decides its height.</param>
    /// <param name="size">How wide and tall the picture is.</param>
    /// <param name="room">How much room is left around the picture.</param>
    public TileSlot(Func<Control> build, TextBlock caption, double size, double room)
    {
        ArgumentNullException.ThrowIfNull(build);
        ArgumentNullException.ThrowIfNull(caption);

        _build = build;
        _caption = caption;
        _placeholder = new Panel { Width = size, Height = size };
        _stack = new StackPanel { Spacing = 8, Children = { _placeholder, caption } };

        Padding = new Thickness(room);
        CornerRadius = new CornerRadius(8);
        Child = _stack;

        EffectiveViewportChanged += (_, changed) => Consider(changed.EffectiveViewport);

        // The viewport is read before the first arrange on a slot that was made while the list was
        // already up, and the size that arrives after it is the first answer this control has.
        SizeChanged += (_, _) => Consider(_viewport);
    }

    /// <summary>How many times the picture was built, which is what a test counts.</summary>
    internal int Builds { get; private set; }

    /// <summary>The tile's name, which a panel reads the font from when it measures names.</summary>
    internal TextBlock Caption => _caption;

    /// <summary>Whether the picture is on screen.</summary>
    internal bool Shown => _content is not null;

    /// <summary>Builds or drops the picture for where the viewport stands.</summary>
    /// <param name="viewport">The control's effective viewport, in its own coordinates.</param>
    private void Consider(Rect viewport)
    {
        if (viewport.Width <= 0 || viewport.Height <= 0)
        {
            return;
        }

        _viewport = viewport;

        if (Bounds.Width <= 0 || Bounds.Height <= 0)
        {
            return;
        }

        var self = new Rect(0, 0, Bounds.Width, Bounds.Height);
        var near = viewport.Inflate(new Thickness(0, viewport.Height, 0, viewport.Height));

        if (near.Intersects(self))
        {
            Show();

            return;
        }

        var far = viewport.Inflate(new Thickness(0, viewport.Height * 2, 0, viewport.Height * 2));

        if (!far.Intersects(self))
        {
            Hide();
        }
    }

    /// <summary>Puts the picture where the placeholder stands.</summary>
    private void Show()
    {
        if (_content is not null)
        {
            return;
        }

        var content = _build();

        _content = content;
        Builds++;
        _stack.Children[0] = content;
    }

    /// <summary>Puts the placeholder back, which detaches the picture and cancels its preview.</summary>
    private void Hide()
    {
        if (_content is null)
        {
            return;
        }

        _stack.Children[0] = _placeholder;
        _content = null;
    }
}
