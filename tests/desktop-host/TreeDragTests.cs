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
    /// chooses that step alone, so neither keeps the choice.
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
}
