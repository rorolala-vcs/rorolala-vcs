using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// What a cell of a table says, whatever it is a table of: the words, the mark, and the ink either is drawn in.
/// </summary>
/// <remarks>
/// A cell is drawn from an answer and not from an entry, which is what lets a listing of a directory and a listing
/// of a plugin's place be drawn by one table: what a contributed column answers about an entry is
/// <see cref="EntryColumnMark"/>, and what a lock provider answers is the same shape of answer. The table itself
/// has no opinion about either.
/// </remarks>
internal static class ColumnCells
{
    /// <summary>What a cell says when its provider has nothing to say.</summary>
    public const string Placeholder = "-";

    /// <summary>How large a dot is: a mark read at a glance rather than a thing to point at.</summary>
    private const double DotSize = 8;

    /// <summary>The column one column provider contributes.</summary>
    /// <remarks>
    /// The value in it is the provider's to write and is not read from the entry's metadata, which is why the
    /// column draws its own cell: what it says goes through <see cref="EntryColumnMark.Said"/>, which is a key
    /// translated or a literal as the contribution asked, and neither is in an entry's own facts.
    /// </remarks>
    /// <param name="provider">The provider that speaks for the directory being listed.</param>
    /// <returns>The column to add where the contribution asked.</returns>
    public static Column<Entry> From(IEntryColumnProvider provider) =>
        new(
            provider.ColumnKey,
            provider.Width,
            _ => Placeholder,
            Draw: entry => Contributed(provider, entry),
            Placement: provider.Placement
        );

    /// <summary>What one entry's cell in a contributed column is drawn as.</summary>
    /// <param name="provider">The provider that speaks for the directory being listed.</param>
    /// <param name="entry">The entry to write about.</param>
    /// <returns>The cell, which draws nothing at all where a column of marks has no mark for the entry.</returns>
    public static Control Contributed(IEntryColumnProvider provider, Entry entry)
    {
        ArgumentNullException.ThrowIfNull(provider);
        ArgumentNullException.ThrowIfNull(entry);

        var mark = provider.Value(entry);

        if (mark.Dot)
        {
            return Dot(mark);
        }

        // A column of marks says nothing by having no mark: the dash the listing says what it does not know with
        // would be a mark of its own, and a cell with no mark has nothing else to say.
        return provider.Marks ? new Panel() : Written(mark);
    }

    /// <summary>One cell's value: one line, cut off rather than wrapped, in the faint metadata ink.</summary>
    /// <param name="text">What the cell says.</param>
    /// <param name="align">Where the text sits in it.</param>
    /// <returns>The cell.</returns>
    public static TextBlock Text(string text, TextAlignment align = TextAlignment.Left) =>
        new()
        {
            Text = text,
            TextAlignment = align,
            TextWrapping = TextWrapping.NoWrap,
            TextTrimming = TextTrimming.CharacterEllipsis,
            VerticalAlignment = VerticalAlignment.Center,
            Classes = { "caption", "faint" },
        };

    /// <summary>What one entry's cell in a contributed column that is words is written as.</summary>
    /// <remarks>
    /// The ink is the contribution's to name and this plugin's to resolve: a column drawn in a colour the
    /// contribution picked for itself would be a colour the look does not hold, which is a column that vanishes
    /// somewhere the theme is another — unless the contribution has a palette of its own, which is what a colour
    /// of its own says, and then it is drawn in that.
    /// </remarks>
    /// <param name="mark">What the contribution answered.</param>
    /// <returns>The cell.</returns>
    public static TextBlock Written(EntryColumnMark mark)
    {
        ArgumentNullException.ThrowIfNull(mark);

        var cell = Text(mark.Said() ?? Placeholder);

        if (mark.Colour is { } colour)
        {
            cell.Foreground = new SolidColorBrush(colour);
        }
        else
        {
            cell[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(Icons.Ink(mark.Ink));
        }

        return cell;
    }

    /// <summary>What one entry's lock is written as, in the words and ink the provider asked for.</summary>
    /// <param name="provider">The provider that speaks for the directory being listed.</param>
    /// <param name="entry">The entry to write about.</param>
    /// <returns>The cell.</returns>
    public static TextBlock Locked(IEntryLockProvider provider, Entry entry)
    {
        ArgumentNullException.ThrowIfNull(provider);
        ArgumentNullException.ThrowIfNull(entry);

        var mark = provider.Mark(entry);
        var cell = Text(mark.Said() ?? Placeholder, TextAlignment.Right);
        cell[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(Icons.Ink(mark.TextInk));

        return cell;
    }

    /// <summary>
    /// What one entry's cell in a contributed column that is a mark is drawn as: a round dot in the ink.
    /// </summary>
    /// <remarks>
    /// A dot rather than a word, because what it stands for is one of a handful of states and the colour is read
    /// at a glance: a column of the same few words repeated is a column of text where a mark would do, and the
    /// words are still there — said when the pointer rests on the mark, which is where a reader who wants them is
    /// already looking.
    /// </remarks>
    /// <param name="mark">What the contribution answered.</param>
    /// <returns>The cell.</returns>
    private static Control Dot(EntryColumnMark mark)
    {
        var dot = new Border
        {
            Width = DotSize,
            Height = DotSize,
            CornerRadius = new CornerRadius(DotSize / 2),
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Center,
        };

        // The colour is the contribution's where it named one, and the role it named otherwise: a mark drawn in
        // the role and then painted over would be drawn twice, and the role's own brush would win.
        if (mark.Colour is { } colour)
        {
            dot.Background = new SolidColorBrush(colour);
        }
        else
        {
            dot[!Border.BackgroundProperty] = new DynamicResourceExtension(Icons.Ink(mark.Ink));
        }

        if (mark.Edge is { } edge)
        {
            // The line is the contribution's own rather than the design's, since whatever named the colour named
            // what reads around it: a hairline of the listing's would be a mark drawn differently from the one the
            // contribution asked for.
            dot.BorderThickness = new Thickness(1);
            dot.BorderBrush = new SolidColorBrush(edge);
        }

        if (mark.Hint is { Length: > 0 } hint)
        {
            ToolTip.SetTip(dot, hint);

            // At once rather than after the wait a tooltip usually takes: the mark is a colour, and a reader who
            // has to stop and wait for what a colour means is a reader who has to stop. Nothing is said by the wait
            // that is not said by the mark.
            ToolTip.SetShowDelay(dot, 0);
        }

        return dot;
    }
}
