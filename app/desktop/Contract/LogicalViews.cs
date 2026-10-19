using System.Text.RegularExpressions;
using Avalonia.Controls;
using Avalonia.Media;

namespace RorolalaDesktop.Contract;

/// <summary>
/// One thing being dragged: an object the source made, with its type rubbed out.
/// </summary>
/// <remarks>
/// Rubbed out on purpose. A drag crosses between views that know nothing of each other — the file tree, a
/// plugin's own listing, whatever is added next — and a contract that named every kind of thing that could be
/// dragged would have to be changed by every one of them. So the source hands over its own object and the view
/// under the pointer decides whether it is something it can take: what a drop means is the dropped-on view's
/// question, which is also why nothing here says how a drag is drawn.
/// </remarks>
/// <param name="Data">What the source made, which the target answers for.</param>
public sealed record LogicalDrag(object Data);

/// <summary>
/// A place a plugin answers for, written as its prefix and everything after the first <c>@</c>.
/// </summary>
/// <remarks>
/// The payload is not read here and has no grammar: it is one string, handed over whole, because what is inside
/// it is the answer's own business — <c>rola@/some/workspace@vault</c> is one address whose middle is a
/// directory and whose end is which of that directory's views is wanted, and only the plugin that answers
/// <c>rola</c> knows the two apart.
/// </remarks>
/// <param name="Scheme">The prefix, which is what the registry finds a provider by.</param>
/// <param name="Payload">Everything after the first <c>@</c>, unread.</param>
public sealed record LogicalAddress(string Scheme, string Payload)
{
    /// <summary>Whether a prefix is one a plugin may claim.</summary>
    /// <remarks>
    /// One or more lower-case segments joined by a dot, each opening with a letter: the shape of an identity
    /// elsewhere in this contract, so that a prefix cannot be something that reads as part of the payload — a
    /// space, an <c>@</c>, or a path separator.
    /// </remarks>
    private static readonly Regex Grammar = new(
        "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)*$",
        RegexOptions.CultureInvariant
    );

    /// <summary>Whether a prefix is one a plugin may claim.</summary>
    /// <param name="scheme">The prefix to read.</param>
    /// <returns>Whether it is well formed.</returns>
    public static bool IsScheme(string scheme) =>
        !string.IsNullOrEmpty(scheme) && Grammar.IsMatch(scheme);

    /// <summary>
    /// Reads an address, as somebody typed it into the path bar.
    /// </summary>
    /// <remarks>
    /// Nothing is refused here: what is not an address at all is not one, and a prefix this program knows nothing
    /// about is an address with no provider, which is a thing to say in the dock rather than a parse failure. The
    /// split is at the <b>first</b> <c>@</c>, since a payload may hold more of them.
    /// </remarks>
    /// <param name="written">What was typed.</param>
    /// <param name="address">The address, when what was typed is one.</param>
    /// <returns>Whether what was typed is an address at all.</returns>
    public static bool TryRead(string written, out LogicalAddress? address)
    {
        address = null;

        if (written is null || written.IndexOf('@', StringComparison.Ordinal) is not (> 0 and var at))
        {
            return false;
        }

        var scheme = written[..at];

        if (!IsScheme(scheme))
        {
            return false;
        }

        address = new LogicalAddress(scheme, written[(at + 1)..]);

        return true;
    }

    /// <summary>This address as it is written: the prefix, the <c>@</c>, and the payload.</summary>
    /// <returns>What to put in the path bar.</returns>
    public override string ToString() => $"{Scheme}@{Payload}";
}

