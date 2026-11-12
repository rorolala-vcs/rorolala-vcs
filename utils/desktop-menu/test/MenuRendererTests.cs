using Avalonia.Controls;
using RorolalaDesktop.Menus;

namespace RorolalaDesktopMenu.Tests;

/// <summary>
/// What a renderer makes of a list of leaves: the categories it folds them into, the one entry it flattens,
/// and what it does with the questions a menu asks before drawing one.
/// </summary>
public sealed class MenuRendererTests
{
    /// <summary>A category with one entry is that entry, with the category's word in front of it.</summary>
    [Fact]
    public void ACategoryOfOneEntryIsFlattened()
    {
        var menu = Built(
            Words(("menu.entry.cut", "Cut"), ("menu.entry.file", "File")),
            new MenuLeaf(["menu.entry.cut", "menu.entry.file"], 0, () => { })
        );

        Assert.Equal(["Cut File"], Headers(menu));
    }

    /// <summary>
    /// The first key is what makes a category: the same one from another plugin joins it, and another one
    /// stays apart even when the words are alike.
    /// </summary>
    [Fact]
    public void TheFirstKeyMakesTheCategory()
    {
        var menu = Built(
            Words(
                ("menu.entry.copy", "Copy"),
                ("menu.entry.paste", "Paste"),
                ("menu.entry.clone", "Clone"),
                ("menu.entry.file", "File"),
                ("menu.entry.things", "Things")
            ),
            new MenuLeaf(["menu.entry.copy", "menu.entry.file"], 0, () => { }),
            new MenuLeaf(["menu.entry.paste", "menu.entry.file"], 0, () => { }),
            new MenuLeaf(["menu.entry.paste", "menu.entry.things"], 0, () => { }),
            new MenuLeaf(["menu.entry.clone", "menu.entry.file"], 0, () => { })
        );

        Assert.Equal(["Copy File", "Paste", "Clone File"], Headers(menu));

        var paste = Assert.Single(menu.Items.OfType<MenuItem>(), item => Header(item) == "Paste");

        Assert.Equal(["File", "Things"], Headers(paste));
    }

    /// <summary>A category keeps the place where it was first registered.</summary>
    /// <remarks>
    /// The second entry of a category may come from a plugin loaded later, and the category still sits where
    /// the first plugin put it rather than moving to the later plugin's place.
    /// </remarks>
    [Fact]
    public void ARootKeepsWhereItWasFirstRegistered()
    {
        var menu = Built(
            Words(("menu.entry.first", "First"), ("menu.entry.second", "Second"), ("menu.entry.item", "Item")),
            new MenuLeaf(["menu.entry.second", "menu.entry.item"], 0, () => { }),
            new MenuLeaf(["menu.entry.first", "menu.entry.item"], 0, () => { }),
            new MenuLeaf(["menu.entry.second", "menu.entry.other"], 0, () => { })
        );

        Assert.Equal(["Second", "First Item"], Headers(menu));
    }

    /// <summary>Entries of one category are ordered by their own order values.</summary>
    [Fact]
    public void EntriesOfACategoryAreOrderedByTheirOwnOrder()
    {
        var menu = Built(
            Words(("menu.entry.copy", "Copy"), ("menu.entry.b", "B"), ("menu.entry.a", "A")),
            new MenuLeaf(["menu.entry.copy", "menu.entry.b"], 20, () => { }),
            new MenuLeaf(["menu.entry.copy", "menu.entry.a"], 10, () => { })
        );

        Assert.Equal(["A", "B"], Headers(Assert.Single(MenuItems(menu))));
    }

