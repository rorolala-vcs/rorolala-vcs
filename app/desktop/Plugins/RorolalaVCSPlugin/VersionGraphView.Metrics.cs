using System.Globalization;

using Avalonia.Controls;
using Avalonia.Media;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

internal sealed partial class VersionGraphView
{
    /// <summary>The colours the drawing is made of, in the look in force.</summary>
    private sealed record Ink(
        Color Ground,
        Color Primary,
        Color PrimaryText,
        Color Accent,
        Color AccentInk,
        Color Elevated,
        Color Front,
        Color Muted,
        Color Splitter,
        Color GridMajor,
        Color GridMinor,
        Color Add,
        Color Del
    );

    /// <summary>One version's label and bubble, laid out at the current zoom.</summary>
    private sealed record VersionMetrics(FormattedText Label, BubbleMetrics? Bubble);

    /// <summary>One variant's label, and how wide its capsule is.</summary>
    private sealed record VariantMetrics(FormattedText Label, double Width);

    /// <summary>One upstream version's label and bubble, laid out at the current zoom.</summary>
    private sealed record UpstreamMetrics(FormattedText Label, BubbleMetrics? Bubble);

    /// <summary>A version's words, laid out at the zoom in force.</summary>
    /// <remarks>
    /// The creator is a pill rather than a line of text: the pill is sized here as well, because
    /// the bubble's width and height are what it adds to the text it wraps.
    /// </remarks>
    private sealed record BubbleMetrics(
        FormattedText? Creator,
        FormattedText? Message,
        FormattedText? Hash,
        double RestWidth,
        double Width,
        double RestHeight,
        double PaddingX,
        double PaddingY,
        double Gap,
        double HashGap,
        double PillWidth,
        double PillHeight,
        double PillPaddingX,
        double PillPaddingY
    );

    /// <summary>What is measured once per zoom and look, so a drag does not lay text out again.</summary>
    private sealed class Metrics
    {
        /// <summary>Makes the metrics.</summary>
        public Metrics(
            double maxRight,
            IReadOnlyDictionary<ulong, VersionMetrics> versions,
            IReadOnlyDictionary<int, VariantMetrics> variants,
            IReadOnlyList<UpstreamMetrics> upstream
        )
        {
            MaxRight = maxRight;
            Versions = versions;
            Variants = variants;
            Upstream = upstream;
        }

        /// <summary>The furthest any node or bubble reaches, in screen units.</summary>
        public double MaxRight { get; }

        /// <summary>Each version's label and bubble.</summary>
        public IReadOnlyDictionary<ulong, VersionMetrics> Versions { get; }

        /// <summary>Each variant's label and capsule width.</summary>
        public IReadOnlyDictionary<int, VariantMetrics> Variants { get; }

        /// <summary>The upstream nodes' labels and bubbles, in the drawing's own order.</summary>
        public IReadOnlyList<UpstreamMetrics> Upstream { get; }
    }

    /// <summary>The colours in force, read once per look.</summary>
    private Ink CurrentInk()
    {
        if (_ink is not { } ink || _inkVersion != _themeVersion || !Matches(ink))
        {
            ink = ReadInk();
            _ink = ink;
            _inkVersion = _themeVersion;
        }

        return ink;
    }

    /// <summary>Whether an ink was made from the palette values in force now.</summary>
    /// <remarks>
    /// The look's resources raise a change of their own, but the primary, the accent and what is written on
    /// the primary are read through the live palette and a change to one of them is not something this
    /// control is told. Asking the palette again as the ink is taken is what makes a primary chosen in the
    /// preference panel the one the next drawing uses.
    /// </remarks>
    /// <param name="ink">The colours last made.</param>
    /// <returns>Whether they are still the ones the palette holds.</returns>
    private bool Matches(Ink ink) =>
        ink.Ground == _palette.Mid
        && ink.Primary == _palette.Primary
        && ink.PrimaryText == _palette.PrimaryText
        && ink.Accent == _palette.Accent
        && ink.Elevated == _palette.Bright
        && ink.Front == _palette.Front
        && ink.Splitter == _palette.Splitter;

