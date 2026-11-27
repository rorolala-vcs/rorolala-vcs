using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// The pieces the Workspace dock is built from: the square button, the mark on it, and the wash a row
/// takes under the pointer.
/// </summary>
/// <remarks>
/// The sizes are the Overview's, since the two docks are read side by side: a twenty-four pixel button
/// with a fourteen pixel mark, a thirty pixel row, and the same wash. The buttons are drawn square and
/// with a line of their own because a row here is a row of actions rather than a card, which is the
/// same answer the Overview's rows make.
/// </remarks>
internal static class WorkspaceFurniture
{
    /// <summary>How wide and tall a square button is.</summary>
    public const double SquareSize = 24;

    /// <summary>How large the mark on a square button is.</summary>
    public const double GlyphSize = 14;

    /// <summary>How tall a rule or an operation row is at least.</summary>
    public const double RowHeight = 30;

    /// <summary>The wash a row takes under the pointer.</summary>
    /// <remarks>
    /// An ink laid thin rather than a colour of its own, so the same wash reads on both looks: white
    /// over the dark one and black over the light one, both at a strength the row's words still read
    /// on.
    /// </remarks>
    public static Color Wash =>
        RorolalaColours.DarkLook
            ? Color.FromArgb(0x14, 0xFF, 0xFF, 0xFF)
            : Color.FromArgb(0x14, 0x00, 0x00, 0x00);

    /// <summary>A picture drawn as a mask over an ink the look publishes.</summary>
    /// <param name="host">The host, whose library holds the picture.</param>
    /// <param name="key">The picture's key.</param>
    /// <param name="ink">The resource name of the ink it is drawn in.</param>
    /// <param name="size">How large it is.</param>
    /// <returns>The mark, which is an empty block where the library holds no picture.</returns>
    public static Border Glyph(IPluginHost host, string key, string ink, double size)
    {
        ArgumentNullException.ThrowIfNull(host);

        var mark = new Border
        {
            Width = size,
            Height = size,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        if (host.Icons.Find(key) is { } picture)
        {
            mark.OpacityMask = new ImageBrush(picture) { Stretch = Stretch.Uniform };
            RenderOptions.SetBitmapInterpolationMode(mark, BitmapInterpolationMode.HighQuality);
        }

        mark[!Border.BackgroundProperty] = new DynamicResourceExtension(ink);

        return mark;
    }

    /// <summary>A picture drawn as a mask over a colour value.</summary>
    /// <param name="host">The host, whose library holds the picture.</param>
    /// <param name="key">The picture's key.</param>
    /// <param name="ink">The colour it is drawn in.</param>
    /// <param name="size">How large it is.</param>
    /// <returns>The mark.</returns>
    public static Border Glyph(IPluginHost host, string key, Color ink, double size)
    {
        var mark = Glyph(host, key, PaletteKeys.Front, size);
        mark.Background = new SolidColorBrush(ink);

        return mark;
    }

    /// <summary>
    /// The square button a row offers.
    /// </summary>
    /// <remarks>
    /// It is always drawn, rather than waiting for the pointer: what a row can be done to is read at
    /// a glance, and a row that had to be pointed at to be read would be a row read twice. The
    /// destructive one wears the error colour and a wash of it, which is the one colour the set
    /// states for something that takes work away.
    /// </remarks>
    /// <param name="host">The host, whose library holds the mark and whose palette holds the colours.</param>
    /// <param name="iconKey">The mark's key.</param>
    /// <param name="tipKey">The i18n key of what the pointer says.</param>
    /// <param name="press">What pressing it does.</param>
    /// <param name="danger">Whether it is the destructive one.</param>
    /// <returns>The button.</returns>
    public static Button Square(
        IPluginHost host,
        string iconKey,
        string tipKey,
        Action press,
        bool danger = false
    )
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(press);

        var button = new Button
        {
            Content = Glyph(host, iconKey, danger ? RorolalaColours.Error : RorolalaColours.Front, GlyphSize),
            Width = SquareSize,
            Height = SquareSize,
            Padding = new Thickness(0),
            HorizontalContentAlignment = HorizontalAlignment.Center,
            VerticalContentAlignment = VerticalAlignment.Center,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(0),
            VerticalAlignment = VerticalAlignment.Center,
        };

        void Paint(bool over)
        {
            if (danger)
            {
                button[!TemplatedControl.BackgroundProperty] = new DynamicResourceExtension(
                    over ? PaletteKeys.DelWash : PaletteKeys.DelBackground
                );
                button[!TemplatedControl.BorderBrushProperty] = new DynamicResourceExtension(
                    PaletteKeys.Del
                );

                return;
            }

            if (over)
            {
                button.Background = new SolidColorBrush(Wash);
            }
            else
            {
                button[!TemplatedControl.BackgroundProperty] = new DynamicResourceExtension(
                    PaletteKeys.Dark
                );
            }

            button[!TemplatedControl.BorderBrushProperty] = new DynamicResourceExtension(
                over ? PaletteKeys.Muted : PaletteKeys.Splitter
            );
        }

        Paint(false);

        button.PointerEntered += (_, _) => Paint(true);
        button.PointerExited += (_, _) => Paint(false);
        button.Click += (_, _) => press();

        if (!string.IsNullOrEmpty(tipKey))
        {
            ToolTip.SetTip(button, RolaI18N.Get(tipKey));
            ToolTip.SetShowDelay(button, 0);
        }

        return button;
    }

