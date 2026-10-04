using FileSystemPlugin;
using RorolalaDesktop.Contract;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What a context menu is made of: which plugin's items are offered, and when.
/// </summary>
/// <remarks>
/// A menu is the File System plugin's and the items in it are other plugins', so what is checked is the asking:
/// which of them belong to the context, which are the forced ones a Shift unfolds, and in what order they are
/// added. What is not checked is the menu itself, which is built out of pictures and a picture needs a screen.
/// </remarks>
public sealed class ContextMenuTests
{
    /// <summary>The entry every one of these menus is opened on.</summary>
    private static readonly ContextTarget Was = new(
        "/work",
        [new Entry("/work/hero.psd", EntryKind.File)]
    );

    /// <summary>
    /// The items of a context are offered by the order their plugins were started in, and then by their own.
    /// </summary>
    [Fact]
    public void ItemsComeInPluginOrderAndThenTheirOwn()
    {
        var services = Host.Services();
        var early = services.For(new PluginId("it.early"), 0);
        var late = services.For(new PluginId("it.late"), 1);

        late.ContextMenus.Add(ContextMenuTarget.File, new ContextMenuItem("late", 0, _ => { }));
        early.ContextMenus.Add(ContextMenuTarget.File, new ContextMenuItem("second", 20, _ => { }));
        early.ContextMenus.Add(ContextMenuTarget.File, new ContextMenuItem("first", 10, _ => { }));

        var registry = early.ContextMenus;

        Assert.Equal(
            ["first", "second", "late"],
            Names(BrowserActions.Offer(registry, ContextMenuTarget.File, Was, force: false))
        );
    }

    /// <summary>An item that says what it is offered for is offered for that and nothing else.</summary>
    [Fact]
    public void AnItemThatSaysWhatItIsForIsOfferedForThatAlone()
    {
        var registry = Host.Services().For(new PluginId("it.mine"), 0).ContextMenus;

        registry.Add(
            ContextMenuTarget.File,
            new ContextMenuItem("only-here", 0, _ => { }, Applies: was => was.Entry?.Path == "/work/hero.psd")
        );

        Assert.Equal(
            ["only-here"],
            Names(BrowserActions.Offer(registry, ContextMenuTarget.File, Was, force: false))
        );

        var elsewhere = new ContextTarget("/work", [new Entry("/work/other.psd", EntryKind.File)]);

        Assert.Empty(BrowserActions.Offer(registry, ContextMenuTarget.File, elsewhere, force: false));
    }

    /// <summary>
    /// A forced item is offered only when it is the forced ones that are being asked for.
    /// </summary>
    /// <remarks>
    /// The two are asked for apart, because the menu holds both and unfolds one of them: what a forced action
    /// does is past the checks the plain one makes, and the pointer should not find it by accident.
    /// </remarks>
    [Fact]
    public void AForcedItemIsOfferedOnlyWhenTheForcedOnesAreAskedFor()
    {
        var registry = Host.Services().For(new PluginId("it.mine"), 0).ContextMenus;

        registry.Add(ContextMenuTarget.File, new ContextMenuItem("plain", 10, _ => { }));
        registry.Add(ContextMenuTarget.File, new ContextMenuItem("forced", 20, _ => { }, Force: true));

        Assert.Equal(["plain"], Names(BrowserActions.Offer(registry, ContextMenuTarget.File, Was, force: false)));
        Assert.Equal(["forced"], Names(BrowserActions.Offer(registry, ContextMenuTarget.File, Was, force: true)));
    }

    /// <summary>What one context offers is nothing of another's.</summary>
    [Fact]
    public void AContextOffersNothingOfAnother()
    {
        var registry = Host.Services().For(new PluginId("it.mine"), 0).ContextMenus;

        registry.Add(ContextMenuTarget.Directory, new ContextMenuItem("for-folders", 0, _ => { }));

        Assert.Empty(BrowserActions.Offer(registry, ContextMenuTarget.File, Was, force: false));
    }

    /// <summary>
    /// A menu opened on several entries is about all of them, and wears the words of the one that was clicked.
    /// </summary>
    /// <remarks>
    /// The whole choice, because an action may be about all of it — claiming three files is one claim, and a
    /// menu item handed only the first would have to be pointed at three times. And the kind of the one that was
    /// clicked, because a menu is about what was pointed at first: a folder among files is still a menu opened
    /// on a folder, which is the kind of thing its words are about.
    /// </remarks>
    [Fact]
    public void AChoiceIsAboutAllOfItAndTheKindThatWasClicked()
    {
        Entry[] chosen =
        [
            new("/work/folder", EntryKind.Directory),
            new("/work/hero.psd", EntryKind.File),
        ];

        var about = BrowserActions.About(chosen, "/work");

        Assert.NotNull(about);

        var (target, was) = about.Value;

        Assert.Equal(ContextMenuTarget.Directory, target);
        Assert.Equal("/work", was.Directory);
        Assert.Equal(chosen, was.Entries);
        Assert.Equal(chosen[0], was.Entry);
    }

    /// <summary>A choice of files is a menu opened on a file, and a folder clicked among them decides.</summary>
    [Fact]
    public void TheClickedOneDecidesWhichMenuItIs()
    {
        Entry[] files = [new("/work/hero.psd", EntryKind.File), new("/work/second.psd", EntryKind.File)];

        Assert.Equal(ContextMenuTarget.File, BrowserActions.About(files, "/work")?.Target);

        Entry[] clickedFolder =
        [
            new("/work/folder", EntryKind.Directory),
            new("/work/hero.psd", EntryKind.File),
        ];

        Assert.Equal(ContextMenuTarget.Directory, BrowserActions.About(clickedFolder, "/work")?.Target);
    }

    /// <summary>A menu opened on nothing is about nothing, and offers nothing of a plugin's.</summary>
    [Fact]
    public void AChoiceOfNothingIsAboutNothing()
    {
        Assert.Null(BrowserActions.About([], "/work"));
    }

    /// <summary>An item that asks about the choice is answered with all of it, and not only the clicked one.</summary>
    [Fact]
    public void AnItemAboutTheWholeChoiceIsOfferedTheWholeOfIt()
    {
        var registry = Host.Services().For(new PluginId("it.mine"), 0).ContextMenus;

        registry.Add(
            ContextMenuTarget.File,
            new ContextMenuItem(
                "several",
                10,
                _ => { },
                Applies: was => was.Entries.Count > 1 && was.Entry?.Kind == EntryKind.File
            )
        );

        Entry[] chosen = [new("/work/hero.psd", EntryKind.File), new("/work/second.psd", EntryKind.File)];
        var was = new ContextTarget("/work", chosen);

        Assert.Equal(["several"], Names(BrowserActions.Offer(registry, ContextMenuTarget.File, was, force: false)));

        var alone = new ContextTarget("/work", [chosen[0]]);

        Assert.Empty(BrowserActions.Offer(registry, ContextMenuTarget.File, alone, force: false));
    }

    /// <summary>What the items say, in the order they are offered.</summary>
    /// <param name="items">The items offered.</param>
    /// <returns>Their label keys.</returns>
    private static string[] Names(IReadOnlyList<ContextMenuItem> items) =>
        [.. items.Select(item => item.LabelKey)];
}
