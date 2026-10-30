using System.Runtime.InteropServices;
using System.Text;

namespace RolaSharp;

/// <summary>One matched range of a fragment, counted in characters.</summary>
/// <param name="Start">The first character of the range.</param>
/// <param name="Length">How many characters it covers.</param>
public readonly record struct BookSpan(int Start, int Length);

/// <summary>One piece of a fragment: text drawn the same way as the pieces beside it, or not.</summary>
/// <param name="Text">The piece itself.</param>
/// <param name="Matched">Whether the piece is one of the words the query matched.</param>
public sealed record BookRun(string Text, bool Matched);

/// <summary>
/// A fragment of an article, and the words in it the query matched.
/// </summary>
/// <remarks>
/// The fragment is cut from the field the query matched — the body when the word is in the body,
/// and the description or the title when the word is only there — with `…` where it was cut. The
/// field it came from is named so that a caller can say where a hit was found.
/// <para>
/// <see cref="Runs"/> is the fragment split at the matched words, which is what a renderer draws:
/// each run is either ordinary text or a match, and nothing has to be done to the string to find
/// out which. <see cref="Spans"/> is the same information as character positions, for a caller
/// that has its own way of drawing.
/// </para>
/// </remarks>
/// <param name="Field">The field the fragment was cut from: <c>content</c>, <c>description</c>,
/// <c>keywords</c>, <c>title</c> or <c>name</c>.</param>
/// <param name="Text">The fragment's text, with <c>…</c> where it was cut.</param>
/// <param name="Spans">The matched words inside <paramref name="Text"/>, in characters.</param>
public sealed record BookSnippet(string Field, string Text, IReadOnlyList<BookSpan> Spans)
{
    /// <summary>The fragment split at the matched words.</summary>
    /// <remarks>
    /// The split is on characters rather than on UTF-16 code units, so a fragment that holds an
    /// emoji or another character outside the Basic Multilingual Plane is still cut in the right
    /// places. A caller that draws <see cref="Text"/> as a whole and uses <see cref="Spans"/> needs
    /// the same conversion; one that draws the runs does not.
    /// </remarks>
    public IReadOnlyList<BookRun> Runs { get; } = RunsOf(Text, Spans);

    /// <summary>Splits a fragment into its ordinary and its matched pieces.</summary>
    /// <param name="text">The fragment's text.</param>
    /// <param name="spans">The matched ranges, in characters of the text.</param>
    /// <returns>The runs, in the order they appear.</returns>
    private static List<BookRun> RunsOf(string text, IReadOnlyList<BookSpan> spans)
    {
        var characters = text.EnumerateRunes().ToArray();
        var runs = new List<BookRun>();
        var cursor = 0;

        foreach (var span in spans)
        {
            // A range is clamped because a caller should not be answered with a string that is
            // shorter than the ranges said it was: what is outside is not drawn as a match.
            var start = Math.Clamp(span.Start, cursor, characters.Length);
            var end = Math.Clamp(span.Start + span.Length, start, characters.Length);

            if (start > cursor)
            {
                runs.Add(new BookRun(Slice(text, characters, cursor, start), Matched: false));
            }

            if (end > start)
            {
                runs.Add(new BookRun(Slice(text, characters, start, end), Matched: true));
            }

            cursor = end;
        }

        if (cursor < characters.Length)
        {
            runs.Add(new BookRun(Slice(text, characters, cursor, characters.Length), Matched: false));
        }

        return runs;
    }

    /// <summary>The UTF-16 offset of the character at <paramref name="at"/>.</summary>
    /// <param name="characters">The text, as characters.</param>
    /// <param name="at">The character position.</param>
    /// <returns>The position in UTF-16 code units.</returns>
    private static int OffsetOf(Rune[] characters, int at)
    {
        var offset = 0;

        for (var index = 0; index < at && index < characters.Length; index++)
        {
            offset += characters[index].Utf16SequenceLength;
        }

        return offset;
    }