/// <summary>
/// One entry of a logical view: the entry is data, everything about it is the provider's.
/// </summary>
/// <remarks>
/// A logical view is a place a plugin answers for rather than a directory, so what is in it is not a file and
/// has no size or time: what it has is what its provider says it has. The list is drawn by the file browser, so
/// that a place is read the same way wherever it is, while what is drawn in it — the name, the picture, what a
/// click does, what the menu offers — is the provider's answer.
/// <para>
/// What an entry carries beyond its words is its to interpret: <see cref="Data"/> is handed back to the provider
/// when the entry is used, so that a provider may keep a key of its own in it rather than parsing its own name
/// back out.
/// </para>
/// </remarks>
/// <param name="Name">What the entry is called, which is what is drawn.</param>
/// <param name="IconKey">A picture in the icon library, or nothing for no picture.</param>
/// <param name="Draggable">Whether the entry may be picked up and dropped somewhere else.</param>
/// <param name="Opens">Where opening it goes, or nothing when it is not a place.</param>
/// <param name="Menu">What the menu opened on it offers, or nothing for nothing at all.</param>
/// <param name="Data">
/// What the provider keeps about it, handed back when the entry is used, which is what lets two entries of a
/// place share a name.
/// </param>
/// <param name="Picture">
/// What to draw where the picture goes, for a picture the icon library holds no key for — an entry whose look is
/// chosen from its name, which is what a file's extension is. Answered beside <paramref name="IconKey"/> rather
/// than instead of it: a key is a picture the library already holds, and a maker is one the answer draws for the
/// size it is asked for.
/// </param>
/// <remarks>
/// What an entry looks like when neither is given is the view's answer, and it is the folder the icon set in force
/// draws: a place's rows are places and things, and a thing drawn as nothing is a row a reader has to read rather
/// than see.
/// </remarks>
public sealed record LogicalEntry(
    string Name,
    string? IconKey = null,
    bool Draggable = true,
    LogicalAddress? Opens = null,
    IReadOnlyList<ContextMenuItem>? Menu = null,
    object? Data = null,
    Func<int, Control>? Picture = null
);

/// <summary>
/// What a plugin answers about the places it offers.
/// </summary>
/// <remarks>
/// Four questions and no more: what the place is called, what is in it, whether a drag is something it can take,
/// and taking it. Everything else about a logical view — what a double click does, what a menu offers, whether an
/// entry may be picked up — is said by the entries themselves, which is what keeps this from growing a method per
/// feature a plugin might have.
/// <para>
/// Asked on the window's thread, and asked often: a listing reads the entries whenever it draws, so an answer
/// that reaches a Vault belongs in something the provider caches rather than in the answer itself.
/// </para>
/// </remarks>
public interface ILogicalViewProvider
{
    /// <summary>What the place is called, which is what the path bar shows while it is open.</summary>
    /// <param name="address">The address being opened.</param>
    /// <returns>What to call it.</returns>
    string Title(LogicalAddress address);

    /// <summary>What is in the place, in the order it is shown.</summary>
    /// <param name="address">The address being opened.</param>
    /// <returns>The entries.</returns>
    IReadOnlyList<LogicalEntry> Entries(LogicalAddress address);

    /// <summary>
    /// Where going up from a place goes, or nothing when there is nowhere further up.
    /// </summary>
    /// <remarks>
    /// Asked rather than worked out, because only the provider knows what is over an address: a payload has no
    /// grammar here, so <c>rola@/work/place@vault</c> may be read apart by the plugin that answers <c>rola</c>
    /// and by nobody else. Nothing is the top of a place, which is what a listing with no step out of it means —
    /// and what the toolbar's way-up button goes dead on.
    /// </remarks>
    /// <param name="address">The address being asked about.</param>
    /// <returns>The address over it, or nothing.</returns>
    LogicalAddress? Parent(LogicalAddress address);

    /// <summary>
    /// Whether a drag is something this place can take.
    /// </summary>
    /// <remarks>
    /// Asked while the pointer is still moving, which is what decides whether the drop is offered at all: a place
    /// that would refuse it says so here rather than accepting and then saying nothing happened.
    /// </remarks>
    /// <param name="address">The place being dropped on.</param>
    /// <param name="drag">What is being dragged.</param>
    /// <returns>Whether it can be taken.</returns>
    bool Accepts(LogicalAddress address, LogicalDrag drag);

    /// <summary>Takes a drag that <see cref="Accepts"/> answered for.</summary>
    /// <param name="address">The place being dropped on.</param>
    /// <param name="drag">What is being dragged.</param>
    void Drop(LogicalAddress address, LogicalDrag drag);

    /// <summary>
    /// What a place says about each of its entries, beyond the name and the picture.
    /// </summary>
    /// <remarks>
    /// A place's rows are drawn by the browser's own table, so what a row is worth saying about it is the
    /// answerer's: an entry of a Vault has a uuid and a holder, and one of a directory has a size and a time — the
    /// table does not know which, and a place that states nothing is a place read by its names alone.
    /// </remarks>
    /// <param name="address">The place being read.</param>
    /// <returns>The columns, in the order they are shown, or nothing for a place read by its names.</returns>
    IReadOnlyList<LogicalColumn> Columns(LogicalAddress address);
}

