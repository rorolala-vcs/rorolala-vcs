using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// The sync buttons, as one strip in the Overview's header.
/// </summary>
/// <remarks>
/// Three buttons and the thing that says what they are to leave out, drawn as one control rather than as four
/// commands, because a header command is a button and what is wanted here is more than a button: three that
/// belong together, three whose colour answers a key the user is holding, and a dropdown of choices. The host
/// puts the strip in its strip and asks nothing about what is inside it (see <see cref="DockHeaderView"/>).
/// <para>
/// Reading the work again is drawn first, as it was when it was a command of the dock's own: what a reader
/// reaches for about the whole work belongs in one row, and the sync is one of the things they reach for.
/// </para>
/// </remarks>
internal sealed class SyncStrip : StackPanel
{
    /// <summary>What the marks and the group's rule are drawn in, and what they turn to when a key is held.</summary>
    /// <remarks>
    /// The look's own names rather than colours of this plugin's, so that a strip of marks is drawn in whatever
    /// the look draws every other mark in, and a held key is drawn in the colour the look spends on what must
    /// not be done rather than in a red of this plugin's.
    /// </remarks>
    private const string Ink = PaletteKeys.Front;

    /// <summary>The rule around the group, which is the look's own line between two things.</summary>
    private const string Rule = PaletteKeys.Splitter;

    /// <summary>What the three are drawn in while the key that changes what a run does is held.</summary>
    private const string Held = PaletteKeys.Del;

    /// <summary>
    /// How wide and tall a mark is, and how much room a button keeps around it.
    /// </summary>
    /// <remarks>
    /// The same two numbers the host draws a header command with, because this strip stands in the row those
    /// commands are drawn in: a strip whose marks were another size would be a row of two heights.
    /// </remarks>
    private const double Mark = 14;

    /// <summary>How much room a button keeps around its mark.</summary>
    private const double Room = 6;

    /// <summary>The three buttons, with the mark each is drawn as and what it is called.</summary>
    private readonly List<(Button Button, Border Mark, string Label)> _ways = [];

    /// <summary>The boxes of what is carried, so that a change made elsewhere is drawn here too.</summary>
    private readonly List<(CheckBox Box, string Setting)> _boxes = [];

    /// <summary>The rule around the three, which is drawn in the held colour as well.</summary>
    private readonly Border _group;

    /// <summary>The host, which is what the buttons reach to ask, run and report.</summary>
    private readonly IPluginHost _host;

    /// <summary>What the strip is drawn from.</summary>
    private readonly SyncWatcher _sync;

    /// <summary>Where the work is rooted, read when a button is pressed rather than when it is drawn.</summary>
    private readonly Func<string?> _at;

    /// <summary>Whether the key that changes what a run does is held.</summary>
    private bool _shifted;

    /// <summary>Makes the strip.</summary>
    /// <param name="host">The host.</param>
    /// <param name="sync">What the strip is drawn from.</param>
    /// <param name="at">Where the work is rooted, read when a button is pressed.</param>
    /// <param name="refresh">What reading the work again is, which is the first button.</param>
    public SyncStrip(IPluginHost host, SyncWatcher sync, Func<string?> at, Action refresh)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(sync);
        ArgumentNullException.ThrowIfNull(at);
        ArgumentNullException.ThrowIfNull(refresh);

        _host = host;
        _sync = sync;
        _at = at;

        Orientation = Orientation.Horizontal;
        Spacing = 2;
        VerticalAlignment = VerticalAlignment.Center;

        Children.Add(Button(Glyph(Icons.Refresh), "rorolala_vcs.status.refresh", refresh));

        _group = new Border
        {
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(6),
            Padding = new Thickness(1),
            Margin = new Thickness(2, 0),
            VerticalAlignment = VerticalAlignment.Center,
        };

        var inside = new StackPanel { Orientation = Orientation.Horizontal };

        Add(inside, SyncWay.Up, Icons.CloudUpload, "rorolala_vcs.sync.up");
        Add(inside, SyncWay.Down, Icons.CloudDownload, "rorolala_vcs.sync.down");
        Add(inside, SyncWay.Both, Icons.Sync, "rorolala_vcs.sync.both");

        _group.Child = inside;
        Children.Add(_group);
        Children.Add(More());

        _sync.Changed += (_, _) => Paint();
        AttachedToVisualTree += (_, _) => Wearing();
        DetachedFromVisualTree += (_, _) => Unwearing();

