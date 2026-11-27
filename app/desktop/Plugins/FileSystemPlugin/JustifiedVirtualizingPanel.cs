using System.Collections.Specialized;

using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Generators;
using Avalonia.Input;
using Avalonia.Layout;

namespace FileSystemPlugin;

/// <summary>
/// What a grid's panel asks about the items it lays out: how wide a tile is, how tall one is, and how
/// the fixed part of a tile is taken from one that was made for real.
/// </summary>
internal interface IGridTileSource
{
    /// <summary>The width a tile is arranged at.</summary>
    double TileWidth { get; }

    /// <summary>How tall the tile at an index is.</summary>
    /// <param name="index">The item's place in the listing.</param>
    /// <returns>The height.</returns>
    double HeightOf(int index);

    /// <summary>Takes the fixed part of a tile from a container that was made for real.</summary>
    /// <param name="item">The container, as it was measured.</param>
    /// <param name="index">The item's place in the listing.</param>
    void Calibrate(Control item, int index);
}

/// <summary>
/// A grid whose items fall into the justified lines of <see cref="JustifiedLayout"/>, realizing only the
/// lines the viewport holds.
/// </summary>
/// <remarks>
/// The panel reports the whole extent as its desired size and realizes the containers of the lines near
/// the viewport, which the scroll viewer moves physically. A line's height comes from
/// <see cref="IGridTileSource"/>, so the extent is known before a container is made; the first container
/// that is made calibrates the fixed part of the height, and every later measure is exact.
/// <para>
/// A container is prepared through the toolkit's <see cref="ItemContainerGenerator"/> and added with
/// <see cref="VirtualizingPanel.AddInternalChild"/>. Containers that leave the window are cleared and
/// removed rather than kept in a pool, because a window is a screenful and the work of making one is
/// the work of the few items that arrived.
/// </para>
/// </remarks>
internal sealed class JustifiedVirtualizingPanel : VirtualizingPanel
{
    /// <summary>How many lines are realized before the viewport has said where it is.</summary>
    private const int OpeningLines = 3;

    /// <summary>The containers on screen, by the item's place in the listing.</summary>
    private readonly Dictionary<int, Control> _realized = [];

    /// <summary>The items whose own control is the container, which cannot be cleared.</summary>
    private readonly HashSet<int> _owned = [];

    /// <summary>The lines as they stood at the last measure.</summary>
    private IReadOnlyList<JustifiedLayout.Line> _lines = [];

    /// <summary>Where each line begins, in the panel's own coordinates.</summary>
    private double[] _tops = [];

    /// <summary>The width the lines were measured at.</summary>
    private double _width;

    /// <summary>The part of the panel the viewport holds, in its own coordinates.</summary>
    private Rect _viewport;

    /// <summary>Whether a container has calibrated the fixed part of a tile's height.</summary>
    private bool _calibrated;

    /// <summary>Makes the panel, and listens for the viewport that decides what is realized.</summary>
    public JustifiedVirtualizingPanel() =>
        EffectiveViewportChanged += (_, changed) =>
        {
            if (changed.EffectiveViewport == _viewport)
            {
                return;
            }

            _viewport = changed.EffectiveViewport;
            InvalidateMeasure();
        };

    /// <summary>The room left between two tiles on one line.</summary>
    public double Gap { get; set; } = 12;

    /// <summary>What the panel asks about the items it lays out.</summary>
    internal IGridTileSource? Source { get; set; }

    /// <summary>How many tiles the first line holds, which is the grid's column count.</summary>
    internal int Columns { get; private set; } = 1;