/// <summary>One column a place shows beside the name and the picture.</summary>
/// <remarks>
/// The heading is a key rather than the words, so that it is said in the reader's own language and stays in step
/// with the catalogue like every other line a plugin draws. What the column says about an entry is the
/// provider's answer, which is why the cell is drawn from the entry rather than from a value the table keeps.
/// </remarks>
/// <param name="HeaderKey">An i18n key for the column's heading.</param>
/// <param name="Width">How wide the column starts.</param>
/// <param name="Value">What the column says about one entry, which is the empty string for an entry it says nothing about.</param>
/// <param name="Align">Where the text sits in the column.</param>
/// <param name="BesideName">
/// Whether the column is read with the name, before the room the table has to spare, rather than with the values
/// at the far side of it: a second name of the entry belongs with the name, where a fact about it does not.
/// </param>
public sealed record LogicalColumn(
    string HeaderKey,
    double Width,
    Func<LogicalEntry, string> Value,
    TextAlignment Align = TextAlignment.Left,
    bool BesideName = false
);

/// <summary>
/// A directory that takes a drag which is not files: a place's entries dropped into the work.
/// </summary>
/// <remarks>
/// The other direction of <see cref="ILogicalViewProvider"/>: a place is dropped <em>on</em>, while a directory
/// takes what was dropped <em>from</em> one. What a directory does with it is not the file browser's to decide — a
/// place's entry is not a file, and what it means to put one in a directory is the plugin's answer — so a plugin
/// says which directories it takes one in and does the work itself.
/// <para>
/// Asked while the pointer is still moving, so that a directory which would refuse it does not offer the drop at
/// all.
/// </para>
/// </remarks>
public interface IDropTarget
{
    /// <summary>Whether a directory can take what a drag carries.</summary>
    /// <param name="directory">The directory it is being dropped on.</param>
    /// <param name="drag">What is being dragged.</param>
    /// <returns>Whether it can be taken.</returns>
    bool Accepts(string directory, LogicalDrag drag);

    /// <summary>Takes a drag that <see cref="Accepts"/> answered for.</summary>
    /// <param name="directory">The directory it was dropped on.</param>
    /// <param name="drag">What was dragged.</param>
    /// <returns>
    /// Whether it was taken, which is what the source is told so that it does not act on what it carried as well.
    /// </returns>
    bool Drop(string directory, LogicalDrag drag);
}

/// <summary>Where a plugin offers what it takes a non-file drag dropped on a directory for.</summary>
public interface IDropTargetRegistry
{
    /// <summary>Offers a target.</summary>
    /// <param name="target">What answers for the directories it takes.</param>
    void Add(IDropTarget target);

    /// <summary>Every target offered, in the order they are asked.</summary>
    IReadOnlyList<IDropTarget> Items { get; }
}

/// <summary>
/// What a place can say about a drag it will not take.
/// </summary>
/// <remarks>
/// A place that refused a drop silently is a place the user cannot tell from one the drag never reached, and "no" with
/// no word is the least useful answer a place can give: the reasons are the place's own — the entry is already there,
/// a path there is taken, the copy is still being read — and none of them is something the browser could work out.
/// <para>
/// Optional, because a provider may have nothing to add: one that does not answer is shown with the one thing the
/// browser knows, which is that the place did not take the drag.
/// </para>
/// </remarks>
public interface ILogicalRefusal
{
    /// <summary>Why a place would refuse a drag, in the words of the place.</summary>
    /// <param name="address">The place being dropped on.</param>
    /// <param name="drag">What is being dragged.</param>
    /// <returns>The words, or nothing when the place has nothing to add to a plain refusal.</returns>
    string? Refusal(LogicalAddress address, LogicalDrag drag);
}

/// <summary>
/// Where a plugin offers the places it answers for, by the prefix their addresses begin with.
/// </summary>
/// <remarks>
/// A prefix is claimed rather than assigned, because a prefix is the shortest thing a person types and the
/// question of who owns it is a question about meaning: <c>rola</c> is the version control system's, and a plugin
/// that registers it is saying these are the places Rorolala itself knows. The first plugin to claim one answers
/// it, so a later one cannot take a prefix away by being started.
/// </remarks>
public interface ILogicalViewRegistry
{
    /// <summary>Offers a provider, which answers every address beginning with a prefix.</summary>
    /// <param name="scheme">The prefix, which must be of the shape <see cref="LogicalAddress.IsScheme"/>.</param>
    /// <param name="provider">What answers for it.</param>
    void Register(string scheme, ILogicalViewProvider provider);

    /// <summary>What answers a prefix, or nothing when no plugin does.</summary>
    /// <param name="scheme">The prefix.</param>
    /// <returns>The provider, or nothing.</returns>
    ILogicalViewProvider? Find(string scheme);

    /// <summary>Every prefix that has an answer, in the order they were offered.</summary>
    IReadOnlyList<string> Schemes { get; }
}