    /// <summary>A path of more than two keys is folded a category at a time.</summary>
    [Fact]
    public void ADeeperPathIsFoldedAtEveryLevel()
    {
        var menu = Built(
            Words(
                ("menu.entry.copy", "Copy"),
                ("menu.entry.path", "Path"),
                ("menu.entry.file", "File"),
                ("menu.entry.other", "Other")
            ),
            new MenuLeaf(["menu.entry.copy", "menu.entry.path", "menu.entry.file"], 0, () => { }),
            new MenuLeaf(["menu.entry.copy", "menu.entry.path", "menu.entry.other"], 0, () => { })
        );

        var copy = Assert.Single(MenuItems(menu));
        var path = Assert.Single(MenuItems(copy));

        Assert.Equal("Copy", Header(copy));
        Assert.Equal("Path", Header(path));
        Assert.Equal(["File", "Other"], Headers(path));
    }

    /// <summary>A key no file states reads as its own last part, with the underscores turned into spaces.</summary>
    [Fact]
    public void AnUntranslatedKeyFallsBackToItsLastPart()
    {
        var menu = Built(
            Words(("menu.entry.copy", "Copy")),
            new MenuLeaf(["menu.entry.copy", "menu.entry.some_thing"], 0, () => { }),
            new MenuLeaf(["menu.entry.other", "menu.entry.another_one"], 0, () => { })
        );

        Assert.Equal(["Copy Some thing", "Other Another one"], Headers(menu));
    }

    /// <summary>Two leaves of one path are one entry, and the first one is kept.</summary>
    [Fact]
    public void ADuplicatePathKeepsTheFirstOneAndSaysSo()
    {
        var warnings = new List<string>();
        var renderer = new MenuRenderer(key => key, warn: warnings.Add);
        var menu = new ContextMenu();

        renderer.Fill(
            menu,
            [
                new MenuLeaf(["menu.entry.cut", "menu.entry.file"], 0, () => { }),
                new MenuLeaf(["menu.entry.cut", "menu.entry.file"], 0, () => { }),
            ],
            force: false
        );

        Assert.Single(MenuItems(menu));
        Assert.Single(warnings);
    }

    /// <summary>
    /// A forced entry is counted when the category is made, so the structure does not change when Shift is held.
    /// </summary>
    [Fact]
    public void AForcedEntryCountsForTheStructureAndIsHiddenUntilHeld()
    {
        MenuLeaf[] leaves =
        [
            new(["menu.entry.ownership", "menu.entry.claim"], 10, () => { }),
            new(["menu.entry.ownership", "menu.entry.force_claim"], 20, () => { }, Force: true),
        ];

        var shown = Built(
            Words(("menu.entry.ownership", "Ownership"), ("menu.entry.claim", "Claim"), ("menu.entry.force_claim", "Force claim")),
            leaves,
            force: false
        );
        var held = Built(
            Words(("menu.entry.ownership", "Ownership"), ("menu.entry.claim", "Claim"), ("menu.entry.force_claim", "Force claim")),
            leaves,
            force: true
        );

        var submenu = Assert.Single(MenuItems(shown));
        var entries = MenuItems(submenu);

        Assert.Equal("Ownership", Header(submenu));
        Assert.Equal(["Claim", "Force claim"], Headers(submenu));
        Assert.True(entries[0].IsVisible);
        Assert.False(entries[1].IsVisible);
        Assert.All(MenuItems(Assert.Single(MenuItems(held))), item => Assert.True(item.IsVisible));
    }

    /// <summary>A category nothing applies to is not drawn at all.</summary>
    [Fact]
    public void ACategoryWithNothingApplicableIsNotDrawn()
    {
        var menu = Built(
            Words(("menu.entry.copy", "Copy"), ("menu.entry.file", "File")),
            new MenuLeaf(["menu.entry.copy", "menu.entry.file"], 0, () => { }, Applies: () => false)
        );

        Assert.Empty(menu.Items);
    }

    /// <summary>An entry that may not be chosen is drawn and cannot be pressed.</summary>
    [Fact]
    public void AnEntryThatMayNotBeChosenIsDrawnAndDisabled()
    {
        var menu = Built(
            Words(("menu.entry.copy", "Copy"), ("menu.entry.file", "File")),
            new MenuLeaf(["menu.entry.copy", "menu.entry.file"], 0, () => { }, Enabled: () => false)
        );

        Assert.False(Assert.Single(MenuItems(menu)).IsEnabled);
    }

