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