        Paint();
    }

    /// <summary>Draws one of the three, and keeps what pressing it does.</summary>
    /// <param name="inside">Where to put it.</param>
    /// <param name="way">Which way it carries.</param>
    /// <param name="key">The key of the picture it wears.</param>
    /// <param name="label">The key of what it is called.</param>
    private void Add(StackPanel inside, SyncWay way, string key, string label)
    {
        var mark = Glyph(key);
        var button = Button(mark, label, () => Pressed(way));

        _ways.Add((button, mark, label));
        inside.Children.Add(button);
    }

    /// <summary>
    /// Begins the run one of the three offers, with what the key being held says about it.
    /// </summary>
    /// <remarks>
    /// What the key says is read when the button is pressed rather than after the question that may follow it:
    /// a reader holds the key to mean "change history" and answers the question with the pointer, and the two
    /// are not the same moment.
    /// </remarks>
    /// <param name="way">Which way it carries.</param>
    private void Pressed(SyncWay way)
    {
        if (_at() is not { Length: > 0 } directory)
        {
            return;
        }

        _sync.Start(directory, way, _shifted);
    }

    /// <summary>
    /// The button that opens what a run is to leave out.
    /// </summary>
    /// <remarks>
    /// It stands beside the three rather than among them, and is pressable whatever they are: the three are
    /// about the work and this is about what a run is told, which a reader may want to change while a run is
    /// going, so that the next one carries something else.
    /// </remarks>
    /// <returns>The button.</returns>
    private Button More()
    {
        var boxes = new StackPanel { Spacing = 4 };

        Box(boxes, SyncSettings.Index, "rorolala_vcs.sync.index", "--no-index");
        Box(boxes, SyncSettings.Layout, "rorolala_vcs.sync.layout", "--no-layout");
        Box(boxes, SyncSettings.Storage, "rorolala_vcs.sync.storage", "--no-storage");

        var more = Button(Glyph(Icons.MoreVert), "rorolala_vcs.sync.more", command: null);
        more.Flyout = new Flyout { Content = boxes };

        return more;
    }

    /// <summary>Adds one of the three boxes, and what ticking it writes.</summary>
    /// <param name="boxes">Where to put it.</param>
    /// <param name="setting">The setting it stands for.</param>
    /// <param name="label">The key of what it is called.</param>
    /// <param name="flag">The flag leaving it out gives a run, which is a command line's own word.</param>
    private void Box(StackPanel boxes, string setting, string label, string flag)
    {
        var box = new CheckBox
        {
            Content = RolaI18N.Get(label),
            IsChecked = SyncSettings.Carries(setting),
        };

        // What the box does is what the flag beside it says, and the flag is the command line's own word for it:
        // a reader who knows the command knows the box, and one who does not is told which part of a run it is.
        ToolTip.SetTip(box, flag);

        box.IsCheckedChanged += (_, _) =>
        {
            // Written through the config rather than kept here, because the setting is the user's and the file
            // is where it lives: what this strip changed is what the next run reads.
            SyncSettings.Choose(setting, box.IsChecked == true);

            // What there is to carry is asked again, since what a run leaves out is what it would carry: a plan
            // made without the storage is not the answer a plan with it is.
            _sync.Plan(_at());
        };

        _boxes.Add((box, setting));
        boxes.Children.Add(box);
    }

    /// <summary>Draws the strip as the state and the key held say it is.</summary>
    private void Paint()
    {
        var said = _shifted ? RolaI18N.Get("rorolala_vcs.sync.forced") : null;

        foreach (var (button, mark, label) in _ways)
        {
            button.IsEnabled = _sync.Pressable;
            mark[!Border.BackgroundProperty] = new DynamicResourceExtension(_shifted ? Held : Ink);

            ToolTip.SetTip(button, Sentence(label, said));
        }

        _group[!Border.BorderBrushProperty] = new DynamicResourceExtension(_shifted ? Held : Rule);

        // What is carried is read again here, because it is changed in the preference panel as well as in this
        // strip's own dropdown: a box drawing what it last wrote would disagree with the next run.
        foreach (var (box, setting) in _boxes)
        {
            box.IsChecked = SyncSettings.Carries(setting);
        }
    }

    /// <summary>
    /// What a button says when the pointer rests on it: what it does, and why it is drawn as it is.
    /// </summary>
    /// <remarks>
    /// Three things may be worth saying beyond the button's own name, and they are said in the order a reader
    /// wants them: that a held key changes the run, that there is nothing to carry, and why there is nothing —
    /// which is the one thing a reader looking at three grey buttons wants to know.
    /// </remarks>
    /// <param name="label">The key of what the button is called.</param>
    /// <param name="held">What the key being held means, or nothing while it is not.</param>
    /// <returns>What to say.</returns>
    private string Sentence(string label, string? held)
    {
        var said = RolaI18N.Get(label);

        if (held is { Length: > 0 })
        {
            return $"{said} — {held}";
        }

        if (!_sync.Pressable && !_sync.Running && _sync.Why is { Length: > 0 } why)
        {
            return $"{said} — {why}";
        }

        return said;
    }

    /// <summary>Watches for the key that changes what a run does, wherever the pointer is.</summary>
    /// <remarks>
    /// On the window rather than on the strip, because the key is held while the pointer is anywhere: a strip
    /// that only heard it while one of its own buttons had the keyboard would stop turning red the moment the
    /// reader clicked somewhere else.
    /// </remarks>
    private void Wearing()
    {
        if (TopLevel.GetTopLevel(this) is not { } window)
        {
            return;
        }

        window.AddHandler(InputElement.KeyDownEvent, KeyHeld, RoutingStrategies.Tunnel);
        window.AddHandler(InputElement.KeyUpEvent, KeyLetGo, RoutingStrategies.Tunnel);
    }

    /// <summary>Stops watching, since a strip taken out of a tree is not drawn by the key any more.</summary>
    private void Unwearing()
    {
        if (TopLevel.GetTopLevel(this) is not { } window)
        {
            return;
        }

        window.RemoveHandler(InputElement.KeyDownEvent, KeyHeld);
        window.RemoveHandler(InputElement.KeyUpEvent, KeyLetGo);
    }

    /// <summary>Draws the strip red while the key that changes what a run does is down.</summary>
    /// <param name="source">What raised it.</param>
    /// <param name="pressed">The event.</param>
    private void KeyHeld(object? source, KeyEventArgs pressed)
    {
        _ = source;
        Shift(pressed.KeyModifiers);
    }

    /// <summary>Draws it as it was once the key is up.</summary>
    /// <param name="source">What raised it.</param>
    /// <param name="released">The event.</param>
    private void KeyLetGo(object? source, KeyEventArgs released)
    {
        _ = source;
        Shift(released.KeyModifiers);
    }

    /// <summary>Redraws the three in the colour the modifiers say.</summary>
    /// <param name="modifiers">What is being held.</param>
    private void Shift(KeyModifiers modifiers)
    {
        var shifted = modifiers.HasFlag(KeyModifiers.Shift);

        if (shifted == _shifted)
        {
            return;
        }

        _shifted = shifted;
        Paint();
    }

    /// <summary>
    /// A button drawn as its picture, which is what a strip is made of.
    /// </summary>
    /// <remarks>
    /// The same shape the host draws a header command in — a ghost button whose content is a mark — because this
    /// strip stands in the row those commands are drawn in, and a strip that did not match the button beside it
    /// would read as something that arrived from another program.
    /// </remarks>
    /// <param name="mark">What it is drawn as.</param>
    /// <param name="label">The key of what it is called.</param>
    /// <param name="command">What pressing it does, or nothing for one that opens something instead.</param>
    /// <returns>The button.</returns>
    private Button Button(Border mark, string label, Action? command)
    {
        var button = new Button
        {
            Content = mark,
            Padding = new Thickness(Room),
            VerticalAlignment = VerticalAlignment.Center,
            Classes = { "ghost" },
        };

        ToolTip.SetTip(button, RolaI18N.Get(label));

        // What is being held is read again at the press, since a key pressed between the last key event and the
        // press is a key the strip has not been told about yet: what the press says wins.
        button.AddHandler(
            InputElement.PointerPressedEvent,
            (_, pressed) =>
            {
                if (pressed is PointerPressedEventArgs at)
                {
                    Shift(at.KeyModifiers);
                }
            },
            RoutingStrategies.Tunnel
        );

        if (command is not null)
        {
            button.Click += (_, _) => command();
        }

        return button;
    }

    /// <summary>The mark a picture is drawn as: a mask over the ink the strip is drawn in.</summary>
    /// <param name="key">The key of the picture.</param>
    /// <returns>What to draw, which is an empty mark where the library does not hold the picture.</returns>
    private Border Glyph(string key)
    {
        var mark = new Border
        {
            Width = Mark,
            Height = Mark,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        if (_host.Icons.Find(key) is { } picture)
        {
            mark.OpacityMask = new ImageBrush(picture) { Stretch = Stretch.Uniform };

            // What is drawn is much larger than the mark it is shown as, so it is reduced here: the high filter
            // is what keeps a thin stroke whole on the way down.
            Avalonia.Media.RenderOptions.SetBitmapInterpolationMode(
                mark,
                BitmapInterpolationMode.HighQuality
            );
        }

        mark[!Border.BackgroundProperty] = new DynamicResourceExtension(Ink);

        return mark;
    }
}
