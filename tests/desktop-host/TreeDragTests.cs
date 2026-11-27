using Avalonia.Input;
using FileSystemPlugin;
using RorolalaDesktop.Contract;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What a drag of a chosen step carries, and when a press keeps the choice it landed in.
/// </summary>
public sealed class TreeDragTests
{
    /// <summary>A drag that begins on a chosen step carries the whole choice, and one alone carries itself.</summary>
    [Fact]
    public void ADragOfAChosenStepCarriesTheWholeChoice()
    {
        Entry one = new("/work/a.psd", EntryKind.File);
        Entry two = new("/work/b.psd", EntryKind.File);

        Assert.Equal([one, two], TreeBrowser.Carrying([one, two], two));
        Assert.Equal([two], TreeBrowser.Carrying([one], two));
    }

    /// <summary>
    /// A press on a step already in a multiple choice keeps the choice, which is what a drag of it begins from.
    /// </summary>
    /// <remarks>
    /// A modified press is the toolkit's selection rather than a drag, and a press on a step outside the choice
    /// begins a frame rather than a drag, so neither keeps the choice.
    /// </remarks>
    [Fact]
    public void APressOnAChosenStepKeepsTheChoiceForADrag()
    {
        Assert.True(TreeBrowser.KeepsChoice(["a", "b"], "b", KeyModifiers.None));
        Assert.False(TreeBrowser.KeepsChoice(["a"], "a", KeyModifiers.None));
        Assert.False(TreeBrowser.KeepsChoice(["a", "b"], "c", KeyModifiers.None));
        Assert.False(TreeBrowser.KeepsChoice(["a", "b"], "b", KeyModifiers.Control));
        Assert.False(TreeBrowser.KeepsChoice(["a", "b"], "b", KeyModifiers.Shift));
    }

    /// <summary>
    /// A frame chooses what it covers, and what is held says whether it replaces, toggles, takes away or adds.
    /// </summary>
    /// <remarks>
    /// The rule is the listing's own (see <see cref="Choice.Framed"/>): nothing held chooses what the frame
    /// covers, <c>Shift</c> toggles each step it covers, <c>Ctrl</c> takes each away, and the two together put
    /// each back.
    /// </remarks>
    [Fact]
    public void AFrameChoosesWhatItCovers()
    {
        Assert.Equal(["b", "c"], TreeBrowser.Framed(["a"], ["b", "c"], KeyModifiers.None));
        Assert.Empty(TreeBrowser.Framed(["a"], [], KeyModifiers.None));
        Assert.Equal(["a"], TreeBrowser.Framed(["a", "b"], ["b"], KeyModifiers.Control));
        Assert.Equal(["a", "b"], TreeBrowser.Framed(["a"], ["b"], KeyModifiers.Shift));
        Assert.Equal(["b"], TreeBrowser.Framed(["a", "b"], ["a"], KeyModifiers.Shift));
        Assert.Equal(
            ["a", "b", "c"],
            TreeBrowser.Framed(["a"], ["b", "c"], KeyModifiers.Control | KeyModifiers.Shift)
        );
    }
}
