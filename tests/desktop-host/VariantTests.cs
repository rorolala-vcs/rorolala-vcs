using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.I18n;
using RorolalaVCSPlugin;
// Both plugins publish an `Icons`, so what this plugin draws its pictures under is named by the one it is,
// and both publish an `Operation`, so the Overview's own is named too.
using VcsIcons = RorolalaVCSPlugin.Icons;
using Operation = RorolalaVCSPlugin.Operation;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A variant file: which merge it belongs to, and what a listing and a tile say about it.
/// </summary>
/// <remarks>
/// A merge in progress is a target the Layout names and a variant file beside it that neither Layout names,
/// so the file is the one thing about a merge no path of the work answers. It is read from the reading's own
/// list of merges, and what is checked here is that the three places that care — the tile's badge, the
/// Upstream column, and the Overview's button — find the same file: what a reader sees on a tile and what a
/// button runs have to be about one file.
/// </remarks>
public sealed class VariantTests
{
    /// <summary>Where the Workspace the readings below were made of is rooted.</summary>
    private const string Root = "/work";

    /// <summary>The merge a variant file belongs to, as a reading reports one.</summary>
    /// <param name="path">Where the record puts the variant file.</param>
    /// <param name="moved">Where the tree holds it, when a move was found.</param>
    /// <returns>The merge.</returns>
    private static Merge Held(string path, string? moved = null) =>
        new("art/hero.psd", "1a2b3c4d5e6f", path, moved);

    /// <summary>A variant file is the one a merge names, whether the record or the tree says where it is.</summary>
    /// <remarks>
    /// What a reader is looking at is the file the tree holds, so a merge whose variant file moved is read
    /// from where it moved to: the record is what `rola align --rename` settles, and a badge or a button that
    /// went by the record would be about a path with no file at it.
    /// </remarks>
    [Fact]
    public void AVariantFileIsTheOneAMergeNames()
    {
        var kept = Held("art/hero.1a2b3c.txt");
        var moved = Held("art/hero.1a2b3c.txt", "art/elsewhere/hero.txt");

        Assert.Equal(kept, VariantFiles.At([kept], Root, "/work/art/hero.1a2b3c.txt"));
        Assert.Equal(moved, VariantFiles.At([moved], Root, "/work/art/elsewhere/hero.txt"));

        // The place the record remembers is not a file any more once the reading has found the move, and a
        // path that is no variant file of any merge is nothing.
        Assert.Null(VariantFiles.At([moved], Root, "/work/art/hero.1a2b3c.txt"));
        Assert.Null(VariantFiles.At([kept], Root, "/work/art/hero.psd"));
        Assert.Null(VariantFiles.At([], Root, "/work/art/hero.1a2b3c.txt"));

        // A path outside the Workspace is nothing a reading is about, however much of it looks alike.
        Assert.Null(VariantFiles.At([kept], Root, "/elsewhere/art/hero.1a2b3c.txt"));
    }

    /// <summary>A variant is known by the head of the hash it is named by.</summary>
    [Fact]
    public void AVariantIsKnownByTheHeadOfItsHash()
    {
        Assert.Equal("1a2b3c4", VariantFiles.Head("1a2b3c4d5e6f"));
        Assert.Equal("1a2b", VariantFiles.Head("1a2b"));
        Assert.Equal(string.Empty, VariantFiles.Head(string.Empty));
    }

    /// <summary>A variant file's tile wears the merge badge, in the amber of work under way.</summary>
    /// <remarks>
    /// The badge is a picture rather than a word, so what it is drawn as is the icon library's to hold and
    /// what it means is on it: which variant is waiting, which is the one thing about the file that no path
    /// of the work says.
    /// </remarks>
    [Fact]
    public void AVariantFilesTileWearsTheMergeBadge()
    {
        _ = Host.Services();

        var mark = RorolalaMergeBadge.Say(Held("art/hero.1a2b3c.txt"));

        Assert.Equal(VcsIcons.Merge, mark.IconKey);
        Assert.Equal(MarkInk.Warn, mark.Ink);

        // Worn on the badge's own ground, as every mark of a tile is: a bar picture beside a badge would be
        // two kinds of mark about one file.
        Assert.True(mark.Tagged);

        // The words are the mark's own rather than a phrase read around them, so what is hovered for is what
        // the badge says and not a key it names.
        Assert.Null(mark.HoverKey);
        Assert.Equal(
            RolaI18N.Get("rorolala_vcs.variant.pending", VariantFiles.Head("1a2b3c4d5e6f")),
            mark.Hover()
        );
    }

    /// <summary>A variant file is named in the Upstream column, which is the only name it has.</summary>
    /// <remarks>
    /// The column says where the Vault names a file, and it names a variant file nowhere: what it says instead
    /// is the variant the file holds, in the amber a merge is drawn in everywhere else. It is the cell's words
    /// that say it, since a column of values is read and not hovered over.
    /// </remarks>
    [Fact]
    public void AVariantFileIsNamedInTheUpstreamColumn()
    {
        _ = Host.Services();

        var mark = RorolalaUpstream.Named(Held("art/hero.1a2b3c.txt"));
        var head = VariantFiles.Head("1a2b3c4d5e6f");

        Assert.Equal(RolaI18N.Get("rorolala_vcs.variant.named", head), mark.Text);
        Assert.Equal(RorolalaColours.UnderWay, mark.Colour);
        Assert.Null(mark.Hint);

        // A value rather than a mark: what the column is for is naming a file, and a dot in it would be a
        // second kind of thing in a column of names.
        Assert.False(mark.Dot);
    }

    /// <summary>A merge can be ended from the Overview, and a merge in progress or a broken one alike.</summary>
    /// <remarks>
    /// What ends a merge is the same whether it is waiting or cannot go on: the variant file is taken away and
    /// the record confirmed gone. A merge that cannot go on is the one a reader is likelier to want rid of,
    /// since it is the one that is stopping the work.
    /// </remarks>
    [Fact]
    public void AMergeCanBeEndedFromTheOverview()
    {
        _ = Host.Services();

        Assert.Equal([Operation.CancelMerge], OverviewOperations.Of(State.Merging));
        Assert.Equal([Operation.CancelMerge], OverviewOperations.Of(State.Broken));

        Assert.Equal(
            "rorolala_vcs.operations.cancel_merge",
            OverviewOperations.Key(Operation.CancelMerge)
        );

        // A merge is no longer a state with nothing to press, and what it offers is not what any other state
        // of a file does: a file that moved and was edited is two things, and this is neither of them.
        Assert.DoesNotContain(Operation.CancelMerge, OverviewOperations.Of(State.Modified));
        Assert.DoesNotContain(Operation.CancelMerge, OverviewOperations.Of(State.Untagged));
    }
}