    /// <inheritdoc />
    protected override Size MeasureOverride(Size availableSize)
    {
        var width = availableSize.Width;

        if (double.IsInfinity(width) || width <= 0)
        {
            width = Math.Max(1, Source?.TileWidth ?? 1);
        }

        if (Source is null || Items.Count == 0)
        {
            Clear();

            return default;
        }

        if (!_calibrated && Ensure(0) is { } probe)
        {
            probe.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
            Source.Calibrate(probe, 0);
            _calibrated = true;
        }

        var sizes = new Func<int, Size>(index => new Size(Source.TileWidth, Source.HeightOf(index)));

        // Every line's height is rounded up to the pixels the window draws at, so that two lines share the
        // edge they meet at rather than one of them covering a pixel of the other. The faces of two cells
        // beside each other are laid thin enough to see through, and two of them over one pixel read as a
        // line brighter than either, which is what a fraction of a pixel of overlap is.
        var scale = TopLevel.GetTopLevel(this)?.RenderScaling ?? 1.0;

        _lines =
        [
            .. JustifiedLayout
                .Lines(Items.Count, sizes, width, Gap)
                .Select(line => line with { Height = LayoutHelper.RoundLayoutValueUp(line.Height, scale) }),
        ];
        _width = width;
        Columns = _lines.Count == 0 ? 1 : _lines[0].Count;
        _tops = new double[_lines.Count];

        var height = 0.0;

        for (var at = 0; at < _lines.Count; at++)
        {
            _tops[at] = height;
            height += _lines[at].Height;
        }

        RealizeWindow();

        return new Size(width, height);
    }

    /// <inheritdoc />
    protected override Size ArrangeOverride(Size finalSize)
    {
        if (Source is null || _lines.Count == 0)
        {
            return finalSize;
        }

        var y = 0.0;
        var step = Gap;
        var scale = TopLevel.GetTopLevel(this)?.RenderScaling ?? 1.0;

        for (var lineIndex = 0; lineIndex < _lines.Count; lineIndex++)
        {
            var line = _lines[lineIndex];

            if (lineIndex < _lines.Count - 1)
            {
                step = line.Count > 1
                    ? Gap + ((finalSize.Width - line.Width) / (line.Count - 1))
                    : Gap;

                // Rounded to the pixels the window draws at, so that two cells of a line start and end on
                // the same edge rather than one of them covering a pixel of the other. A face is thin
                // enough to see through, and two of them over one pixel are a brighter line between them.
                step = LayoutHelper.RoundLayoutValue(step, scale);
            }

            var x = 0.0;

            // The room between two tiles belongs to the cell in front of it: the face a tile is washed
            // with under the pointer and while it is chosen fills the cell, so the cells of a line meet
            // at their edges rather than leaving a strip of the ground between them.
            var cell = Source.TileWidth + step;

            for (var at = line.Start; at < line.Start + line.Count; at++)
            {
                if (_realized.TryGetValue(at, out var child))
                {
                    // Every cell of a line is the line's height, not the tile's own: a short caption
                    // would otherwise leave the chosen wash a different height from the tile beside it.
                    // The last cell ends where the line or the row does rather than one room past it.
                    var width = Math.Min(cell, Math.Max(0, finalSize.Width - x));

                    child.Arrange(new Rect(x, y, width, line.Height));
                }

                x += cell;
            }

            y += line.Height;
        }

        return finalSize;
    }

    /// <inheritdoc />
    protected override Control? ScrollIntoView(int index)
    {
        if (Source is null || index < 0 || index >= Items.Count || _lines.Count == 0)
        {
            return null;
        }

        var line = LineOf(index);

        if (line < 0)
        {
            return null;
        }

        Ensure(index);

        ControlExtensions.BringIntoView(
            this,
            new Rect(0, _tops[line], Math.Max(1, _width), _lines[line].Height)
        );

        return ContainerFromIndex(index);
    }

    /// <inheritdoc />
    protected override Control? ContainerFromIndex(int index) =>
        _realized.TryGetValue(index, out var known) ? known : null;

    /// <summary>The container for an item, for a test that cannot reach the protected member.</summary>
    /// <param name="index">The item's place in the listing.</param>
    /// <returns>The container, or nothing when it is not realized.</returns>
    internal Control? RealizedAt(int index) => ContainerFromIndex(index);

    /// <summary>Brings an item into view, for a test that cannot reach the protected member.</summary>
    /// <param name="index">The item's place in the listing.</param>
    /// <returns>The container, or nothing when it could not be made.</returns>
    internal Control? ScrollTo(int index) => ScrollIntoView(index);

