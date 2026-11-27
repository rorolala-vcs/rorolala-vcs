namespace RorolalaVCSPlugin;

/// <summary>
/// One thing the rules file shows as a row: a pattern, or a block of comment lines.
/// </summary>
/// <remarks>
/// The block is the unit the list is edited in, because a comment is read as prose: a run of
/// consecutive <c>#</c> lines is one note rather than one note per line, and it is dragged, edited
/// and deleted whole. A pattern is its own block and stops a run of comments, which is where the
/// file's order is read from.
/// </remarks>
/// <param name="Start">The file line the block begins on.</param>
/// <param name="End">The file line the block ends on, which is <paramref name="Start"/> for a pattern.</param>
internal abstract record RuleBlock(int Start, int End);

/// <summary>One pattern line of the rules file, trimmed as the file's loader reads it.</summary>
/// <param name="At">The file line it sits on.</param>
/// <param name="Text">The pattern, with the <c>!</c> of an allow rule still on it.</param>
internal sealed record RulePattern(int At, string Text) : RuleBlock(At, At)
{
    /// <summary>Whether the rule allows a path rather than ignores it.</summary>
    public bool Allows => Text.StartsWith('!');

    /// <summary>The pattern itself, without the <c>!</c> that says it allows.</summary>
    public string Pattern => Allows ? Text[1..] : Text;
}

/// <summary>A run of consecutive comment lines, shown as the prose it holds.</summary>
/// <param name="Start">The file line the run begins on.</param>
/// <param name="End">The file line the run ends on.</param>
/// <param name="Lines">The run's text, without the <c>#</c> marks and with blank comment lines kept.</param>
internal sealed record RuleComment(int Start, int End, IReadOnlyList<string> Lines)
    : RuleBlock(Start, End);

/// <summary>
/// What a rules file's lines are, as the list shows them and as the list changes them.
/// </summary>
/// <remarks>
/// The file is the truth and this is a reading of it: blank lines are the file's own spacing and are
/// not shown, a run of <c>#</c> lines is one row, and every change is made to the whole list of
/// lines rather than to the rows, so what is written back holds every line the file held, whether or
/// not the list draws it.
/// <para>
/// What a line means is the loader's, and none of it is decided here: a pattern is kept trimmed, an
/// allow rule keeps its <c>!</c>, and a comment is a line whose first non-blank character is
/// <c>#</c>. The order the lines are in is the order the loader applies them in, which is why
/// moving a row is moving its lines and nothing else.
/// </para>
/// </remarks>
internal static class RuleLines
{
    /// <summary>
    /// The rows a file's lines make, in the file's order.
    /// </summary>
    /// <param name="lines">The file's lines.</param>
    /// <returns>One row per pattern and one per run of comment lines.</returns>
    public static IReadOnlyList<RuleBlock> Blocks(IReadOnlyList<string> lines)
    {
        ArgumentNullException.ThrowIfNull(lines);

        var blocks = new List<RuleBlock>();

        for (var at = 0; at < lines.Count;)
        {
            var text = lines[at].Trim();

            if (text.Length == 0)
            {
                at++;
            }
            else if (text.StartsWith('#'))
            {
                var start = at;

                while (at < lines.Count && lines[at].Trim().StartsWith('#'))
                {
                    at++;
                }

                var written = new List<string>(at - start);
                for (var line = start; line < at; line++)
                {
                    written.Add(Uncommented(lines[line]));
                }

                blocks.Add(new RuleComment(start, at - 1, written));
            }
            else
            {
                blocks.Add(new RulePattern(at, text));
                at++;
            }
        }

        return blocks;
    }

    /// <summary>How many patterns the lines hold, which is the count the pane's heading shows.</summary>
    /// <param name="lines">The file's lines.</param>
    /// <returns>The patterns, without the comment lines and the blanks.</returns>
    public static int Count(IReadOnlyList<string> lines)
    {
        ArgumentNullException.ThrowIfNull(lines);

        var count = 0;

        foreach (var line in lines)
        {
            var text = line.Trim();
            if (text.Length > 0 && !text.StartsWith('#'))
            {
                count++;
            }
        }

        return count;
    }

    /// <summary>
    /// The lines with one pattern turned into the other kind of rule, or back.
    /// </summary>
    /// <remarks>
    /// An allow rule is a pattern that begins <c>!</c>, so turning one into the other is that mark
    /// being added or taken away. Nothing else about the line moves, which is what keeps the rule
    /// where the reader put it in the file's order.
    /// </remarks>
    /// <param name="lines">The file's lines.</param>
    /// <param name="at">The line of the pattern.</param>
    /// <returns>The lines with the rule turned.</returns>
    public static IReadOnlyList<string> Toggle(IReadOnlyList<string> lines, int at)
    {
        ArgumentNullException.ThrowIfNull(lines);

        var turned = new List<string>(lines);
        var text = turned[at].Trim();

        turned[at] = text.StartsWith('!') ? text[1..] : $"!{text}";

        return turned;
    }

