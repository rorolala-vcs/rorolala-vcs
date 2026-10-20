using System.Runtime.InteropServices;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Platform;
using Avalonia.Threading;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Banner;

/// <summary>
/// The picture at the top of the window: the frames of an aseprite file, played in a loop.
/// </summary>
/// <remarks>
/// It fills its whole area rather than fitting inside it, because it is the window's own frame as much as a
/// picture: the area is the banner, and the banner is four to one whatever the file it was made in. A
/// pixel picture is drawn with the hard interpolation that keeps a pixel a square, and a picture that is
/// taller than a pixel picture would sensibly be — where a smoother interpolation is what the plan asks
/// for — is drawn with the smoother one.
/// <para>
/// A double click asks for a replacement rather than opening anything itself: choosing a file is a window
/// over this one, and the window is what knows how to put one up.
/// </para>
/// </remarks>
internal sealed class BannerView : UserControl
{
    /// <summary>The height a picture is drawn smoothly above, which is the height a pixel picture is not.</summary>
    private const double Gentle = 256;

    /// <summary>Where the frames are drawn.</summary>
    private readonly Image _image = new()
    {
        Stretch = Stretch.Fill,
        HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Stretch,
        VerticalAlignment = Avalonia.Layout.VerticalAlignment.Stretch,
    };

    /// <summary>What is behind the frames, which is what there is to see before one is loaded.</summary>
    private readonly Border _ground = new();

    /// <summary>The clock the frames are played on.</summary>
    private readonly DispatcherTimer _timer = new();

    /// <summary>What is played, if anything.</summary>
    private AsepriteAnimation? _animation;

    /// <summary>The bitmap the current frame is drawn from.</summary>
    private WriteableBitmap? _bitmap;

    /// <summary>Which frame is on screen.</summary>
    private int _frame;

    /// <summary>Makes an empty banner.</summary>
    public BannerView()
    {
        _ground.Child = _image;

        Content = _ground;
        ClipToBounds = true;

        _timer.Tick += (_, _) => Next();

        RenderOptions.SetBitmapInterpolationMode(_image, BitmapInterpolationMode.None);
    }

    /// <summary>Raised when the picture was double-clicked, which asks for another one.</summary>
    public event EventHandler? ReplacementAsked;

    /// <summary>Whether there is a picture to see.</summary>
    public bool HasPicture => _animation is not null;

    /// <summary>
    /// Puts a picture up, or takes the one that is there away.
    /// </summary>
    /// <param name="animation">The frames to play, or nothing to clear them.</param>
    public void Play(AsepriteAnimation? animation)
    {
        _timer.Stop();

        _animation = animation;
        _bitmap = animation is null
            ? null
            : new WriteableBitmap(
                new PixelSize(animation.Width, animation.Height),
                new Vector(96, 96),
                PixelFormat.Bgra8888,
                AlphaFormat.Unpremul
            );

        _frame = 0;
        Show();

        if (animation is { Frames.Count: > 0 })
        {
            _timer.Interval = animation.Frames[0].Duration;
            _timer.Start();
        }
    }

    /// <summary>Draws the banner in the colours of the variant in force.</summary>
    /// <param name="skin">The colours.</param>
    public void Repaint(Skin skin) => _ground.Background = skin.Dark;

    /// <summary>Moves to the next frame, and keeps going while there is one.</summary>
    private void Next()
    {
        if (_animation is not { Frames.Count: > 0 } animation)
        {
            _timer.Stop();

            return;
        }

        _frame = (_frame + 1) % animation.Frames.Count;
        _timer.Interval = animation.Frames[_frame].Duration;

        Show();
    }

    /// <summary>Draws the frame that is up.</summary>
    private void Show()
    {
        if (_animation is not { } animation || _bitmap is null || animation.Frames.Count == 0)
        {
            _image.Source = null;

            return;
        }

        var frame = animation.Frames[Math.Clamp(_frame, 0, animation.Frames.Count - 1)];
        var length = animation.Width * 4;

        using (var locked = _bitmap.Lock())
        {
            for (var row = 0; row < animation.Height; row++)
            {
                Marshal.Copy(
                    frame.Pixels,
                    row * length,
                    IntPtr.Add(locked.Address, row * locked.RowBytes),
                    Math.Min(length, locked.RowBytes)
                );
            }
        }

        _image.Source = _bitmap;

        // The picture is drawn as hard or as smooth as its height asks for. It is measured rather than
        // assumed, since what the banner is tall is the window's answer and not a number written here.
        RenderOptions.SetBitmapInterpolationMode(
            _image,
            Bounds.Height > Gentle ? BitmapInterpolationMode.HighQuality : BitmapInterpolationMode.None
        );
    }

    /// <inheritdoc />
    protected override void OnPropertyChanged(AvaloniaPropertyChangedEventArgs change)
    {
        base.OnPropertyChanged(change);

        if (change.Property == BoundsProperty)
        {
            Show();
        }
    }

    /// <inheritdoc />
    protected override void OnPointerPressed(PointerPressedEventArgs e)
    {
        ArgumentNullException.ThrowIfNull(e);

        if (!e.GetCurrentPoint(this).Properties.IsLeftButtonPressed)
        {
            base.OnPointerPressed(e);

            return;
        }

        if (e.ClickCount == 2)
        {
            // UNWRAP: a double click is two presses, and the second is the one that asks for another
            // picture — the first has already been given to the window to move by.
            e.Handled = true;
            ReplacementAsked?.Invoke(this, EventArgs.Empty);

            return;
        }

        // The banner is the window's own handle: pressing it where there is no button moves the window,
        // which is what a window that draws its own frame has to offer in place of a title bar.
        (TopLevel.GetTopLevel(this) as Window)?.BeginMoveDrag(e);
    }

    /// <inheritdoc />
    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        _timer.Stop();

        base.OnDetachedFromVisualTree(e);
    }
}
