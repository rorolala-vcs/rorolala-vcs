using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The reading of a rules file as the Workspace dock draws it, and the changes the list makes.
/// </summary>
/// <remarks>
/// The file is text and the list is a reading of it: a pattern is a row, a run of <c>#</c> lines is
/// one row, and a change is made to the lines rather than to the rows. What is checked here is that
/// the reading keeps every line the file held, so that what is written back is the file rather than
/// what the list happened to draw.
/// </remarks>
public sealed class RuleLinesTests
{
    /// <summary>A file's patterns are rows of their own, and one run of notes is one row.</summary>
    [Fact]
    public void AFileIsItsPatternsAndOneRowPerRunOfNotes()
    {
        string[] lines = ["# first", "# second", "*.tmp", "", "# alone", "!keep/"];

        var blocks = RuleLines.Blocks(lines);

        Assert.Equal(4, blocks.Count);

        var note = Assert.IsType<RuleComment>(blocks[0]);

        Assert.Equal((0, 1), (note.Start, note.End));
        Assert.Equal(["first", "second"], note.Lines);

        var pattern = Assert.IsType<RulePattern>(blocks[1]);

        Assert.Equal((2, "*.tmp"), (pattern.At, pattern.Text));
        Assert.False(pattern.Allows);
        Assert.Equal("*.tmp", pattern.Pattern);

        var allow = Assert.IsType<RulePattern>(blocks[3]);

        Assert.True(allow.Allows);
        Assert.Equal("keep/", allow.Pattern);

        // The blank line is spacing rather than a rule, so the count and the rows both leave it out.
        Assert.Equal(2, RuleLines.Count(lines));
    }

    /// <summary>A comment line says what its mark leaves, and a note's own blank line is one mark.</summary>
    [Fact]
    public void ANotesLinesAreWhatTheMarksLeave()
    {
        string[] lines = ["#", "# one", "## two", "#  three"];

        var note = Assert.IsType<RuleComment>(Assert.Single(RuleLines.Blocks(lines)));

        Assert.Equal(["", "one", "# two", " three"], note.Lines);
    }

    /// <summary>Turning a rule is that mark being added or taken away, and nothing else moving.</summary>
    [Fact]
    public void ARuleIsTurnedByItsOwnMark()
    {
        string[] lines = ["*.tmp"];

        var allowed = RuleLines.Toggle(lines, 0);

        Assert.Equal(["!*.tmp"], allowed);
        Assert.Equal(["*.tmp"], RuleLines.Toggle(allowed, 0));
    }

    /// <summary>A note is written back with one mark a line, and a note that says nothing goes.</summary>
    [Fact]
    public void ANoteIsWrittenBackWithItsMarks()
    {
        string[] lines = ["# old", "# note", "*.tmp"];

        var written = RuleLines.Comment(lines, 0, 1, "first\n\nsecond\n\n");

        Assert.Equal(["# first", "#", "# second", "*.tmp"], written);

        // A note left blank is a note that was deleted, not an empty block in the file.
        Assert.Equal(["*.tmp"], RuleLines.Comment(lines, 0, 1, " \n "));
    }

    /// <summary>A block moved down lands where it was shown rather than one of its own lines later.</summary>
    [Fact]
    public void ABlockLandsAgainstTheRowItWasDroppedOn()
    {
        string[] lines = ["a", "b", "c", "d"];

        Assert.Equal(["b", "c", "a", "d"], RuleLines.Move(lines, 0, 0, 2, 2, true));
        Assert.Equal(["a", "d", "b", "c"], RuleLines.Move(lines, 3, 3, 1, 1, false));
        Assert.Equal(["a", "c", "d"], RuleLines.Remove(lines, 1, 1));
    }

    /// <summary>An empty or blank line is not added, and what is added is trimmed.</summary>
    [Fact]
    public void TheStripAddsOneTrimmedLine()
    {
        string[] lines = ["a"];

        Assert.Same(lines, RuleLines.Add(lines, "   "));
        Assert.Equal(["a", "b"], RuleLines.Add(lines, " b "));
    }
}