    /// <summary>Reads the look's colours, and works out the two derived from it.</summary>
    private Ink ReadInk()
    {
        // The five a surface is made of, the primary, the accent and the ink on the primary are the live
        // palette's own, so a colour changed while the program runs is the next one this drawing is made of.
        // The two step colours of the ink are resources because the look publishes them and no role names
        // them.
        var ground = _palette.Mid;
        var primary = _palette.Primary;
        var accent = _palette.Accent;
        var front = _palette.Front;
        var light = ground.R + ground.G + ground.B > 3 * 0x80;

        return new Ink(
            ground,
            primary,
            _palette.PrimaryText,
            accent,
            light ? Mix(accent, front, 0.4) : accent,
            _palette.Bright,
            front,
            Colour(PaletteKeys.Muted, Color.FromRgb(0xAB, 0xAB, 0xAB)),
            _palette.Splitter,
            WithAlpha(accent, light ? (byte)26 : (byte)15),
            WithAlpha(accent, light ? (byte)13 : (byte)8),
            Colour(PaletteKeys.Add, Color.FromRgb(0x5F, 0xD0, 0x7A)),
            Colour(PaletteKeys.Del, Color.FromRgb(0xFF, 0x7A, 0x6B))
        );
    }

    /// <summary>The layout of the current drawing at the current zoom and look.</summary>
    private Metrics CurrentMetrics()
    {
        var ink = CurrentInk();

        if (
            _metrics is null
            || _metricsDrawing != _drawing
            || Math.Abs(_metricsZoom - _zoom) > double.Epsilon
            || _metricsTheme != _themeVersion
        )
        {
            _metrics = BuildMetrics(ink);
            _metricsDrawing = _drawing;
            _metricsZoom = _zoom;
            _metricsTheme = _themeVersion;
        }

        return _metrics;
    }

    /// <summary>Lays every label and bubble out.</summary>
    private Metrics BuildMetrics(Ink ink)
    {
        var drawing = _drawing ?? throw new InvalidOperationException("no drawing is laid out");
        var zoom = _zoom;
        var versions = new Dictionary<ulong, VersionMetrics>(drawing.Versions.Count);
        var maxRight =
            (drawing.VersionX + VersionGraphDrawing.Radius) * zoom;

        if (drawing.Upstream.Count > 0)
        {
            maxRight = Math.Max(
                maxRight,
                (drawing.UpstreamX + VersionGraphDrawing.Radius) * zoom
            );
        }

        foreach (var version in drawing.Versions)
        {
            var label = Text(version.Label, FaceSemiBold, 12 * zoom, ink.PrimaryText);
            var bubble = BuildBubble(
                version.Creator,
                version.Message,
                version.Hash,
                version.Origin,
                ink
            );

            versions[version.Level] = new VersionMetrics(label, bubble);

            if (bubble is not null)
            {
                var left =
                    (version.X
                        + VersionGraphDrawing.Radius
                        + VersionGraphDrawing.BubbleGap)
                    * zoom;

                maxRight = Math.Max(maxRight, left + bubble.Width);
            }
        }

        var variants = new Dictionary<int, VariantMetrics>(drawing.Variants.Count);

        foreach (var variant in drawing.Variants)
        {
            // The state a capsule carries is what it is coloured by, so an edit is read before its
            // label is: an unrecorded change is green, and one that must not be recorded is red.
            var colour = variant.Merging
                ? RorolalaColours.UnderWay
                : variant.State is null
                    ? ink.AccentInk
                    : variant.Wrong
                        ? ink.Del
                        : ink.Add;
            var label = Text(variant.Label, MonoMedium, 11 * zoom, colour);

            variants[variant.Index] = new VariantMetrics(
                label,
                label.Width + 18 * zoom
            );
        }

        var upstream = new List<UpstreamMetrics>(drawing.Upstream.Count);

        foreach (var one in drawing.Upstream)
        {
            var label = Text(one.Label, FaceSemiBold, 12 * zoom, ink.Muted);
            var bubble = one.Unknown
                ? null
                : BuildBubble(one.Creator, one.Message, one.Hash, false, ink);

            upstream.Add(new UpstreamMetrics(label, bubble));

            if (bubble is not null)
            {
                var left =
                    (one.X
                        + VersionGraphDrawing.Radius
                        + VersionGraphDrawing.BubbleGap)
                    * zoom;

                maxRight = Math.Max(maxRight, left + bubble.Width);
            }
        }

        return new Metrics(maxRight, versions, variants, upstream);
    }

