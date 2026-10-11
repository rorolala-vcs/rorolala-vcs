using Avalonia.Input;
using FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What a gesture on a listing leaves chosen: which entries a click leaves chosen, and which a frame does.
/// </summary>
/// <remarks>
/// The gestures themselves are the toolkit's pointer events and are not reachable from a test, so what is
/// checked here is the rules they are read by. The two are deliberately not one rule: a frame reads a modifier
/// over every entry it covers, and a click reads the same modifier over the one entry under the pointer, so a
/// wrong rule is what a user meets as a listing that chooses what they did not point at, that drops a choice
/// they were adding to, or that reads a range where they meant a single entry.
/// </remarks>
public sealed class SelectionTests
{
    /// <summary>With nothing held, a frame chooses what it covers and nothing else.</summary>
    /// <remarks>
    /// This is the rule that makes a frame over nothing a gesture that chooses nothing rather than one that
    /// leaves the choice as it was, which is what clearing the choice reads as.
    /// </remarks>
    [Fact]
    public void AFrameWithNothingHeldChoosesWhatItCovers()
    {
        Assert.Equal([3], Choice.Framed([1, 2], [3], KeyModifiers.None));
        Assert.Empty(Choice.Framed([1, 2], [], KeyModifiers.None));
    }

    /// <summary>Shift over a frame toggles each entry it covers, in either direction.</summary>
    [Fact]
    public void ShiftOverAFrameTogglesEachEntry()
    {
        Assert.Equal([1, 3], Choice.Framed([1, 2], [2, 3], KeyModifiers.Shift));
        Assert.Equal([2], Choice.Framed([2, 3], [3], KeyModifiers.Shift));
    }

    /// <summary>Control over a frame takes each entry it covers away, and never adds one.</summary>
    [Fact]
    public void ControlOverAFrameTakesEachEntryAway()
    {
        Assert.Equal([1], Choice.Framed([1, 2, 3], [2, 3], KeyModifiers.Control));
        Assert.Equal([1], Choice.Framed([1], [2, 3], KeyModifiers.Control));
    }

    /// <summary>Control with Shift over a frame adds each entry it covers, and never takes one away.</summary>
    [Fact]
    public void ControlShiftOverAFrameAddsEachEntry()
    {
        var both = KeyModifiers.Control | KeyModifiers.Shift;

        Assert.Equal([1, 2, 3], Choice.Framed([1, 3], [3, 2], both));
        Assert.Equal([1], Choice.Framed([1], [1], both));
    }

    /// <summary>
    /// A modifier over a frame that covers nothing leaves the choice exactly as it stood.
    /// </summary>
    /// <remarks>
    /// What a click on the space around the entries comes to with a modifier held: there is no entry to toggle,
    /// take away or add, so the hand has asked for nothing rather than for a listing with nothing chosen.
    /// </remarks>
    [Fact]
    public void AModifierOverNothingLeavesTheChoiceStanding()
    {
        Assert.Equal([4], Choice.Framed([4], [], KeyModifiers.Shift));
        Assert.Equal([4], Choice.Framed([4], [], KeyModifiers.Control));
        Assert.Equal([4], Choice.Framed([4], [], KeyModifiers.Control | KeyModifiers.Shift));
    }

    /// <summary>What a frame covers is answered in the order the listing shows, whatever order it covered it in.</summary>
    [Fact]
    public void AFrameIsAnsweredInTheOrderTheListingShows() =>
        Assert.Equal([0, 2, 5], Choice.Framed([2, 5], [0], KeyModifiers.Control | KeyModifiers.Shift));

    /// <summary>With nothing held, a click chooses the entry it landed on and lets go of every other.</summary>
    [Fact]
    public void AClickWithNothingHeldChoosesTheOneEntry()
    {
        Assert.Equal([3], Choice.Clicked([1, 2], 3, 1, KeyModifiers.None));

        // A click on the space around the entries is the choice let go of.
        Assert.Empty(Choice.Clicked([1, 2], -1, 1, KeyModifiers.None));
    }