    /// <summary>The substring between two character positions.</summary>
    /// <param name="text">The text.</param>
    /// <param name="characters">The text, as characters.</param>
    /// <param name="start">The first character position.</param>
    /// <param name="end">The position past the last character.</param>
    /// <returns>The substring.</returns>
    private static string Slice(string text, Rune[] characters, int start, int end)
    {
        var from = OffsetOf(characters, start);
        var to = OffsetOf(characters, end);

        return text.Substring(from, to - from);
    }
}

/// <summary>
/// One article a search found, as a list of hits draws it.
/// </summary>
/// <param name="Name">The article's name, which finds it again through the book.</param>
/// <param name="Title">The article's title.</param>
/// <param name="Description">The article's description.</param>
/// <param name="Score">How well the query matched, relative to the other hits of the same search.</param>
/// <param name="Snippets">The fragments the query matched, best first.</param>
public sealed record BookHit(
    string Name,
    string Title,
    string Description,
    double Score,
    IReadOnlyList<BookSnippet> Snippets
);

/// <summary>
/// The built-in book: the articles and the index over them, read from one file.
/// </summary>
/// <remarks>
/// This is the hand-written wrapper over the generated bindings, as <see cref="RolaIgnoreRules"/> is:
/// what crosses is an opaque handle, and what this adds is the shape a caller wants — a book opened
/// from a path or located beside the program, hits as values rather than as pointers, and the native
/// handle given back on <see cref="Dispose"/>.
/// <para>
/// A book is read whole when it is opened, so a search does not touch the disk again: a program opens
/// it once, keeps it, and asks as many times as a search box needs. The file is checked against the
/// digest it carries, so a book that was cut short or written by a format this build does not know
/// opens as nothing rather than being searched.
/// </para>
/// </remarks>
public sealed class RolaBookSearch : IDisposable
{
    /// <summary>The native book, owned by this instance while it lives.</summary>
    private nint _handle;

    /// <summary>Makes a wrapper over a native book, which it takes ownership of.</summary>
    /// <param name="handle">The native book, which must not be null.</param>
    private RolaBookSearch(nint handle) => _handle = handle;

