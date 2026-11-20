using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Controls;

/// <summary>
/// One address in the create window's picker: a lamp, a name, an address, and the radio that says it is
/// the one chosen.
/// </summary>
/// <remarks>
/// The row is a radio rather than a list item that does something when clicked: choosing an address is
/// choosing which Vault the new Workspace is bound to, and the radio is the mark that says so. The lamp
/// repeats what the Vaults dock says about reachability, so the choice can be made on the same fact twice.
/// </remarks>
internal sealed class VaultPick : Border
{
    /// <summary>What the row is about.</summary>
    private readonly VaultEntry _entry;

    /// <summary>The lamp.</summary>
    private readonly VaultLed _led;

    /// <summary>What the Vault is called.</summary>
    private readonly TextBlock _name;

    /// <summary>The address.</summary>
    private readonly TextBlock _address;

    /// <summary>The ring the chosen mark is drawn in.</summary>
    private readonly Border _radio;

    /// <summary>The fill of a chosen mark.</summary>
    private readonly Border _dot;

    /// <summary>The bar at the left of a chosen row.</summary>
    private readonly Border _accent;

    /// <summary>The colours in force.</summary>
    private Skin _skin;

    /// <summary>Whether this is the row chosen.</summary>
    private bool _chosen;

    /// <summary>Whether the pointer is over it.</summary>
    private bool _over;

    /// <summary>Makes one row.</summary>
    /// <param name="entry">The address.</param>
    /// <param name="chosen">Whether it is the row chosen.</param>
    /// <param name="skin">The colours in force.</param>
    /// <param name="choose">What choosing it does.</param>
    public VaultPick(VaultEntry entry, bool chosen, Skin skin, Action choose)
    {
        ArgumentNullException.ThrowIfNull(entry);
        ArgumentNullException.ThrowIfNull(skin);
        ArgumentNullException.ThrowIfNull(choose);

        _entry = entry;
        _skin = skin;
        _chosen = chosen;

        BorderThickness = new Thickness(0, 0, 0, 1);
        BorderBrush = skin.BorderSoft;
        Padding = new Thickness(11, 9, 11, 9);
        Cursor = new Cursor(StandardCursorType.Hand);
        Focusable = true;

        _led = new VaultLed();

        _name = new TextBlock
        {
            Text = entry.Name,
            FontSize = 13,
            FontWeight = FontWeight.SemiBold,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };

        _address = new TextBlock
        {
            Text = entry.Address,
            FontFamily = skin.Mono,
            FontSize = 11,
            Margin = new Thickness(0, 2, 0, 0),
            TextTrimming = TextTrimming.CharacterEllipsis,
        };

        var words = new StackPanel { VerticalAlignment = VerticalAlignment.Center };

        words.Children.Add(_name);
        words.Children.Add(_address);

        _dot = new Border
        {
            Width = 6,
            Height = 6,
            CornerRadius = new CornerRadius(3),
            Background = skin.Primary,
            IsVisible = chosen,
        };

        _radio = new Border
        {
            Width = 14,
            Height = 14,
            CornerRadius = new CornerRadius(7),
            BorderThickness = new Thickness(1),
            BorderBrush = chosen ? skin.Primary : skin.Splitter,
            Child = _dot,
        };

        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("16,*,18"),
            ColumnSpacing = 9,
        };

        row.Children.Add(_led);

        Grid.SetColumn(words, 1);
        row.Children.Add(words);

        Grid.SetColumn(_radio, 2);
        row.Children.Add(_radio);

        _accent = new Border
        {
            Width = 2,
            Background = skin.Primary,
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Stretch,
            IsVisible = chosen,
            IsHitTestVisible = false,
        };

        var root = new Panel();

        root.Children.Add(row);
        root.Children.Add(_accent);

        Child = root;

        PointerEntered += (_, _) =>
        {
            _over = true;
            Paint();
        };

        PointerExited += (_, _) =>
        {
            _over = false;
            Paint();
        };

        PointerReleased += (_, e) =>
        {
            ArgumentNullException.ThrowIfNull(e);

            if (e.InitialPressMouseButton != MouseButton.Left)
            {
                return;
            }

            e.Handled = true;
            choose();
        };

        KeyDown += (_, e) =>
        {
            ArgumentNullException.ThrowIfNull(e);

            if (e.Key is Key.Enter or Key.Space)
            {
                e.Handled = true;
                choose();
            }
        };

        Repaint(skin);
    }

    /// <summary>What the row is about.</summary>
    public VaultEntry Entry => _entry;

    /// <summary>Draws the row as chosen or not chosen.</summary>
    /// <param name="chosen">Whether it is the row chosen.</param>
    /// <param name="skin">The colours in force.</param>
    public void SetChosen(bool chosen, Skin skin)
    {
        ArgumentNullException.ThrowIfNull(skin);

        _chosen = chosen;
        _skin = skin;

        _dot.IsVisible = chosen;
        _radio.BorderBrush = chosen ? skin.Primary : skin.Splitter;
        _accent.IsVisible = chosen;

        Paint();
    }

    /// <summary>Draws the row in the colours of the variant in force.</summary>
    /// <param name="skin">The colours.</param>
    public void Repaint(Skin skin)
    {
        ArgumentNullException.ThrowIfNull(skin);

        _skin = skin;

        _name.Foreground = skin.Front;
        _address.Foreground = skin.Muted;
        _led.Show(_entry.Status, skin);

        var says = VaultSpeech.Tip(_entry);

        ToolTip.SetTip(this, says);
        AutomationProperties.SetName(this, $"{_entry.Name}, {VaultSpeech.Label(_entry.Status)}");

        Paint();
    }

    /// <summary>Draws the row as it stands.</summary>
    private void Paint()
    {
        Background = _chosen
            ? _skin.Selection
            : _over
                ? _skin.Hover
                : Brushes.Transparent;

        _radio.BorderBrush = _chosen ? _skin.Primary : _skin.Splitter;
    }
}