    /// <summary>The control a direction reaches, for a test that cannot reach the protected member.</summary>
    /// <param name="direction">Where the movement goes.</param>
    /// <param name="from">The control it begins at.</param>
    /// <param name="wrap">Whether it wraps around at the ends.</param>
    /// <returns>The control reached, or nothing.</returns>
    internal IInputElement? Move(NavigationDirection direction, Control from, bool wrap) =>
        GetControl(direction, from, wrap);

    /// <inheritdoc />
    protected override int IndexFromContainer(Control container)
    {
        ArgumentNullException.ThrowIfNull(container);

        foreach (var (index, held) in _realized)
        {
            if (ReferenceEquals(held, container))
            {
                return index;
            }
        }

        return -1;
    }

    /// <inheritdoc />
    protected override IEnumerable<Control> GetRealizedContainers() => _realized.Values;

    /// <inheritdoc />
    protected override IInputElement? GetControl(
        NavigationDirection direction,
        IInputElement? from,
        bool wrap
    )
    {
        if (from is not Control control || Items.Count == 0)
        {
            return null;
        }

        var index = IndexFromContainer(control);

        if (index < 0)
        {
            return null;
        }

        var columns = Math.Max(1, Columns);
        var target = direction switch
        {
            NavigationDirection.Left or NavigationDirection.Previous => index - 1,
            NavigationDirection.Right or NavigationDirection.Next => index + 1,
            NavigationDirection.Up => index - columns,
            NavigationDirection.Down => index + columns,
            NavigationDirection.First => 0,
            NavigationDirection.Last => Items.Count - 1,
            NavigationDirection.PageUp => index - (columns * 4),
            NavigationDirection.PageDown => index + (columns * 4),
            _ => -1,
        };

        if (wrap)
        {
            target = ((target % Items.Count) + Items.Count) % Items.Count;
        }

        if (target < 0 || target >= Items.Count)
        {
            return null;
        }

        ScrollIntoView(target);

        return ContainerFromIndex(target);
    }

    /// <inheritdoc />
    protected override void OnItemsChanged(
        IReadOnlyList<object?> items,
        NotifyCollectionChangedEventArgs e
    )
    {
        ArgumentNullException.ThrowIfNull(e);

        switch (e.Action)
        {
            case NotifyCollectionChangedAction.Replace when e.OldStartingIndex >= 0:
                Unrealize(e.OldStartingIndex);

                break;

            case NotifyCollectionChangedAction.Move
                when e.OldStartingIndex >= 0 && e.NewStartingIndex >= 0:
                Relabel(index =>
                    index == e.OldStartingIndex
                        ? e.NewStartingIndex
                        : MoveWithin(index, e.OldStartingIndex, e.NewStartingIndex)
                );

                break;

            case NotifyCollectionChangedAction.Add when e.NewStartingIndex >= 0:
                Relabel(index => index >= e.NewStartingIndex ? index + 1 : index);

                break;

            case NotifyCollectionChangedAction.Remove when e.OldStartingIndex >= 0:
                Unrealize(e.OldStartingIndex);
                Relabel(index => index > e.OldStartingIndex ? index - 1 : index);

                break;

            default:
                Clear();

                break;
        }

        base.OnItemsChanged(items, e);
        InvalidateMeasure();
    }

    /// <summary>Where an item stands after one item was moved over it.</summary>
    /// <param name="index">The item's place before the move.</param>
    /// <param name="from">Where the moved item stood.</param>
    /// <param name="to">Where it stands now.</param>
    /// <returns>The item's place after the move.</returns>
    private static int MoveWithin(int index, int from, int to) =>
        from < to && index > from && index <= to
            ? index - 1
            : to < from && index >= to && index < from
                ? index + 1
                : index;

