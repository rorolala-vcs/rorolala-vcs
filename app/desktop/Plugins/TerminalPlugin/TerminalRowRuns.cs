using System.Globalization;
using System.Text;

using Avalonia.Media;
using Avalonia.Media.TextFormatting;

namespace TerminalPlugin;

/// <summary>One run of a row: the cells that are drawn as one piece of shaped text.</summary>
/// <param name="Column">The column it starts at.</param>
/// <param name="Columns">How many columns it spans.</param>
/// <param name="Text">The shaped text, drawn at the row's own top.</param>
/// <param name="Ink">The brush its text and its underline are drawn in.</param>
/// <param name="Underline">Whether its cells are lined underneath.</param>
internal readonly record struct TerminalRun(
    int Column,
    int Columns,
    FormattedText Text,
    IBrush Ink,
    bool Underline
);

/// <summary>The shaped runs of each visible row, kept so that a frame that changes one line shapes that line only.</summary>
/// <remarks>
/// A terminal redraws its whole screen on every frame, and a shell's scrollback is the same cells from
/// one frame to the next; shaping is the expensive half of drawing them. A row keeps the cells it was
/// last drawn from and the runs that shaped them, and hands those runs back while its cells compare
/// equal. The comparison is cell by cell rather than by a hash of the row: a collision would draw the
/// line the row used to hold, and a wrong character in a shell is worse than the shaping it saves.
/// <para>
/// The rows are keyed by their place on the screen, not by the line they show, so scrolling rebuilds the
/// rows it moves; what it saves is the frames between two changes, which is most of them.
/// </para>
/// </remarks>
internal sealed class TerminalRowRuns
{
    /// <summary>The size the text is shaped at.</summary>
    private readonly double _size;

    /// <summary>The face a cell's marks ask for.</summary>
    private readonly Func<CellMarks, Typeface> _face;

    /// <summary>The ink and the ground a cell is drawn in.</summary>
    private readonly Func<TerminalCell, (IBrush Fore, IBrush Back)> _colours;

    /// <summary>The terminal's own ground, which a blank cell needs no run for.</summary>
    private readonly IBrush _ground;

    /// <summary>What was shaped, by the row it was drawn in.</summary>
    private readonly List<RowShape> _rows = [];

    /// <summary>Makes the runs of one view's rows.</summary>
    /// <param name="size">The size the text is shaped at.</param>
    /// <param name="face">The face a cell's marks ask for.</param>
    /// <param name="colours">The ink and the ground a cell is drawn in.</param>
    /// <param name="ground">The terminal's own ground.</param>
    internal TerminalRowRuns(
        double size,
        Func<CellMarks, Typeface> face,
        Func<TerminalCell, (IBrush Fore, IBrush Back)> colours,
        IBrush ground
    )
    {
        ArgumentNullException.ThrowIfNull(face);
        ArgumentNullException.ThrowIfNull(colours);
        ArgumentNullException.ThrowIfNull(ground);

        _size = size;
        _face = face;
        _colours = colours;
        _ground = ground;
    }

    /// <summary>Lets go of the rows a frame no longer draws.</summary>
    /// <param name="rows">How many rows are on screen.</param>
    internal void Trim(int rows)
    {
        if (_rows.Count > rows)
        {
            _rows.RemoveRange(rows, _rows.Count - rows);
        }
    }

    /// <summary>The runs of one row, shaped again only when its cells changed.</summary>
    /// <param name="row">The row's place on the screen.</param>
    /// <param name="cells">What the row holds now.</param>
    /// <returns>The runs to draw, which are the previous ones when the row is unchanged.</returns>
    internal IReadOnlyList<TerminalRun> Row(int row, ReadOnlySpan<TerminalCell> cells)
    {
        var kept = At(row);

        if (Same(kept.Cells.AsSpan(0, kept.Count), cells))
        {
            return kept.Runs;
        }

        if (kept.Cells.Length < cells.Length)
        {
            kept.Cells = new TerminalCell[cells.Length];
        }

        cells.CopyTo(kept.Cells);
        kept.Count = cells.Length;
        kept.Runs.Clear();
        Shape(kept.Runs, cells);

        return kept.Runs;
    }

