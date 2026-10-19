using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// The file the user last clicked in a directory listing.
/// </summary>
/// <remarks>
/// The listing is this plugin's, and a view of one file's history is another plugin's: what crosses
/// is the one answer both can agree on — which file the most recent click was on. A click on a
/// directory says nothing here, since a version chain hangs from a file and not from a place, and
/// the file clicked before it stays the answer.
/// <para>
/// It is a catalogue rather than a member of a control because there are several listings — every
/// directory dock, and every one taken out of step — and the most recent click between them is one
/// answer for the program.
/// </para>
/// </remarks>
public static class ChosenEntry
{
    /// <summary>The file clicked most recently, or nothing before any has been.</summary>
    public static Entry? Current { get; private set; }

    /// <summary>Raised when <see cref="Current"/> says something else.</summary>
    public static event Action? Changed;

    /// <summary>
    /// Says that a listing's entry was clicked, which is the file now being asked about.
    /// </summary>
    /// <remarks>
    /// A click on the file already said is not said again: a reader that redraws on every click
    /// would rebuild one reading for the same answer.
    /// </remarks>
    /// <param name="entry">The entry the click landed on.</param>
    public static void Publish(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        if (entry.Kind != EntryKind.File)
        {
            return;
        }

        if (Current is { } known && string.Equals(known.Path, entry.Path, StringComparison.Ordinal))
        {
            return;
        }

        Current = entry;
        Changed?.Invoke();
    }
}

/// <summary>
/// The files chosen in a directory listing, which is the set a view of the work acts on.
/// </summary>
/// <remarks>
/// <see cref="ChosenEntry"/> is the one file clicked most recently and is what a view of one
/// file's history follows; a choice may be several files, and what is done with the choice — the
/// track a Version Graph offers, say — is about all of them at once. The two are catalogues apart
/// because they answer different questions: the last click stays the file the graph is of even
/// when the choice beside it is empty, and the choice is nothing when the user clicks the space
/// around the entries without changing what was clicked.
/// <para>
/// Only files are published. A directory is not a version chain, and a step up is a place rather
/// than work: a choice of either leaves the files of it out, which is what the callers that track or
/// read a chain want anyway.
/// </para>
/// </remarks>
public static class ChosenFiles
{
    /// <summary>The files in the choice, in the order the listing shows them.</summary>
    public static IReadOnlyList<Entry> Current { get; private set; } = [];

    /// <summary>Raised when <see cref="Current"/> says something else.</summary>
    public static event Action? Changed;

    /// <summary>
    /// Says what the choice holds now, so that a view of the work follows it.
    /// </summary>
    /// <remarks>
    /// A catalogue like <see cref="ChosenEntry"/> rather than a member of one view, because there
    /// are several listings and what the program acts on is the choice the user made last. What is
    /// said twice the same is not said again: a listing rebuilt with the choice it already had is a
    /// redraw and not a new choice.
    /// </remarks>
    /// <param name="chosen">The entries the listing has chosen.</param>
    public static void Publish(IReadOnlyList<Entry> chosen)
    {
        ArgumentNullException.ThrowIfNull(chosen);

        var files = chosen.Where(entry => entry.Kind == EntryKind.File).ToArray();

        if (Same(Current, files))
        {
            return;
        }

        Current = files;
        Changed?.Invoke();
    }

    /// <summary>Whether two choices name the same files in the same order.</summary>
    /// <param name="left">One choice.</param>
    /// <param name="right">The other.</param>
    /// <returns>Whether they are the same answer.</returns>
    private static bool Same(IReadOnlyList<Entry> left, IReadOnlyList<Entry> right)
    {
        if (left.Count != right.Count)
        {
            return false;
        }

        for (var at = 0; at < left.Count; at++)
        {
            if (!string.Equals(left[at].Path, right[at].Path, StringComparison.Ordinal))
            {
                return false;
            }
        }

        return true;
    }
}
