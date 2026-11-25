using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Media.Imaging;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace SightPlugin;

/// <summary>
/// The previewer button in the preview's corner: one tool wearing the previewer's picture, opening a
/// folded list of every previewer offered.
/// </summary>
/// <remarks>
/// The control it hands on is a plain <see cref="ToggleButton"/>, not a subclass: the base theme
/// dresses a control by its exact type, so a subclass would not wear the rectangular, bordered tool
/// the Version Graph's corner buttons wear. This one is read as a button and never toggled, so the
/// check a press gives it is given back at once.
/// <para>
/// The picture says which previewer is in force and the tooltip says its name, and the list opens in
/// the way the File System's hidden-rules menu opens. Auto wears the picture of the previewer that
/// answered last, so a reader who has not forced one still sees what is about to draw.
/// </para>
/// </remarks>
internal sealed class SightPreviewerStrip
{
    /// <summary>What a mark is drawn in.</summary>
    private const string Ink = PaletteKeys.Front;

    /// <summary>How wide and tall the button's own mark is.</summary>
    private const double MarkSize = 14;

    /// <summary>How wide and tall a row's mark is.</summary>
    private const double RowMarkSize = 18;

    /// <summary>The host, whose pictures the list is drawn with.</summary>
    private readonly IPluginHost _host;

    /// <summary>What choosing a previewer does.</summary>
    private readonly Action<string?> _chosen;

    /// <summary>The button the corner holds.</summary>
    private readonly ToggleButton _button;

    /// <summary>The menu in force, so that choosing a row closes it.</summary>
    private Flyout? _flyout;

    /// <summary>The file chosen last, as the list answers what may be chosen.</summary>
    private SightFile? _primary;

    /// <summary>The other chosen file, when two are chosen.</summary>
    private SightFile? _second;

    /// <summary>The previewer the user forced, or nothing for Auto.</summary>
    private string? _current;

    /// <summary>The previewer that answered last, which Auto wears.</summary>
    private string? _drawn;

    /// <summary>Makes the button.</summary>
    /// <param name="host">The host.</param>
    /// <param name="chosen">What choosing a previewer does, `null` being Auto.</param>
    public SightPreviewerStrip(IPluginHost host, Action<string?> chosen)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(chosen);

        _host = host;
        _chosen = chosen;
        _button = new ToggleButton { Classes = { "tool" }, Padding = new Thickness(6) };
        _button.Click += (_, _) =>
        {
            _button.IsChecked = false;

            if (_flyout is { IsOpen: false } menu)
            {
                menu.ShowAt(_button);
            }
        };

        Paint();
    }

    /// <summary>The toggle the corner holds.</summary>
    public ToggleButton Button => _button;

    /// <summary>Draws the button against the files that are chosen now.</summary>
    /// <param name="primary">The file chosen last, or nothing.</param>
    /// <param name="second">The other chosen file, when two are chosen.</param>
    /// <param name="current">The previewer in force, or nothing for Auto.</param>
    public void Refresh(SightFile? primary, SightFile? second, string? current)
    {
        _primary = primary;
        _second = second;
        _current = current;
        _drawn = null;
        Paint();
    }

    /// <summary>Says which previewer answered, which is the picture Auto wears.</summary>
    /// <param name="id">The previewer's stable id, or nothing when none answered.</param>
    public void Drawn(string? id)
    {
        _drawn = string.IsNullOrEmpty(id) ? null : id;
        Paint();
    }

    /// <summary>Builds the list from the previewers offered now.</summary>
    /// <returns>The rows, one per choice.</returns>
    internal StackPanel BuildMenu()
    {
        var boxes = new StackPanel { Spacing = 2, Width = 240 };

        boxes.Children.Add(Row(null, "rorolala_sight.previewer.auto", Wheeled(), true));

        foreach (var previewer in SightPreviewers.All)
        {
            var enabled = _primary is not null && previewer.Cares(_primary, _second);

            boxes.Children.Add(
                Row(previewer.Id, previewer.NameKey, previewer.IconKey ?? Icons.Eye, enabled)
            );
        }

        return boxes;
    }

    /// <summary>The picture Auto wears: the previewer that answered, or the eye when none has.</summary>
    /// <returns>The picture's key.</returns>
    private string Wheeled() =>
        (_drawn is null ? null : SightPreviewers.ById(_drawn)?.IconKey) ?? Icons.Eye;

    /// <summary>Wears the picture and says the name of the previewer in force.</summary>
    private void Paint()
    {
        var previewer = _current is not null
            ? SightPreviewers.ById(_current)
            : _drawn is not null
                ? SightPreviewers.ById(_drawn)
                : null;
        var nameKey = previewer?.NameKey ?? "rorolala_sight.previewer.auto";

        _button.Content = Mark(previewer?.IconKey ?? Icons.Eye, MarkSize, Ink);
        ToolTip.SetTip(_button, RolaI18N.Get(nameKey));

        _flyout = new Flyout { Content = BuildMenu() };
        _button.Flyout = _flyout;
    }

    /// <summary>One row of the list, drawn as the File System's hidden-rules rows are.</summary>
    /// <param name="id">The previewer, or nothing for Auto.</param>
    /// <param name="nameKey">What the row says.</param>
    /// <param name="iconKey">What it wears.</param>
    /// <param name="enabled">Whether it may be chosen.</param>
    /// <returns>The row.</returns>
    private Button Row(string? id, string nameKey, string? iconKey, bool enabled)
    {
        var selected = string.Equals(_current, id, StringComparison.Ordinal);
        var glyph = Mark(
            iconKey ?? Icons.Eye,
            RowMarkSize,
            selected ? PaletteKeys.Primary : Ink
        );
        var label = new TextBlock
        {
            Text = RolaI18N.Get(nameKey),
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
            FontSize = 13,
        };
        label[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(
            selected ? PaletteKeys.Primary : PaletteKeys.Front
        );

        var content = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Children = { glyph, label },
        };
        var row = new Button
        {
            Content = content,
            Classes = { "ghost" },
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Left,
            Padding = new Thickness(8, 4),
            MinHeight = 28,
            IsEnabled = enabled,
        };

        row.Click += (_, _) =>
        {
            _flyout?.Hide();
            _chosen(id);
        };

        AutomationProperties.SetName(row, RolaI18N.Get(nameKey));

        return row;
    }

    /// <summary>One picture from the library, stamped in one of the look's inks.</summary>
    /// <param name="key">The picture's key.</param>
    /// <param name="size">How wide and tall it is.</param>
    /// <param name="ink">The resource the mask is filled with.</param>
    /// <returns>The mark, or an empty one when the library holds no such picture.</returns>
    private Border Mark(string key, double size, string ink)
    {
        if (_host.Icons.Find(key) is not { } picture)
        {
            return new Border { Width = size, Height = size };
        }

        // The mask is drawn whole rather than resampled with the toolkit's default filter: the
        // pictures are drawn larger than the marks that wear them, and a thin stroke reduced this way
        // loses its stem.
        var mark = new Border
        {
            Width = size,
            Height = size,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            OpacityMask = new ImageBrush(picture) { Stretch = Stretch.Uniform },
        };

        RenderOptions.SetBitmapInterpolationMode(mark, BitmapInterpolationMode.HighQuality);
        mark[!Border.BackgroundProperty] = new DynamicResourceExtension(ink);

        return mark;
    }
}
