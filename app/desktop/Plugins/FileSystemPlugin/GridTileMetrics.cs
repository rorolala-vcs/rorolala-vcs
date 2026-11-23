using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;

namespace FileSystemPlugin;

/// <summary>
/// The width and height a grid tile is laid out at, without a tile being made.
/// </summary>
/// <remarks>
/// A virtualizing panel has to know how tall every tile is before it realizes the ones it draws, and a
/// tile's height is its picture, the room around it, and how many lines its name takes. The picture and
/// the room are the same for every tile and are calibrated once from a container that was made for
/// real; the name is measured with a block that wears the same classes as a caption, so the answer is
/// the one the real caption would have given.
/// <para>
/// The names are kept by the entry's key, so a read that leaves a name alone leaves its height alone.
/// </para>
/// </remarks>
/// <typeparam name="TItem">What one entry is.</typeparam>
internal sealed class GridTileMetrics<TItem>
    where TItem : class
{
    /// <summary>The block a name is measured with, which is a caption that is not drawn.</summary>
    private readonly TextBlock _measure;

    /// <summary>What an entry is called.</summary>
    private readonly Func<TItem, string> _name;

    /// <summary>What tells one entry from another.</summary>
    private readonly Func<TItem, object> _key;

    /// <summary>The name heights measured so far, by the entry's key.</summary>
    private readonly Dictionary<object, double> _heights = [];

    /// <summary>What every tile has besides its name: the picture, the room, the border and the margin.</summary>
    private double _chrome;

    /// <summary>The width a tile is arranged at.</summary>
    private double _width;

    /// <summary>Makes the metrics over the block a name is measured with.</summary>
    /// <param name="measure">A caption that is never drawn, wearing the caption's classes.</param>
    /// <param name="icon">How wide and tall a tile's picture is.</param>
    /// <param name="room">How much room is left around the picture.</param>
    /// <param name="name">What an entry is called.</param>
    /// <param name="key">What tells one entry from another.</param>
    public GridTileMetrics(
        TextBlock measure,
        double icon,
        double room,
        Func<TItem, string> name,
        Func<TItem, object> key
    )
    {
        ArgumentNullException.ThrowIfNull(measure);
        ArgumentNullException.ThrowIfNull(name);
        ArgumentNullException.ThrowIfNull(key);

        _measure = measure;
        _measure.MaxWidth = icon;
        _measure.TextWrapping = TextWrapping.Wrap;
        _name = name;
        _key = key;
        _width = icon + (room * 2);
    }

    /// <summary>The width a tile is arranged at.</summary>
    internal double Width => _width;

    /// <summary>How tall one entry's tile is, in the listing's own units.</summary>
    /// <param name="item">The entry.</param>
    /// <returns>The height.</returns>
    internal double HeightOf(TItem item)
    {
        var key = _key(item);

        if (_heights.TryGetValue(key, out var known))
        {
            return known;
        }

        var height = _chrome + Caption(item);

        _heights[key] = height;

        return height;
    }

    /// <summary>Takes the fixed part of a tile from one that was made for real.</summary>
    /// <param name="item">The container, as it was measured.</param>
    /// <param name="source">The entry the container stands for.</param>
    /// <param name="caption">The caption the container drew, whose font measures names.</param>
    internal void Calibrate(Control item, TItem source, TextBlock caption)
    {
        ArgumentNullException.ThrowIfNull(item);
        ArgumentNullException.ThrowIfNull(source);
        ArgumentNullException.ThrowIfNull(caption);

        _measure.FontFamily = caption.FontFamily;
        _measure.FontSize = caption.FontSize;
        _measure.FontWeight = caption.FontWeight;
        _measure.FontStyle = caption.FontStyle;
        _measure.FontStretch = caption.FontStretch;
        _measure.LineHeight = caption.LineHeight;

        _width = item.DesiredSize.Width;
        _chrome = item.DesiredSize.Height - Caption(source);
    }

    /// <summary>Lets go of the heights measured for a listing that is not the one on screen any more.</summary>
    internal void Forget() => _heights.Clear();

    /// <summary>How many lines one entry's name takes, in the exact height it takes them.</summary>
    /// <param name="item">The entry.</param>
    /// <returns>The height of the name.</returns>
    private double Caption(TItem item)
    {
        _measure.Text = _name(item);
        _measure.Measure(new Size(_measure.MaxWidth, double.PositiveInfinity));

        return _measure.DesiredSize.Height;
    }
}