    /// <summary>Lays a version's words out, or answers nothing when it has none.</summary>
    private BubbleMetrics? BuildBubble(
        string creatorText,
        string messageText,
        string hashText,
        bool origin,
        Ink ink
    )
    {
        if (creatorText.Length == 0 && messageText.Length == 0)
        {
            return null;
        }

        var zoom = _zoom;
        var paddingX = BubblePaddingX * zoom;
        var paddingY = BubblePaddingY * zoom;
        var contentWidth = BubbleMaxWidth * zoom - paddingX * 2;
        var creatorLabel = creatorText.Length > 0
            ? $"({creatorText})"
            : null;
        var creator = creatorLabel is null
            ? null
            : Text(
                creatorLabel,
                origin ? FaceSemiBold : FaceMedium,
                11.5 * zoom,
                ink.Muted
            );
        FormattedText? message = null;

        if (messageText.Length > 0)
        {
            var lineHeight = BubbleLineHeight * zoom;

            message = Text(
                messageText,
                origin ? FaceSemiBold : FaceNormal,
                13 * zoom,
                origin ? ink.Primary : ink.Front
            );
            message.MaxTextWidth = contentWidth;
            message.MaxTextHeight = 3 * lineHeight;
            message.LineHeight = lineHeight;
            message.Trimming = TextTrimming.CharacterEllipsis;
        }

        FormattedText? hash = null;

        if (hashText.Length > 0)
        {
            var lineHeight = BubbleHashLineHeight * zoom;

            hash = Text(hashText, MonoMedium, 10 * zoom, ink.Muted);
            hash.MaxTextWidth = contentWidth;
            hash.MaxTextHeight = 2 * lineHeight;
            hash.LineHeight = lineHeight;
            hash.Trimming = TextTrimming.CharacterEllipsis;
        }

        var creatorWidth = creator?.Width ?? 0;
        var messageWidth = message?.Width ?? 0;
        var hashWidth = hash?.Width ?? 0;
        var pillPaddingX = CreatorPillPaddingX * zoom;
        var pillPaddingY = CreatorPillPaddingY * zoom;
        var pillWidth = creator is null ? 0 : creatorWidth + pillPaddingX * 2;
        var pillHeight = creator is null ? 0 : creator.Height + pillPaddingY * 2;
        var content = Math.Max(pillWidth, Math.Max(messageWidth, hashWidth));
        var restContent = Math.Max(pillWidth, messageWidth);
        var maxWidth = BubbleMaxWidth * zoom;

        var messageHeight = message?.Height ?? 0;
        var gap = creator is not null && message is not null ? 2 * zoom : 0;
        var hashGap =
            hash is not null && (creator is not null || message is not null)
                ? 3 * zoom
                : 0;

        return new BubbleMetrics(
            creator,
            message,
            hash,
            Math.Min(maxWidth, restContent + paddingX * 2),
            Math.Min(maxWidth, content + paddingX * 2),
            paddingY * 2 + pillHeight + gap + messageHeight,
            paddingX,
            paddingY,
            gap,
            hashGap,
            pillWidth,
            pillHeight,
            pillPaddingX,
            pillPaddingY
        );
    }

    /// <summary>Makes one line of text in one face and one colour.</summary>
    private static FormattedText Text(
        string text,
        Typeface typeface,
        double size,
        Color colour
    ) =>
        new(
            text,
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            typeface,
            size,
            new SolidColorBrush(colour)
        );

    /// <summary>Reads a colour the look publishes, or the stand-in where it publishes none.</summary>
    private Color Colour(string key, Color fallback)
    {
        var top = TopLevel.GetTopLevel(this);

        return top?.TryFindResource(key, null, out var found) == true
            ? found switch
            {
                ISolidColorBrush brush => brush.Color,
                Color colour => colour,
                _ => fallback,
            }
            : fallback;
    }

    /// <summary>Mixes two colours, `amount` of the way from the first to the second.</summary>
    private static Color Mix(Color from, Color to, double amount) =>
        Color.FromRgb(
            (byte)Math.Round(from.R + (to.R - from.R) * amount),
            (byte)Math.Round(from.G + (to.G - from.G) * amount),
            (byte)Math.Round(from.B + (to.B - from.B) * amount)
        );

    /// <summary>The same colour, at one alpha.</summary>
    private static Color WithAlpha(Color colour, byte alpha) =>
        Color.FromArgb(alpha, colour.R, colour.G, colour.B);
}