    /// <summary>Gives every realized container the place its item now has.</summary>
    /// <param name="map">Where an old place is now.</param>
    private void Relabel(Func<int, int> map)
    {
        var moved = new Dictionary<int, Control>(_realized.Count);

        foreach (var (index, container) in _realized)
        {
            moved[map(index)] = container;
        }

        _realized.Clear();

        foreach (var (index, container) in moved)
        {
            _realized[index] = container;
        }

        var owned = _owned.Select(map).ToHashSet();

        _owned.Clear();

        foreach (var index in owned)
        {
            _owned.Add(index);
        }
    }

    /// <summary>Realizes the lines the viewport reaches, and lets go of the rest.</summary>
    private void RealizeWindow()
    {
        if (_lines.Count == 0)
        {
            Clear();

            return;
        }

        var first = 0;
        var last = _lines.Count - 1;

        if (_viewport.Height > 0)
        {
            var margin = _viewport.Height;

            first = LineAt(_viewport.Y - margin);
            last = LineAt(_viewport.Bottom + margin);
        }
        else
        {
            last = Math.Min(last, OpeningLines - 1);
        }

        if (last < first)
        {
            return;
        }

        var wanted = new HashSet<int>();

        for (var line = first; line <= last; line++)
        {
            var held = _lines[line];

            for (var at = held.Start; at < held.Start + held.Count; at++)
            {
                wanted.Add(at);
            }
        }

        foreach (var index in wanted)
        {
            Ensure(index);
        }

        foreach (var index in _realized.Keys.ToArray())
        {
            if (!wanted.Contains(index))
            {
                Unrealize(index);
            }
        }
    }

    /// <summary>The container for one item, made and measured if it is not on screen yet.</summary>
    /// <param name="index">The item's place in the listing.</param>
    /// <returns>The container, or nothing when the toolkit has no generator yet.</returns>
    private Control? Ensure(int index)
    {
        if (_realized.TryGetValue(index, out var known))
        {
            return known;
        }

        if (ItemContainerGenerator is null || index < 0 || index >= Items.Count)
        {
            return null;
        }

        var item = Items[index];
        Control container;

        if (ItemContainerGenerator.NeedsContainer(item, index, out var recycleKey))
        {
            container = ItemContainerGenerator.CreateContainer(item, index, recycleKey);
            ItemContainerGenerator.PrepareItemContainer(container, item, index);
        }
        else
        {
            if (item is not Control own)
            {
                return null;
            }

            container = own;
            ItemContainerGenerator.PrepareItemContainer(container, item, index);
            _owned.Add(index);
        }

        AddInternalChild(container);
        ItemContainerGenerator.ItemContainerPrepared(container, item, index);
        _realized[index] = container;
        container.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));

        return container;
    }

    /// <summary>Takes a container off the panel and lets the toolkit let go of the item.</summary>
    /// <param name="index">The item's place in the listing.</param>
    private void Unrealize(int index)
    {
        if (!_realized.Remove(index, out var container))
        {
            return;
        }

        RemoveInternalChild(container);

        if (_owned.Remove(index))
        {
            return;
        }

        ItemContainerGenerator?.ClearItemContainer(container);
    }

    /// <summary>Lets go of every container and of the lines measured for them.</summary>
    private void Clear()
    {
        foreach (var index in _realized.Keys.ToArray())
        {
            Unrealize(index);
        }

        _lines = [];
        _tops = [];
        _calibrated = false;
        Columns = 1;
    }

    /// <summary>The line an item stands on.</summary>
    /// <param name="index">The item's place in the listing.</param>
    /// <returns>The line's index, or -1 when the lines are not measured.</returns>
    private int LineOf(int index)
    {
        for (var at = 0; at < _lines.Count; at++)
        {
            var held = _lines[at];

            if (index >= held.Start && index < held.Start + held.Count)
            {
                return at;
            }
        }

        return -1;
    }

    /// <summary>The line a vertical position reaches, clamped to the lines there are.</summary>
    /// <param name="y">The position, in the panel's coordinates.</param>
    /// <returns>The line's index.</returns>
    private int LineAt(double y)
    {
        for (var at = 0; at < _lines.Count; at++)
        {
            if (y < _tops[at] + _lines[at].Height)
            {
                return at;
            }
        }

        return Math.Max(0, _lines.Count - 1);
    }
}