    /// <summary>The lines without one block.</summary>
    /// <param name="lines">The file's lines.</param>
    /// <param name="start">The line the block begins on.</param>
    /// <param name="end">The line the block ends on.</param>
    /// <returns>The lines, with the block's lines taken out.</returns>
    public static IReadOnlyList<string> Remove(IReadOnlyList<string> lines, int start, int end)
    {
        ArgumentNullException.ThrowIfNull(lines);

        var kept = new List<string>(lines);
        kept.RemoveRange(start, end - start + 1);

        return kept;
    }

    /// <summary>
    /// Writes a comment block from what its editor holds, or takes the block away when it says nothing.
    /// </summary>
    /// <remarks>
    /// Each line is written back as the file states a comment, one <c>#</c> and a space before what it
    /// says, and a line left empty is a <c>#</c> of its own rather than a blank line: a blank line
    /// ends a comment block, so writing one would split the note in two.
    /// </remarks>
    /// <param name="lines">The file's lines.</param>
    /// <param name="start">The line the block begins on.</param>
    /// <param name="end">The line the block ends on.</param>
    /// <param name="text">What the editor holds, one line per line.</param>
    /// <returns>The lines with the comment written, or without it when it states nothing.</returns>
    public static IReadOnlyList<string> Comment(
        IReadOnlyList<string> lines,
        int start,
        int end,
        string text
    )
    {
        ArgumentNullException.ThrowIfNull(lines);
        ArgumentNullException.ThrowIfNull(text);

        var written = new List<string>();

        foreach (var line in text.Replace("\r\n", "\n", StringComparison.Ordinal).Split('\n'))
        {
            written.Add(line.Trim().Length == 0 ? "#" : $"# {line}");
        }

        while (written.Count > 0 && written[^1] == "#")
        {
            written.RemoveAt(written.Count - 1);
        }

        var kept = new List<string>(lines);
        kept.RemoveRange(start, end - start + 1);

        if (written.Count == 0)
        {
            return kept;
        }

        kept.InsertRange(start, written);

        return kept;
    }

    /// <summary>
    /// The lines in a new order, with one block moved before or after another.
    /// </summary>
    /// <remarks>
    /// The target is a block rather than a line because that is what the pointer is over: a reader
    /// drops a note on a rule, not between two of the note's own lines. The block is taken out before
    /// the landing line is worked out, so a block moved down lands where it was shown rather than one
    /// of its own lines further on.
    /// </remarks>
    /// <param name="lines">The file's lines.</param>
    /// <param name="start">The line the moved block begins on.</param>
    /// <param name="end">The line the moved block ends on.</param>
    /// <param name="target">The line the target block begins on.</param>
    /// <param name="targetEnd">The line the target block ends on.</param>
    /// <param name="after">Whether the block lands after the target rather than before it.</param>
    /// <returns>The lines in the new order.</returns>
    public static IReadOnlyList<string> Move(
        IReadOnlyList<string> lines,
        int start,
        int end,
        int target,
        int targetEnd,
        bool after
    )
    {
        ArgumentNullException.ThrowIfNull(lines);

        var count = end - start + 1;
        var at = after ? targetEnd + 1 : target;
        var moved = new List<string>(lines);
        var block = moved.GetRange(start, count);

        moved.RemoveRange(start, count);

        var landing = start < at ? at - count : at;
        moved.InsertRange(landing, block);

        return moved;
    }

    /// <summary>Adds a line at the end, which is the strongest place in the file for it.</summary>
    /// <param name="lines">The file's lines.</param>
    /// <param name="text">What the strip holds, a pattern or a note begun with <c>#</c>.</param>
    /// <returns>The lines with the text appended, or the same lines when the text states nothing.</returns>
    public static IReadOnlyList<string> Add(IReadOnlyList<string> lines, string text)
    {
        ArgumentNullException.ThrowIfNull(lines);
        ArgumentNullException.ThrowIfNull(text);

        var pattern = text.Trim();

        if (pattern.Length == 0)
        {
            return lines;
        }

        var added = new List<string>(lines) { pattern };

        return added;
    }

    /// <summary>The prose a comment line holds, without its <c>#</c> and the one space after it.</summary>
    /// <param name="line">The line.</param>
    /// <returns>What the line says to a reader of the note.</returns>
    private static string Uncommented(string line)
    {
        var text = line.Trim();

        return text.Length > 1 && text[1] == ' ' ? text[2..] : text[1..];
    }
}
