using System.Text;

using Avalonia.Media;
using TerminalPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A terminal's rows are shaped once and drawn again from what was shaped.
/// </summary>
/// <remarks>
/// The view redraws its whole screen on every frame, so what it can save is the shaping of the lines that
/// did not change. The check is made here rather than through a window: the screen and the runs are the
/// two halves the drawing meets, and the shaped text each row hands back is the whole of what is reused.
/// </remarks>
public sealed class TerminalTests
{
    /// <summary>The ground every cell of these checks is drawn on.</summary>
    private static readonly SolidColorBrush Ground = new(Colors.Black);

    /// <summary>The ink every cell of these checks is drawn in.</summary>
    private static readonly SolidColorBrush Ink = new(Colors.White);

    /// <summary>A row whose cells did not change hands back the text it shaped before.</summary>
    [Fact]
    public void ARowIsShapedOnceUntilItsCellsChange()
    {
        var screen = new TerminalScreen(20, 3);
        var runs = new TerminalRowRuns(12, Face, Colours, Ground);

        screen.Feed(Encoding.UTF8.GetBytes("one\r\ntwo\r\nthree"));

        var first = Snapshot(screen, runs);
        var again = Snapshot(screen, runs);

        for (var row = 0; row < screen.Rows; row++)
        {
            Assert.NotEmpty(first[row]);
            Assert.Equal(first[row].Count, again[row].Count);

            for (var at = 0; at < first[row].Count; at++)
            {
                Assert.Same(first[row][at].Text, again[row][at].Text);
            }
        }

        // One character on the first row is that row's cells changing, and the other two rows are
        // still what they were.
        screen.Feed(Encoding.UTF8.GetBytes("\u001b[1;1HX"));

        var changed = Snapshot(screen, runs);

        Assert.Equal(first[0].Count, changed[0].Count);
        Assert.NotSame(first[0][0].Text, changed[0][0].Text);
        Assert.Equal('X', screen.Line(0, 0)[0].Codepoint);

        for (var row = 1; row < screen.Rows; row++)
        {
            Assert.Same(first[row][0].Text, changed[row][0].Text);
        }
    }

    /// <summary>Every row of the screen, as the runs the view would draw it from.</summary>
    /// <param name="screen">The screen.</param>
    /// <param name="runs">What the view shapes it with.</param>
    /// <returns>The runs of each row.</returns>
    private static List<TerminalRun>[] Snapshot(TerminalScreen screen, TerminalRowRuns runs)
    {
        runs.Trim(screen.Rows);

        var rows = new List<TerminalRun>[screen.Rows];

        for (var row = 0; row < screen.Rows; row++)
        {
            rows[row] = [.. runs.Row(row, screen.Line(row, 0))];
        }

        return rows;
    }

    /// <summary>The face every cell of these checks is set in.</summary>
    /// <param name="marks">The marks the cell wears.</param>
    /// <returns>The face.</returns>
    private static Typeface Face(CellMarks marks) => new(FontFamily.Default);

    /// <summary>The ink and the ground every cell of these checks is drawn in.</summary>
    /// <param name="cell">The cell.</param>
    /// <returns>The two brushes.</returns>
    private static (IBrush Fore, IBrush Back) Colours(TerminalCell cell) => (Ink, Ground);
}