    /// <summary>A word on the look's dark face, which is how a header's own action is drawn.</summary>
    /// <param name="labelKey">The i18n key of the word.</param>
    /// <param name="width">How wide it is, or nothing to take the room its word needs.</param>
    /// <param name="press">What pressing it does.</param>
    /// <returns>The button.</returns>
    public static Button Compact(string labelKey, double? width, Action press)
    {
        ArgumentNullException.ThrowIfNull(press);

        var words = new TextBlock
        {
            Text = RolaI18N.Get(labelKey),
            FontSize = 11,
            VerticalAlignment = VerticalAlignment.Center,
        };
        words[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Front);

        var button = new Button
        {
            Content = words,
            Height = SquareSize,
            MinWidth = SquareSize,
            Width = width ?? double.NaN,
            Padding = new Thickness(10, 0),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(0),
            HorizontalContentAlignment = HorizontalAlignment.Center,
            VerticalContentAlignment = VerticalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };

        void Paint(bool over)
        {
            if (over)
            {
                button.Background = new SolidColorBrush(Wash);
            }
            else
            {
                button[!TemplatedControl.BackgroundProperty] = new DynamicResourceExtension(
                    PaletteKeys.Dark
                );
            }

            button[!TemplatedControl.BorderBrushProperty] = new DynamicResourceExtension(
                over ? PaletteKeys.Muted : PaletteKeys.Splitter
            );
        }

        Paint(false);

        button.PointerEntered += (_, _) => Paint(true);
        button.PointerExited += (_, _) => Paint(false);
        button.Click += (_, _) => press();

        return button;
    }

    /// <summary>The small loud label a pane is headed by.</summary>
    /// <param name="key">The i18n key of the words.</param>
    /// <returns>The label.</returns>
    public static TextBlock Label(string key)
    {
        var label = new TextBlock
        {
            Text = RolaI18N.Get(key).ToUpperInvariant(),
            FontSize = 11,
            FontWeight = FontWeight.SemiBold,
            LetterSpacing = 0.9,
            VerticalAlignment = VerticalAlignment.Center,
        };
        label[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Front);

        return label;
    }

    /// <summary>The number a pane's heading counts.</summary>
    /// <param name="count">The number.</param>
    /// <returns>The count, in the quiet ink.</returns>
    public static TextBlock Count(int count)
    {
        var number = new TextBlock
        {
            Text = count.ToString(System.Globalization.CultureInfo.InvariantCulture),
            FontSize = 11,
            VerticalAlignment = VerticalAlignment.Center,
        };
        number[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(PaletteKeys.Muted);

        return number;
    }

    /// <summary>Gives a row the wash it takes under the pointer.</summary>
    /// <param name="row">The row.</param>
    public static void Hover(Border row)
    {
        ArgumentNullException.ThrowIfNull(row);

        row.PointerEntered += (_, _) => row.Background = new SolidColorBrush(Wash);
        row.PointerExited += (_, _) => row.Background = Brushes.Transparent;
    }
}