    /// <summary>
    /// Shift with a click moves the nearer end of a run that is already chosen, and leaves the other end.
    /// </summary>
    /// <remarks>
    /// This is what makes a run that was made too long shortenable again: adding a run can only ever grow the
    /// choice, where moving an end can take one back.
    /// </remarks>
    [Fact]
    public void ShiftWithAClickMovesTheNearerEndOfARun()
    {
        // Beyond either end, the run grows that way and the end further off stays where it is.
        Assert.Equal([5, 6, 7, 8, 9], Choice.Clicked([5, 6, 7], 9, 5, KeyModifiers.Shift));
        Assert.Equal([3, 4, 5, 6, 7], Choice.Clicked([5, 6, 7], 3, 7, KeyModifiers.Shift));

        // Inside the run, the side the click is nearer to is the side the run is taken back on.
        Assert.Equal([6, 7, 8, 9], Choice.Clicked([5, 6, 7, 8, 9], 6, 5, KeyModifiers.Shift));
        Assert.Equal([5, 6, 7, 8], Choice.Clicked([5, 6, 7, 8, 9], 8, 9, KeyModifiers.Shift));

        // A click the same distance from both ends is read as the back end's, so the front of the run stays.
        Assert.Equal([5, 6, 7], Choice.Clicked([5, 6, 7, 8, 9], 7, 5, KeyModifiers.Shift));

        // An end clicked is the end it already is.
        Assert.Equal([5, 6, 7], Choice.Clicked([5, 6, 7], 5, 7, KeyModifiers.Shift));

        // A run of one is the run the click grows from, whichever side the click is.
        Assert.Equal([5, 6, 7], Choice.Clicked([5], 7, 5, KeyModifiers.Shift));
        Assert.Equal([3, 4, 5], Choice.Clicked([5], 3, 5, KeyModifiers.Shift));
    }

    /// <summary>
    /// Shift with a click over a choice that has a gap in it adds the run from where the last click landed.
    /// </summary>
    /// <remarks>
    /// A choice with a gap has no one end at either side for a click to move, so it falls back to the run added
    /// from the anchor, which is what the two ends of a run replaced.
    /// </remarks>
    [Fact]
    public void ShiftWithAClickOverAGapAddsTheRunFromTheAnchor()
    {
        Assert.Equal([1, 2, 3, 4, 5], Choice.Clicked([1, 3], 5, 1, KeyModifiers.Shift));
        Assert.Equal([1, 2, 3], Choice.Clicked([1, 3], 2, 3, KeyModifiers.Shift));
    }

    /// <summary>
    /// Shift with a click where nothing has landed yet and nothing is chosen is the click on the one entry.
    /// </summary>
    /// <remarks>
    /// A dock that was just opened has no entry behind it to take a run from, and a run from nowhere would be
    /// every entry before the one clicked.
    /// </remarks>
    [Fact]
    public void ShiftWithAClickAndNoAnchorChoosesTheOneEntry() =>
        Assert.Equal([3], Choice.Clicked([], 3, -1, KeyModifiers.Shift));

    /// <summary>Control with a click toggles the one entry, which is the one rule a click reads both ways.</summary>
    [Fact]
    public void ControlWithAClickTogglesTheOneEntry()
    {
        Assert.Equal([1, 3], Choice.Clicked([1, 2, 3], 2, 0, KeyModifiers.Control));
        Assert.Equal([1, 2, 3], Choice.Clicked([1, 3], 2, 0, KeyModifiers.Control));
    }

    /// <summary>
    /// A click has no run of entries, so Control with Shift is read as Control.
    /// </summary>
    /// <remarks>
    /// What is checked is that the one entry is toggled and not that a run is added: the two modifiers divide a
    /// run between them, and a click is on one entry.
    /// </remarks>
    [Fact]
    public void ControlShiftWithAClickIsReadAsControl() =>
        Assert.Equal([1, 2, 3], Choice.Clicked([1, 3], 2, 0, KeyModifiers.Control | KeyModifiers.Shift));
}
