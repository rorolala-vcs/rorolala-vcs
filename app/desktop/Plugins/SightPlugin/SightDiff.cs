namespace SightPlugin;

/// <summary>The previewer that compares two chosen text files line by line.</summary>
/// <remarks>
/// There is no version history here: this plugin depends on the File System plugin and nothing else,
/// so the two files are the two the user chose. The answer is a unified diff with three lines of
/// context, and it names no files: the listing already names them.
/// </remarks>
internal sealed class SightDiff : ISightPreviewer
{
    /// <summary>How many lines of a file are compared before the diff is made whole-file.</summary>
    private const int LineCap = 2000;

    /// <summary>How many unchanged lines stand around a change.</summary>
    private const int Context = 3;

    /// <inheritdoc />
    public string Id => "sight.diff";

    /// <inheritdoc />
    public string NameKey => "rorolala_sight.previewer.diff";

    /// <inheritdoc />
    public string? IconKey => Icons.ArrowsLeftRight;

    /// <inheritdoc />
    public int Rank => 70;

    /// <inheritdoc />
    public bool Cares(SightFile primary, SightFile? second) =>
        second is not null
        && SightText.TextExtensions.Contains(primary.Extension)
        && SightText.TextExtensions.Contains(second.Extension);

    /// <inheritdoc />
    public async ValueTask<SightDocument?> OpenAsync(
        SightOpenRequest request,
        CancellationToken cancel
    )
    {
        ArgumentNullException.ThrowIfNull(request);

        if (request.Second is not { } second)
        {
            return null;
        }

        var limit = SightSettings.TextLimitBytes;

        if (request.Primary.Length > limit || second.Length > limit)
        {
            return new SightTooLargeDocument(limit);
        }

        var left = await SightText.ReadLinesAsync(request.Primary, limit, cancel)
            .ConfigureAwait(false);
        var right = await SightText.ReadLinesAsync(second, limit, cancel).ConfigureAwait(false);

        if (left is null || right is null)
        {
            return null;
        }

        cancel.ThrowIfCancellationRequested();

        var operations = Compare(left, right);
        var added = operations.Count(operation => operation.Kind == SightDiffKind.Added);
        var removed = operations.Count(operation => operation.Kind == SightDiffKind.Removed);

        return new SightDiffDocument(Hunks(operations), added, removed);
    }

    /// <summary>One line of the comparison before it is grouped into hunks.</summary>
    /// <param name="Kind">What the line does.</param>
    /// <param name="Text">The line.</param>
    /// <param name="LeftAt">Its line number in the first file, counted before it.</param>
    /// <param name="RightAt">Its line number in the second file, counted before it.</param>
    private sealed record Operation(SightDiffKind Kind, string Text, int LeftAt, int RightAt);

    /// <summary>The edit script of two files, line by line.</summary>
    /// <param name="left">The first file.</param>
    /// <param name="right">The second file.</param>
    /// <returns>The operations, in the order a reader meets them.</returns>
    private static List<Operation> Compare(IReadOnlyList<string> left, IReadOnlyList<string> right)
    {
        if (left.Count > LineCap || right.Count > LineCap)
        {
            return WholeFiles(left, right);
        }

        var common = new int[left.Count + 1][];

        for (var at = 0; at <= left.Count; at++)
        {
            common[at] = new int[right.Count + 1];
        }

        for (var at = left.Count - 1; at >= 0; at--)
        {
            for (var other = right.Count - 1; other >= 0; other--)
            {
                common[at][other] =
                    string.Equals(left[at], right[other], StringComparison.Ordinal)
                        ? common[at + 1][other + 1] + 1
                        : Math.Max(common[at + 1][other], common[at][other + 1]);
            }
        }

        var operations = new List<Operation>();
        var a = 0;
        var b = 0;
        var leftAt = 1;
        var rightAt = 1;

        while (a < left.Count && b < right.Count)
        {
            if (string.Equals(left[a], right[b], StringComparison.Ordinal))
            {
                operations.Add(new Operation(SightDiffKind.Context, left[a], leftAt++, rightAt++));
                a++;
                b++;
            }
            else if (common[a + 1][b] >= common[a][b + 1])
            {
                operations.Add(new Operation(SightDiffKind.Removed, left[a], leftAt++, rightAt));
                a++;
            }
            else
            {
                operations.Add(new Operation(SightDiffKind.Added, right[b], leftAt, rightAt++));
                b++;
            }
        }

        while (a < left.Count)
        {
            operations.Add(new Operation(SightDiffKind.Removed, left[a++], leftAt++, rightAt));
        }

        while (b < right.Count)
        {
            operations.Add(new Operation(SightDiffKind.Added, right[b++], leftAt, rightAt++));
        }

        return operations;
    }

    /// <summary>The comparison of two files too long to look at line by line.</summary>
    /// <param name="left">The first file.</param>
    /// <param name="right">The second file.</param>
    /// <returns>Every line of the first removed, then every line of the second added.</returns>
    private static List<Operation> WholeFiles(
        IReadOnlyList<string> left,
        IReadOnlyList<string> right
    )
    {
        var operations = new List<Operation>(left.Count + right.Count);

        for (var at = 0; at < left.Count; at++)
        {
            operations.Add(new Operation(SightDiffKind.Removed, left[at], at + 1, 1));
        }

        for (var at = 0; at < right.Count; at++)
        {
            operations.Add(new Operation(SightDiffKind.Added, right[at], left.Count + 1, at + 1));
        }

        return operations;
    }

    /// <summary>Groups the operations into hunks with three lines of context.</summary>
    /// <param name="operations">The edit script.</param>
    /// <returns>The lines a reader sees.</returns>
    private static List<SightDiffLine> Hunks(IReadOnlyList<Operation> operations)
    {
        var lines = new List<SightDiffLine>();
        var changed = new List<int>();

        for (var at = 0; at < operations.Count; at++)
        {
            if (operations[at].Kind != SightDiffKind.Context)
            {
                changed.Add(at);
            }
        }

        if (changed.Count == 0)
        {
            return lines;
        }

        var groups = new List<(int Start, int End)>();
        var start = Math.Max(0, changed[0] - Context);
        var end = Math.Min(operations.Count, changed[0] + Context + 1);

        foreach (var at in changed.Skip(1))
        {
            var from = Math.Max(0, at - Context);
            var to = Math.Min(operations.Count, at + Context + 1);

            if (from <= end)
            {
                end = Math.Max(end, to);

                continue;
            }

            groups.Add((start, end));
            start = from;
            end = to;
        }

        groups.Add((start, end));

        foreach (var (from, to) in groups)
        {
            var leftCount = 0;
            var rightCount = 0;

            for (var at = from; at < to; at++)
            {
                if (operations[at].Kind != SightDiffKind.Added)
                {
                    leftCount++;
                }

                if (operations[at].Kind != SightDiffKind.Removed)
                {
                    rightCount++;
                }
            }

            lines.Add(
                new SightDiffLine(
                    SightDiffKind.Hunk,
                    $"@@ -{operations[from].LeftAt},{leftCount} +{operations[from].RightAt},{rightCount} @@"
                )
            );

            for (var at = from; at < to; at++)
            {
                lines.Add(
                    new SightDiffLine(operations[at].Kind, operations[at].Text)
                );
            }
        }

        return lines;
    }
}
