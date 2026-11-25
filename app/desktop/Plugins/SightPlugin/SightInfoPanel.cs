using System.Globalization;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace SightPlugin;

/// <summary>
/// The table in the preview's corner: what the preview is made of.
/// </summary>
/// <remarks>
/// It is drawn as the Version Graph's hover table is drawn, and stands in the same corner: a card over
/// the preview with a title and a row per thing said about it, so the drawing keeps the whole pane and
/// the corner is where a reader asks what it is. What stands here is what
/// <see cref="SightMetadataProviders"/> answered, one row per item, and the panel is only on screen
/// while the pointer rests on the corner, which is what the eye in that corner is for.
/// </remarks>
internal sealed class SightInfoPanel : Control
{
    /// <summary>How wide the table stands.</summary>
    private const double PanelWidth = 300;

    /// <summary>The room the table keeps inside its own edge.</summary>
    private const double PanelPadding = 12;

    /// <summary>The column the labels stand in.</summary>
    private const double LabelWidth = 64;

    /// <summary>The height of one row.</summary>
    private const double RowHeight = 19;

    /// <summary>The room the title keeps above the first row.</summary>
    private const double TitleHeight = 26;

    /// <summary>How far the shadow falls below the card.</summary>
    private const double ShadowDrop = 4;

    /// <summary>The colours the table is drawn in.</summary>
    private readonly SightInk _ink;

    /// <summary>What is written over the rows.</summary>
    private string _title = string.Empty;

    /// <summary>What the rows say.</summary>
    private IReadOnlyList<SightMetadataItem> _rows = [];

    /// <summary>Makes the empty table, standing in the corner it is drawn in.</summary>
    /// <param name="host">The host, whose palette states the colours.</param>
    public SightInfoPanel(IPluginHost host)
    {
        ArgumentNullException.ThrowIfNull(host);

        _ink = SightInk.From(host.Palette);
        Width = PanelWidth;
        HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Right;
        VerticalAlignment = Avalonia.Layout.VerticalAlignment.Bottom;
        Margin = new Thickness(0, 0, SightView.Corner, SightView.Corner);
        IsHitTestVisible = false;
        IsVisible = false;
    }

    /// <summary>Puts what the preview says on screen.</summary>
    /// <param name="title">What is written over the rows.</param>
    /// <param name="rows">The label and value of each row.</param>
    public void Show(string title, IReadOnlyList<SightMetadataItem> rows)
    {
        ArgumentNullException.ThrowIfNull(title);
        ArgumentNullException.ThrowIfNull(rows);

        _title = title;
        _rows = rows;
        Height = TitleHeight + (rows.Count * RowHeight) + 10 + ShadowDrop;
        IsVisible = true;
        InvalidateVisual();
    }

    /// <summary>Takes the table off screen.</summary>
    public void Hide() => IsVisible = false;

