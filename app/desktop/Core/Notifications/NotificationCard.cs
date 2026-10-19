using Avalonia;
using Avalonia.Animation;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using Avalonia.Threading;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;

namespace RorolalaDesktop.Notifications;

/// <summary>
/// One notification in the stack: what it says, how long it has left, and the way to put it away.
/// </summary>
/// <remarks>
/// The card owns its clock rather than the stack, because the two things a notification's life is made of
/// are its own: the pointer being on it, and the five seconds it was given. The stack owns the room it
/// takes and the movement of the cards around it, which is the part that belongs to the list rather than
/// to one entry.
/// </remarks>
internal sealed class NotificationCard : Border
{
    /// <summary>How long a notification stands before it puts itself away.</summary>
    public static readonly TimeSpan Life = TimeSpan.FromSeconds(5);

    /// <summary>How often the bar and the remaining time are brought up to date.</summary>
    private static readonly TimeSpan Tick = TimeSpan.FromSeconds(1.0 / 60.0);

    /// <summary>The horizontal scale of the bar, from one at the start to nothing when the card goes.</summary>
    private readonly ScaleTransform _bar;

    /// <summary>What counts the five seconds down.</summary>
    private readonly DispatcherTimer _clock;

    /// <summary>Whether the clock is stopped while the pointer is on the card.</summary>
    private bool _paused;

    /// <summary>Whether this card has already asked to be taken away.</summary>
    private bool _dismissed;

    /// <summary>How much of the five seconds is left.</summary>
    private double _remaining = Life.TotalSeconds;

    /// <summary>Makes the card and everything it is drawn from.</summary>
    /// <param name="notification">What to show.</param>
    /// <param name="i18n">Where the close button's name is written.</param>
    /// <param name="pictures">Where the close button's picture is read from.</param>
    public NotificationCard(Notification notification, I18nService i18n, IIconLibrary pictures)
    {
        ArgumentNullException.ThrowIfNull(notification);
        ArgumentNullException.ThrowIfNull(i18n);
        ArgumentNullException.ThrowIfNull(pictures);

        var ink = Ink(notification.Level);

        this[!BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Bright);
        this[!BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);
        this[!BoxShadowProperty] = new DynamicResourceExtension(PaletteKeys.Shadow);

        BorderThickness = new Thickness(1);
        CornerRadius = new CornerRadius(0);
        ClipToBounds = true;

        var stripe = new Border { Width = 3 };
        stripe[!BackgroundProperty] = new DynamicResourceExtension(ink);

        var title = new TextBlock
        {
            Text = notification.Title,
            Classes = { "title" },
            TextWrapping = TextWrapping.Wrap,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, 8, 0),
        };

        var close = new Button
        {
            Content = Picture(pictures),
            Classes = { "tool" },
            HorizontalAlignment = HorizontalAlignment.Right,
            VerticalAlignment = VerticalAlignment.Top,
        };

        AutomationProperties.SetName(close, i18n.Get("window.dismiss"));
        close.Click += (_, _) => Dismiss();

        var head = new Grid { ColumnDefinitions = new ColumnDefinitions("*,28") };
        Grid.SetColumn(title, 0);
        Grid.SetColumn(close, 1);
        head.Children.Add(title);
        head.Children.Add(close);

        var body = notification.Content();

        var content = new StackPanel
        {
            Spacing = 8,
            Margin = new Thickness(14, 10, 10, 10),
            Children = { head, body },
        };

        // The bar is a plain border scaled from the left rather than a progress control: at one pixel the
        // toolkit's own template is not guaranteed to draw its track, and what is wanted here is a line that
        // is there the moment the card is.
        _bar = new ScaleTransform(1, 1);

        var fill = new Border
        {
            RenderTransform = _bar,
            RenderTransformOrigin = new RelativePoint(0, 0.5, RelativeUnit.Relative),
        };
        fill[!BackgroundProperty] = new DynamicResourceExtension(ink);

        var track = new Border
        {
            Height = 1,
            ClipToBounds = true,
            Child = fill,
        };
        track[!BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Dark);

        var area = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("3,*"),
            RowDefinitions = new RowDefinitions("*,1"),
        };

        Grid.SetColumn(stripe, 0);
        Grid.SetRow(stripe, 0);
        Grid.SetColumn(content, 1);
        Grid.SetRow(content, 0);
        Grid.SetColumn(track, 0);
        Grid.SetColumnSpan(track, 2);
        Grid.SetRow(track, 1);

        area.Children.Add(stripe);
        area.Children.Add(content);
        area.Children.Add(track);

        Child = area;

        _clock = new DispatcherTimer { Interval = Tick };
        _clock.Tick += (_, _) => Advance();

        PointerEntered += (_, _) => _paused = true;
        PointerExited += (_, _) => _paused = false;

        Transitions =
            new Transitions
            {
                new DoubleTransition
                {
                    Property = Visual.OpacityProperty,
                    Duration = TimeSpan.FromMilliseconds(160),
                },
            };
    }

    /// <summary>Raised once, when the card has decided it should go.</summary>
    public event EventHandler? Dismissed;

    /// <summary>Starts counting the five seconds down.</summary>
    public void Start() => _clock.Start();

    /// <summary>Stops the clock, whether the card is going away or being held.</summary>
    public void Stop() => _clock.Stop();

    /// <summary>Puts the card away, which the close button and the clock both do.</summary>
    public void Dismiss()
    {
        if (_dismissed)
        {
            return;
        }

        _dismissed = true;
        _clock.Stop();
        Dismissed?.Invoke(this, EventArgs.Empty);
    }

    /// <summary>Takes the clock's step, if the pointer is not holding the card still.</summary>
    private void Advance()
    {
        if (_paused)
        {
            return;
        }

        _remaining -= Tick.TotalSeconds;

        if (_remaining <= 0)
        {
            Dismiss();

            return;
        }

        _bar.ScaleX = _remaining / Life.TotalSeconds;
    }

    /// <summary>What a level is drawn with: the program's own blue, its amber, or its red.</summary>
    /// <param name="level">How serious the notification is.</param>
    /// <returns>The resource name of the colour it wears.</returns>
    private static string Ink(NotificationLevel level) =>
        level switch
        {
            NotificationLevel.Warning => PaletteKeys.Warn,
            NotificationLevel.Error => PaletteKeys.Del,
            _ => PaletteKeys.Primary,
        };

    /// <summary>The close button's mark, or a cross when no picture was contributed.</summary>
    /// <param name="pictures">Where pictures are read from.</param>
    /// <returns>What the close button is drawn as.</returns>
    private static Control Picture(IIconLibrary pictures)
    {
        if (pictures.Find(Icons.Close) is { } picture)
        {
            return WindowControls.Glyph(picture, WindowControls.GlyphSize);
        }

        return new TextBlock
        {
            Text = "\u00d7",
            FontSize = 16,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };
    }
}
