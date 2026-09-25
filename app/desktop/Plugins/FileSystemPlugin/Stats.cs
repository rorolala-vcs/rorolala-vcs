using System.Globalization;
using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>What the columns of a table say about one entry, besides its name.</summary>
/// <param name="Permissions">The nine letters, or empty where there is nothing to say.</param>
/// <param name="Modified">When it was last written, or empty where it cannot be asked.</param>
/// <param name="Size">How large it is, or empty where its size is not its own to tell.</param>
internal sealed record Facts(string Permissions, string Modified, string Size);

/// <summary>
/// What a table's columns say about an entry besides its name: its permissions, when it was last written,
/// and how large it is.
/// </summary>
/// <remarks>
/// Read from the file when a row is drawn rather than while the directory is listed. A directory of
/// thousands is worth listing whether or not anybody looks at it, and the rows on screen are a handful —
/// so the asking is done where the looking is, and a long listing costs what is seen of it.
/// <para>
/// Every answer is an answer or a blank: a file that has gone, or that cannot be asked about, leaves its
/// cells empty rather than stopping the row from being drawn at all. The blanks with nothing to say are
/// the way up and the computer, which are places rather than files.
/// </para>
/// </remarks>
internal static class Stats
{
    /// <summary>What a cell says when it has nothing to say.</summary>
    private const string Blank = "";

    /// <summary>How a time is written: to the minute, in an order no reader can mistake.</summary>
    private const string Stamp = "yyyy-MM-dd HH:mm";

    /// <summary>The nine letters a permissions column is read in, in the order they are written.</summary>
    private static readonly (UnixFileMode Mode, char Letter)[] Permissions =
    [
        (UnixFileMode.UserRead, 'r'),
        (UnixFileMode.UserWrite, 'w'),
        (UnixFileMode.UserExecute, 'x'),
        (UnixFileMode.GroupRead, 'r'),
        (UnixFileMode.GroupWrite, 'w'),
        (UnixFileMode.GroupExecute, 'x'),
        (UnixFileMode.OtherRead, 'r'),
        (UnixFileMode.OtherWrite, 'w'),
        (UnixFileMode.OtherExecute, 'x'),
    ];

    /// <summary>The units a size is read in, smallest first.</summary>
    private static readonly string[] Units = ["B", "KB", "MB", "GB", "TB"];

    /// <summary>
    /// What the table's columns say about one entry.
    /// </summary>
    /// <param name="entry">The entry to ask about.</param>
    /// <returns>What each column says, each empty when there is nothing to say.</returns>
    public static Facts Of(Entry entry) =>
        Browser.IsUp(entry.Path) || Browser.IsComputer(entry.Path)
            ? new Facts(Blank, Blank, Blank)
            : new Facts(PermissionsOf(entry), ModifiedOf(entry), SizeOf(entry));

    /// <summary>
    /// The nine permission letters, or nothing where the system has no such thing.
    /// </summary>
    /// <remarks>
    /// Unix only. Windows does not carry what these letters say, and a column answering there anyway would
    /// be answering about something else.
    /// </remarks>
    /// <param name="entry">The entry to ask about.</param>
    private static string PermissionsOf(Entry entry)
    {
        if (OperatingSystem.IsWindows())
        {
            return Blank;
        }

        try
        {
            var mode = File.GetUnixFileMode(entry.Path);
            var letters = new char[Permissions.Length];

            for (var at = 0; at < Permissions.Length; at++)
            {
                var (flag, letter) = Permissions[at];

                letters[at] = (mode & flag) == flag ? letter : '-';
            }

            return new string(letters);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return Blank;
        }
    }

    /// <summary>
    /// When the entry was last written.
    /// </summary>
    /// <remarks>
    /// The same call answers for either kind, so a directory's row says when the directory itself changed
    /// rather than when anything in it did.
    /// </remarks>
    /// <param name="entry">The entry to ask about.</param>
    private static string ModifiedOf(Entry entry)
    {
        try
        {
            return File.GetLastWriteTime(entry.Path).ToString(Stamp, CultureInfo.InvariantCulture);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return Blank;
        }
    }

    /// <summary>
    /// How large the entry is, in units of a thousand and twenty-four.
    /// </summary>
    /// <remarks>
    /// A directory says nothing: how large one is is how large everything under it is, which is a walk of
    /// the whole tree for one cell, and the number a system reports for the entry itself is about its own
    /// bookkeeping rather than about what is in it.
    /// </remarks>
    /// <param name="entry">The entry to ask about.</param>
    private static string SizeOf(Entry entry)
    {
        if (entry.Kind == EntryKind.Directory)
        {
            return Blank;
        }

        try
        {
            var size = (double)new FileInfo(entry.Path).Length;
            var unit = 0;

            while (size >= 1024 && unit < Units.Length - 1)
            {
                size /= 1024;
                unit += 1;
            }

            // Whole bytes are written whole — no file is 512.0 bytes — and anything larger keeps one
            // fraction, which is as much as a column of a fixed width is worth.
            var written = unit == 0
                ? size.ToString("0", CultureInfo.InvariantCulture)
                : size.ToString("0.#", CultureInfo.InvariantCulture);

            return $"{written} {Units[unit]}";
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            return Blank;
        }
    }
}