    /// <summary>What a cell's character is, as text.</summary>
    /// <param name="cell">The cell.</param>
    /// <returns>The character, or a space for a blank one.</returns>
    internal static string Character(in TerminalCell cell) =>
        cell.Blank ? " " : char.ConvertFromUtf32(cell.Codepoint);

    /// <summary>The row's entry, made when the frame first reaches it.</summary>
    /// <param name="row">The row's place on the screen.</param>
    /// <returns>The entry.</returns>
    private RowShape At(int row)
    {
        while (_rows.Count <= row)
        {
            _rows.Add(new RowShape());
        }

        return _rows[row];
    }

    /// <summary>Shapes one row into the runs it is drawn as.</summary>
    /// <param name="runs">Where the runs are collected.</param>
    /// <param name="line">The row's cells.</param>
    private void Shape(List<TerminalRun> runs, ReadOnlySpan<TerminalCell> line)
    {
        for (var at = 0; at < line.Length;)
        {
            var cell = line[at];

            // The right half of a wide character is drawn by the character it belongs to.
            if (cell.Continuation)
            {
                at++;

                continue;
            }

            var start = at;
            var face = _face(cell.Marks);
            var (fore, back) = _colours(cell);
            var run = new StringBuilder();

            run.Append(Character(cell));
            at++;

            while (at < line.Length)
            {
                var next = line[at];

                if (next.Continuation || !SameRun(next, cell))
                {
                    break;
                }

                run.Append(Character(next));
                at++;
            }

            // A lone blank cell on the terminal's own ground is what the ground already is: no run
            // would be drawn for it, so none is shaped.
            if (at == start + 1 && cell.Blank && ReferenceEquals(back, _ground))
            {
                continue;
            }

            runs.Add(
                new TerminalRun(
                    start,
                    at - start,
                    new FormattedText(
                        run.ToString(),
                        CultureInfo.InvariantCulture,
                        FlowDirection.LeftToRight,
                        face,
                        _size,
                        fore
                    ),
                    fore,
                    cell.Marks.HasFlag(CellMarks.Underline)
                )
            );
        }
    }

    /// <summary>Whether two spans of cells hold the same cells.</summary>
    /// <param name="kept">The cells the row was shaped from.</param>
    /// <param name="cells">What the row holds now.</param>
    /// <returns>Whether they are equal.</returns>
    private static bool Same(ReadOnlySpan<TerminalCell> kept, ReadOnlySpan<TerminalCell> cells)
    {
        if (kept.Length != cells.Length)
        {
            return false;
        }

        for (var at = 0; at < cells.Length; at++)
        {
            if (!SameCell(kept[at], cells[at]))
            {
                return false;
            }
        }

        return true;
    }

    /// <summary>Whether two cells are the same character in the same ink, ground and marks.</summary>
    /// <param name="cell">One cell.</param>
    /// <param name="of">The other.</param>
    /// <returns>Whether they are equal.</returns>
    private static bool SameCell(in TerminalCell cell, in TerminalCell of) =>
        cell.Codepoint == of.Codepoint
        && cell.Foreground == of.Foreground
        && cell.Background == of.Background
        && cell.HasForeground == of.HasForeground
        && cell.HasBackground == of.HasBackground
        && cell.Marks == of.Marks;

    /// <summary>Whether two cells are drawn the same, which is what makes one run of them.</summary>
    /// <param name="cell">One cell.</param>
    /// <param name="of">The other.</param>
    /// <returns>Whether they are one run.</returns>
    private bool SameRun(in TerminalCell cell, in TerminalCell of)
    {
        var (fore, back) = _colours(cell);
        var (otherFore, otherBack) = _colours(of);

        return cell.Marks == of.Marks
            && ReferenceEquals(fore, otherFore)
            && ReferenceEquals(back, otherBack);
    }

    /// <summary>What one row was drawn from, and what that shaped.</summary>
    private sealed class RowShape
    {
        /// <summary>The cells it was shaped from, which may be longer than it holds.</summary>
        internal TerminalCell[] Cells = [];

        /// <summary>How many of <see cref="Cells"/> are the row.</summary>
        internal int Count;

        /// <summary>The runs the cells shaped.</summary>
        internal List<TerminalRun> Runs { get; } = [];
    }
}