    /// <summary>How many articles the book holds.</summary>
    public int Count
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return (int)RorolalaBinding.count_rola_book_articles(_handle);
        }
    }

    /// <summary>Opens the book at <paramref name="path"/>.</summary>
    /// <param name="path">The book file, as the native side reads it.</param>
    /// <returns>The book, or nothing when the file is not there or is not a book this build reads.</returns>
    public static RolaBookSearch? Open(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        var at = Marshal.StringToCoTaskMemUTF8(path);

        try
        {
            var opened = RorolalaBinding.open_rola_book_search(at);

            if (opened.tag == RorolalaResultTag.RorolalaResult_Ok)
            {
                return opened.payload == 0 ? null : new RolaBookSearch(opened.payload);
            }

            if (opened.payload != 0)
            {
                RorolalaBinding.free_rola_book_error(opened.payload);
            }

            return null;
        }
        finally
        {
            Marshal.FreeCoTaskMem(at);
        }
    }

    /// <summary>
    /// Opens the book laid beside the programs under <paramref name="start"/>, if there is one.
    /// </summary>
    /// <remarks>
    /// The walk goes up from the directory a program is in and looks for <c>docs/builtin.book</c>
    /// under the root that holds both the programs and the documentation, so the same call is right
    /// for the Desktop program and for the command line program two directories under the root.
    /// </remarks>
    /// <param name="start">The directory to walk up from, which is the program's own.</param>
    /// <returns>The book, or nothing when there is none beside the programs.</returns>
    public static RolaBookSearch? Locate(string start)
    {
        ArgumentNullException.ThrowIfNull(start);

        var at = Marshal.StringToCoTaskMemUTF8(start);

        try
        {
            var handle = RorolalaBinding.locate_rola_book_search(at);

            return handle == 0 ? null : new RolaBookSearch(handle);
        }
        finally
        {
            Marshal.FreeCoTaskMem(at);
        }
    }

    /// <summary>The articles <paramref name="query"/> finds, best first, at most <paramref name="limit"/>.</summary>
    /// <remarks>
    /// A query is a word or a phrase in any language the book is written in. Every term the query
    /// names has to be in an article for it to be a hit — Chinese is cut into characters and pairs,
    /// so a phrase asks for the phrase — and the hits carry the description and the fragments the
    /// query matched in them.
    /// </remarks>
    /// <param name="query">The words to find.</param>
    /// <param name="limit">How many hits to answer with at most.</param>
    /// <returns>The hits, best first; an empty list when nothing matched.</returns>
    public IReadOnlyList<BookHit> Search(string query, int limit)
    {
        ObjectDisposedException.ThrowIf(_handle == 0, this);
        ArgumentNullException.ThrowIfNull(query);
        ArgumentOutOfRangeException.ThrowIfNegative(limit);

        var text = Marshal.StringToCoTaskMemUTF8(query);
        nint results = 0;

        try
        {
            results = RorolalaBinding.search_rola_book(_handle, text, (nuint)limit);

            if (results == 0)
            {
                return Array.Empty<BookHit>();
            }

            var count = (int)RorolalaBinding.rola_book_results_count(results);
            var hits = new List<BookHit>(count);

            for (var at = 0; at < count; at++)
            {
                var handle = RorolalaBinding.rola_book_results_hit(results, (nuint)at);

                if (handle == 0)
                {
                    continue;
                }

                try
                {
                    hits.Add(HitOf(handle));
                }
                finally
                {
                    RorolalaBinding.free_rola_book_hit(handle);
                }
            }

            return hits;
        }
        finally
        {
            if (results != 0)
            {
                RorolalaBinding.free_rola_book_results(results);
            }

            Marshal.FreeCoTaskMem(text);
        }
    }

    /// <summary>Gives the native book back.</summary>
    public void Dispose()
    {
        // Nothing to give back the second time, and nothing was lost the first: the handle is
        // forgotten as soon as it is released, so a double `Dispose` releases nothing twice.
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_book_search(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }

    /// <summary>Reads a native hit out, fragments and all.</summary>
    /// <param name="handle">The native hit, which the caller releases.</param>
    /// <returns>The hit as a value.</returns>
    private static BookHit HitOf(nint handle)
    {
        var count = (int)RorolalaBinding.rola_book_hit_snippet_count(handle);
        var snippets = new List<BookSnippet>(count);

        for (var at = 0; at < count; at++)
        {
            var snippet = RorolalaBinding.rola_book_hit_snippet(handle, (nuint)at);

            if (snippet == 0)
            {
                continue;
            }

            try
            {
                snippets.Add(SnippetOf(snippet));
            }
            finally
            {
                RorolalaBinding.free_rola_book_snippet(snippet);
            }
        }

        return new BookHit(
            NativeText.Taken(RorolalaBinding.rola_book_hit_name(handle)),
            NativeText.Taken(RorolalaBinding.rola_book_hit_title(handle)),
            NativeText.Taken(RorolalaBinding.rola_book_hit_description(handle)),
            RorolalaBinding.rola_book_hit_score(handle),
            snippets
        );
    }

    /// <summary>Reads a native fragment out, ranges and all.</summary>
    /// <param name="handle">The native fragment, which the caller releases.</param>
    /// <returns>The fragment as a value.</returns>
    private static BookSnippet SnippetOf(nint handle)
    {
        var count = (int)RorolalaBinding.rola_book_snippet_range_count(handle);
        var spans = new List<BookSpan>(count);

        for (var at = 0; at < count; at++)
        {
            spans.Add(
                new BookSpan(
                    (int)RorolalaBinding.rola_book_snippet_range_start(handle, (nuint)at),
                    (int)RorolalaBinding.rola_book_snippet_range_length(handle, (nuint)at)
                )
            );
        }

        return new BookSnippet(
            NativeText.Taken(RorolalaBinding.rola_book_snippet_field(handle)),
            NativeText.Taken(RorolalaBinding.rola_book_snippet_text(handle)),
            spans
        );
    }
}