    /// <summary>A path of one key is an entry of its own, with no category and no rule around it.</summary>
    [Fact]
    public void APathOfOneKeyIsAnEntryOfItsOwn()
    {
        var menu = Built(
            Words(("menu.dock.close", "Close"), ("menu.dock.own_window", "Own window")),
            new MenuLeaf(["menu.dock.close"], 20, () => { }),
            new MenuLeaf(["menu.dock.own_window"], 10, () => { })
        );

        Assert.Equal(["Close", "Own window"], Headers(menu));
        Assert.Empty(menu.Items.OfType<Separator>());
    }

    /// <summary>Categories are separated from one another.</summary>
    [Fact]
    public void CategoriesAreSeparated()
    {
        var menu = Built(
            Words(("menu.entry.copy", "Copy"), ("menu.entry.delete", "Delete"), ("menu.entry.item", "Item")),
            new MenuLeaf(["menu.entry.copy", "menu.entry.item"], 0, () => { }),
            new MenuLeaf(["menu.entry.delete", "menu.entry.item"], 0, () => { })
        );

        Assert.Single(menu.Items.OfType<Separator>());
    }

    /// <summary>A menu made of a list of leaves and a dictionary of words.</summary>
    /// <param name="words">What a key reads as, where a file states it.</param>
    /// <param name="leaves">The leaves.</param>
    /// <returns>The menu.</returns>
    private static ContextMenu Built(Dictionary<string, string> words, params MenuLeaf[] leaves) =>
        Built(words, leaves, force: false);

    /// <summary>A menu made of a list, a dictionary of words and whether Shift is held.</summary>
    /// <param name="words">What a key reads as, where a file states it.</param>
    /// <param name="leaves">The leaves.</param>
    /// <param name="force">Whether the forced entries are shown.</param>
    /// <returns>The menu.</returns>
    private static ContextMenu Built(Dictionary<string, string> words, MenuLeaf[] leaves, bool force)
    {
        var menu = new ContextMenu();

        new MenuRenderer(key => words.GetValueOrDefault(key) ?? key).Fill(menu, leaves, force);

        return menu;
    }

    /// <summary>A dictionary of words, case for case.</summary>
    /// <param name="pairs">Each a key and what it reads as.</param>
    /// <returns>The words.</returns>
    private static Dictionary<string, string> Words(params (string Key, string Word)[] pairs) =>
        pairs.ToDictionary(pair => pair.Key, pair => pair.Word, StringComparer.Ordinal);

    /// <summary>The entries a menu or a submenu holds.</summary>
    /// <param name="control">The menu, or a submenu.</param>
    /// <returns>Its entries, in the order they are drawn.</returns>
    private static IReadOnlyList<MenuItem> MenuItems(Control control) =>
        control switch
        {
            ContextMenu menu => [.. menu.Items.OfType<MenuItem>()],
            MenuItem item => [.. item.Items.OfType<MenuItem>()],
            _ => throw new ArgumentOutOfRangeException(nameof(control)),
        };

    /// <summary>What the entries of a menu are headed.</summary>
    /// <param name="menu">The menu.</param>
    /// <returns>Each header, in the order they are drawn.</returns>
    private static IReadOnlyList<string> Headers(ContextMenu menu) =>
        [.. MenuItems(menu).Select(Header)];

    /// <summary>What the entries of a submenu are headed.</summary>
    /// <param name="menu">The submenu.</param>
    /// <returns>Each header, in the order they are drawn.</returns>
    private static IReadOnlyList<string> Headers(MenuItem menu) =>
        [.. MenuItems(menu).Select(Header)];

    /// <summary>What one entry is headed.</summary>
    /// <param name="item">The entry.</param>
    /// <returns>The header as a word.</returns>
    private static string Header(MenuItem item) => (string)item.Header!;
}