    /// <inheritdoc />
    public override void Render(DrawingContext context)
    {
        ArgumentNullException.ThrowIfNull(context);

        if (_title.Length == 0 && _rows.Count == 0)
        {
            return;
        }

        var width = PanelWidth;
        var height = Bounds.Height - ShadowDrop;

        context.DrawRectangle(
            new SolidColorBrush(_ink.Shadow),
            null,
            new Rect(0, ShadowDrop, width, height),
            8,
            8
        );
        context.DrawRectangle(
            new SolidColorBrush(Ink.Alpha(_ink.Card, 242)),
            new Pen(new SolidColorBrush(_ink.CardEdge), 1),
            new Rect(0, 0, width, height),
            8,
            8
        );

        var title = new FormattedText(
            _title,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            new Typeface(FontFamily.Default, FontStyle.Normal, FontWeight.SemiBold),
            12.5,
            new SolidColorBrush(_ink.Accent)
        );

        context.DrawText(title, new Point(PanelPadding, 9));
        var rowsTop = TitleHeight;

        for (var index = 0; index < _rows.Count; index++)
        {
            var item = _rows[index];
            var label = RolaI18N.Get(item.LabelKey);
            var top = rowsTop + (index * RowHeight);
            var valueLeft = label.Length > 0 ? PanelPadding + LabelWidth : PanelPadding;
            var valueWidth = label.Length > 0
                ? width - (PanelPadding * 2) - LabelWidth
                : width - (PanelPadding * 2);

            if (label.Length > 0)
            {
                var said = new FormattedText(
                    label,
                    CultureInfo.InvariantCulture,
                    FlowDirection.LeftToRight,
                    new Typeface(FontFamily.Default),
                    10.5,
                    new SolidColorBrush(_ink.Faint)
                );

                context.DrawText(
                    said,
                    new Point(PanelPadding, top + ((RowHeight - said.Height) / 2))
                );
            }

            var text = new FormattedText(
                item.Value,
                CultureInfo.InvariantCulture,
                FlowDirection.LeftToRight,
                new Typeface(FontFamily.Default),
                11,
                new SolidColorBrush(label.Length > 0 ? _ink.Text : _ink.Muted)
            )
            {
                MaxTextWidth = Math.Max(1, valueWidth),
                MaxLineCount = 1,
                Trimming = TextTrimming.CharacterEllipsis,
            };

            context.DrawText(text, new Point(valueLeft, top + ((RowHeight - text.Height) / 2)));
        }
    }

    /// <summary>The colours the table is drawn in, read from the look the way the graph's are.</summary>
    /// <param name="Card">What the card is filled with.</param>
    /// <param name="CardEdge">Its border.</param>
    /// <param name="Text">A value's ink.</param>
    /// <param name="Muted">A value with no label beside it.</param>
    /// <param name="Faint">A label's ink.</param>
    /// <param name="Shadow">The shadow under the card.</param>
    /// <param name="Accent">The title's ink.</param>
    private sealed record SightInk(
        Color Card,
        Color CardEdge,
        Color Text,
        Color Muted,
        Color Faint,
        Color Shadow,
        Color Accent
    )
    {
        /// <summary>Reads the colours the look states for a card in a corner.</summary>
        /// <param name="palette">The look's own colours.</param>
        /// <returns>The colours.</returns>
        public static SightInk From(IPalette palette)
        {
            ArgumentNullException.ThrowIfNull(palette);

            var ground = palette.Dark;
            var card = palette.Bright;
            var front = palette.Front;
            var splitter = palette.Splitter;
            var light = (ground.R + ground.G + ground.B) > (3 * 0x80);

            return new SightInk(
                Card: card,
                CardEdge: Ink.Mix(splitter, front, 0.22),
                Text: front,
                Muted: Ink.Mix(front, ground, 0.45),
                Faint: Ink.Mix(front, ground, 0.28),
                Shadow: Color.FromArgb(light ? (byte)26 : (byte)110, 0, 0, 0),
                Accent: palette.Primary
            );
        }
    }

    /// <summary>The two colour answers the card is drawn with.</summary>
    private static class Ink
    {
        /// <summary>A colour part way from one to another.</summary>
        /// <param name="from">The colour at zero.</param>
        /// <param name="to">The colour at one.</param>
        /// <param name="part">How far to move, from 0 to 1.</param>
        /// <returns>The mixed colour, opaque.</returns>
        public static Color Mix(Color from, Color to, double part) =>
            Color.FromRgb(
                (byte)Math.Round(from.R + ((to.R - from.R) * part)),
                (byte)Math.Round(from.G + ((to.G - from.G) * part)),
                (byte)Math.Round(from.B + ((to.B - from.B) * part))
            );

        /// <summary>The colour with another alpha.</summary>
        /// <param name="colour">The colour.</param>
        /// <param name="alpha">The alpha it is to have.</param>
        /// <returns>The colour.</returns>
        public static Color Alpha(Color colour, byte alpha) =>
            Color.FromArgb(alpha, colour.R, colour.G, colour.B);
    }
}
