namespace SightPlugin;

/// <summary>
/// What has been read, kept by path and the file it was read from, so the pointer moving around a
/// file does not read it again.
/// </summary>
/// <remarks>
/// An entry is keyed by both files of a preview and by their write times and lengths, so a changed
/// file is a new key rather than a stale picture. The newest answer stands at the front; the oldest
/// is let go of, and a picture it holds is disposed with it.
/// </remarks>
internal sealed class SightCache
{
    /// <summary>How many answers are kept before the oldest is dropped.</summary>
    private const int Kept = 16;

    /// <summary>What is kept, newest first.</summary>
    private readonly List<(Key Key, SightDocument Document)> _entries = [];

    /// <summary>The answer kept for these files, when one is.</summary>
    /// <param name="primary">The file chosen last.</param>
    /// <param name="second">The other chosen file, when two are chosen.</param>
    /// <param name="choice">The previewer in force, or `auto`, which is part of the key.</param>
    /// <param name="document">The answer, when one is kept.</param>
    /// <returns>Whether an answer was kept.</returns>
    public bool TryGet(SightFile primary, SightFile? second, string choice, out SightDocument? document)
    {
        ArgumentNullException.ThrowIfNull(primary);

        var key = MakeKey(primary, second, choice);
        var at = _entries.FindIndex(entry => entry.Key == key);

        if (at < 0)
        {
            document = null;

            return false;
        }

        var found = _entries[at];
        _entries.RemoveAt(at);
        _entries.Insert(0, found);
        document = found.Document;

        return true;
    }

    /// <summary>Keeps an answer, letting the oldest go when there are too many.</summary>
    /// <param name="primary">The file chosen last.</param>
    /// <param name="second">The other chosen file, when two are chosen.</param>
    /// <param name="choice">The previewer in force, or `auto`, which is part of the key.</param>
    /// <param name="document">The answer.</param>
    public void Keep(SightFile primary, SightFile? second, string choice, SightDocument document)
    {
        ArgumentNullException.ThrowIfNull(primary);
        ArgumentNullException.ThrowIfNull(document);

        var key = MakeKey(primary, second, choice);

        _entries.RemoveAll(entry => entry.Key == key);
        _entries.Insert(0, (key, document));

        while (_entries.Count > Kept)
        {
            var oldest = _entries[^1];
            _entries.RemoveAt(_entries.Count - 1);
            Dispose(oldest.Document);
        }
    }

    /// <summary>Lets go of every answer read from one directory.</summary>
    /// <param name="directory">The directory whose files may have changed.</param>
    public void ForgetDirectory(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        _entries.RemoveAll(entry =>
        {
            if (!string.Equals(Directory(entry.Key.Primary), directory, StringComparison.Ordinal))
            {
                return false;
            }

            Dispose(entry.Document);

            return true;
        });
    }

    /// <summary>Lets go of every answer.</summary>
    public void Clear()
    {
        foreach (var (_, document) in _entries)
        {
            Dispose(document);
        }

        _entries.Clear();
    }

    /// <summary>The key these files are kept under.</summary>
    private static Key MakeKey(SightFile primary, SightFile? second, string choice) =>
        new(
            primary.Path,
            primary.Length,
            primary.WriteTicks,
            choice,
            second?.Path,
            second?.Length ?? 0,
            second?.WriteTicks ?? 0
        );

    /// <summary>The directory a path sits in.</summary>
    private static string Directory(string path)
    {
        var at = path.LastIndexOfAny(['/', '\\']);

        return at < 0 ? path : path[..at];
    }

    /// <summary>Lets go of a picture an answer holds.</summary>
    private static void Dispose(SightDocument document)
    {
        switch (document)
        {
            case SightImageDocument image:
                image.Picture.Dispose();

                break;
            case SightPsdDocument { Picture: { } picture }:
                picture.Dispose();

                break;
        }
    }

    /// <summary>What one answer is kept under.</summary>
    private readonly record struct Key(
        string Primary,
        long PrimaryLength,
        long PrimaryTicks,
        string Choice,
        string? Second,
        long SecondLength,
        long SecondTicks
    );
}
