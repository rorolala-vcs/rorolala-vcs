using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Controls;

/// <summary>
/// One unit of the Vaults rack: the ear with its screws, the name and address, and the lamp.
/// </summary>
/// <remarks>
/// A slot is a place in a list rather than a button. Pressing it does nothing, and what can be done with it
/// is the context menu: the left button is not given a meaning the address does not have. The slot is
/// focusable so that the menu can be reached from the keyboard with Shift+F10, which is the same menu the
/// right button opens.
/// </remarks>
internal sealed class VaultSlot : Border
{
    /// <summary>What the slot is about.</summary>
    private readonly VaultEntry _entry;

    /// <summary>What opens the menu over the slot.</summary>
    private readonly Action<VaultEntry, Control> _menu;

    /// <summary>What the address is called.</summary>
    private readonly TextBlock _name;

    /// <summary>The address itself.</summary>
    private readonly TextBlock _address;

    /// <summary>The lamp that says what the last handshake came to.</summary>
    private readonly VaultLed _led;

    /// <summary>Whether the pointer is over the slot.</summary>
    private bool _over;

    /// <summary>Makes one slot.</summary>
    /// <param name="entry">The address.</param>
    /// <param name="skin">The colours in force.</param>
    /// <param name="first">Whether it is the first slot of its rack, which carries no separator above it.</param>
    /// <param name="menu">What opens the menu over the slot.</param>
    public VaultSlot(VaultEntry entry, Skin skin, bool first, Action<VaultEntry, Control> menu)
    {
        ArgumentNullException.ThrowIfNull(entry);
        ArgumentNullException.ThrowIfNull(skin);
        ArgumentNullException.ThrowIfNull(menu);

        _entry = entry;
        _menu = menu;

        _name = new TextBlock
        {
            Text = entry.Name,
            FontSize = 14,
            FontWeight = FontWeight.SemiBold,
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };

        _address = new TextBlock
        {
            Text = entry.Address,
            FontFamily = skin.Mono,
            FontSize = 11.5,
            Margin = new Thickness(0, 3, 0, 0),
            TextTrimming = TextTrimming.CharacterEllipsis,
        };

        var words = new StackPanel
        {
            VerticalAlignment = VerticalAlignment.Center,
            Spacing = 0,
            Margin = new Thickness(12, 0, 0, 0),
        };

        words.Children.Add(_name);
        words.Children.Add(_address);

        var ear = new Border
        {
            Width = 22,
            Background = skin.Ear,
            BorderBrush = skin.BorderSoft,
            BorderThickness = new Thickness(0, 0, 1, 0),
            Child = Ear(skin),
        };

        _led = new VaultLed();

        var lamp = new Border
        {
            Padding = new Thickness(0, 0, 10, 0),
            Child = _led,
        };

        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("22,*,30"),
            Height = 58,
        };

        row.Children.Add(ear);

        Grid.SetColumn(words, 1);
        row.Children.Add(words);

        Grid.SetColumn(lamp, 2);
        row.Children.Add(lamp);

        Child = row;
        Background = Brushes.Transparent;
        BorderBrush = skin.BorderSoft;
        BorderThickness = new Thickness(0, first ? 0 : 1, 0, 0);
        Focusable = true;
        Cursor = new Cursor(StandardCursorType.Arrow);

        PointerEntered += (_, _) =>
        {
            _over = true;
            Background = skin.Hover;
        };

        PointerExited += (_, _) =>
        {
            _over = false;
            Background = Brushes.Transparent;
        };

        PointerPressed += (_, e) =>
        {
            ArgumentNullException.ThrowIfNull(e);

            if (e.GetCurrentPoint(this).Properties.IsRightButtonPressed)
            {
                e.Handled = true;
                _menu(_entry, this);
            }
        };

        KeyDown += (_, e) =>
        {
            ArgumentNullException.ThrowIfNull(e);

            if (e.Key == Key.F10 && (e.KeyModifiers & KeyModifiers.Shift) != 0)
            {
                e.Handled = true;
                _menu(_entry, this);
            }
        };

        Repaint(skin);
    }

    /// <summary>What the slot is about.</summary>
    public VaultEntry Entry => _entry;

    /// <summary>Whether the pointer is over the slot.</summary>
    public bool Over => _over;

    /// <summary>Stops the lamp's pulse, which is what a slot taken out of the tree has to do.</summary>
    public void Stop() => _led.Stop();

    /// <summary>Draws the slot in the colours of the variant in force.</summary>
    /// <param name="skin">The colours.</param>
    public void Repaint(Skin skin)
    {
        ArgumentNullException.ThrowIfNull(skin);

        _name.Foreground = skin.Front;
        _address.Foreground = skin.Muted;
        BorderBrush = skin.BorderSoft;
        _led.Show(_entry.Status, skin);

        var says = VaultSpeech.Tip(_entry);
        var label = VaultSpeech.Label(_entry.Status);

        ToolTip.SetTip(this, says);
        AutomationProperties.SetName(this, $"{_entry.Name}, {label}");
    }

    /// <summary>
    /// The ear at the left of a slot: two screws and the strip they hold.
    /// </summary>
    /// <param name="skin">The colours in force.</param>
    /// <returns>The ear's inside.</returns>
    private static Control Ear(Skin skin)
    {
        var screws = new Grid
        {
            RowDefinitions = new RowDefinitions("*,*"),
            Margin = new Thickness(0, 5, 0, 5),
        };

        var top = Screw(skin);
        var bottom = Screw(skin);

        screws.Children.Add(top);
        Grid.SetRow(bottom, 1);
        screws.Children.Add(bottom);

        return screws;
    }

    /// <summary>One screw of the ear.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <returns>The screw.</returns>
    private static Control Screw(Skin skin)
    {
        var head = new Border
        {
            Width = 5,
            Height = 5,
            CornerRadius = new CornerRadius(2.5),
            Background = skin.Mid,
            BorderBrush = skin.Splitter,
            BorderThickness = new Thickness(1),
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        return head;
    }
}
