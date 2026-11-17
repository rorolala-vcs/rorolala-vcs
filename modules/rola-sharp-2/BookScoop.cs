using System.Runtime.InteropServices;

namespace RolaSharp.V2;

/// <summary>A run of text a hit matched, and where in the field it is.</summary>
/// <param name="Start">Where it starts, in characters.</param>
/// <param name="Length">How many characters it covers.</param>
public sealed record BookRange(ulong Start, ulong Length);

/// <summary>One place a hit matched, with the text around it.</summary>
/// <param name="Field">What field it is: <c>title</c>, <c>description</c> or <c>body</c>.</param>
/// <param name="Text">The text around the match.</param>
/// <param name="Ranges">Where the match is inside <paramref name="Text"/>.</param>
public sealed record BookSnippet(string Field, string Text, IReadOnlyList<BookRange> Ranges);

/// <summary>One article a search found.</summary>
/// <param name="Name">The article's name.</param>
/// <param name="Title">Its title.</param>
/// <param name="Description">What it says about itself.</param>
/// <param name="Score">How well it matched.</param>
/// <param name="Snippets">The places it matched.</param>
public sealed record BookHit(
    string Name,
    string Title,
    string Description,
    double Score,
    IReadOnlyList<BookSnippet> Snippets
);

/// <summary>The book the program ships, and what a query finds in it.</summary>
public sealed class BookScoop : Scoop, IDisposable
{
    private readonly string _start;
    private nint _handle;

    /// <summary>Makes a scope that looks for the book at or above a directory.</summary>
    /// <param name="start">Where the search starts.</param>
    public BookScoop(string start) => _start = Path.GetFullPath(start);

    /// <summary>How many articles the book holds.</summary>
    public int ArticleCount { get; private set; }

    /// <summary>Searches the book.</summary>
    /// <param name="query">The words to look for.</param>
    /// <param name="limit">The most hits to answer, or zero for every one.</param>
    /// <returns>The hits, best first.</returns>
    public IReadOnlyList<BookHit> Search(string query, int limit = 20)
    {
        ArgumentNullException.ThrowIfNull(query);
        if (_handle == 0)
        {
            throw new InvalidOperationException("call Read first");
        }

        var text = Utf8(query);
        try
        {
            var results = RorolalaBinding.search_rola_book(_handle, text, (nuint)limit);
            if (results == 0)
            {
                return [];
            }

            try
            {
                var count = (int)RorolalaBinding.rola_book_results_count(results);
                var hits = new List<BookHit>(count);
                for (var index = 0; index < count; index++)
                {
                    var hit = RorolalaBinding.rola_book_results_hit(results, (nuint)index);
                    if (hit == 0)
                    {
                        continue;
                    }

                    try
                    {
                        hits.Add(
                            new BookHit(
                                NativeText.Taken(RorolalaBinding.rola_book_hit_name(hit)),
                                NativeText.Taken(RorolalaBinding.rola_book_hit_title(hit)),
                                NativeText.Taken(RorolalaBinding.rola_book_hit_description(hit)),
                                RorolalaBinding.rola_book_hit_score(hit),
                                Snippets(hit)
                            )
                        );
                    }
                    finally
                    {
                        RorolalaBinding.free_rola_book_hit(hit);
                    }
                }

                return hits;
            }
            finally
            {
                RorolalaBinding.free_rola_book_results(results);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(text);
        }
    }

    /// <inheritdoc/>
    protected override void IsValid()
    {
        if (!Directory.Exists(_start) && !File.Exists(_start))
        {
            throw new RolaReadError(
                RolaReadErrorKind.InvalidArgument,
                _start,
                "the book's starting point is not there"
            );
        }
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        Dispose();
        cancellationToken.ThrowIfCancellationRequested();

        var start = Utf8(_start);
        try
        {
            var handle = RorolalaBinding.locate_rola_book_search(start);
            if (handle == 0)
            {
                throw new RolaReadError(
                    RolaReadErrorKind.Unavailable,
                    _start,
                    "no book is there"
                );
            }

            _handle = handle;
        }
        finally
        {
            Marshal.FreeCoTaskMem(start);
        }

        ArticleCount = (int)RorolalaBinding.count_rola_book_articles(_handle);
    }

    /// <summary>Gives the native book back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_book_search(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }

    private static IReadOnlyList<BookSnippet> Snippets(nint hit)
    {
        var count = (int)RorolalaBinding.rola_book_hit_snippet_count(hit);
        var snippets = new List<BookSnippet>(count);
        for (var index = 0; index < count; index++)
        {
            var snippet = RorolalaBinding.rola_book_hit_snippet(hit, (nuint)index);
            if (snippet == 0)
            {
                continue;
            }

            try
            {
                var ranges = new List<BookRange>();
                var rangeCount = (int)
                    RorolalaBinding.rola_book_snippet_range_count(snippet);
                for (var at = 0; at < rangeCount; at++)
                {
                    ranges.Add(
                        new BookRange(
                            RorolalaBinding.rola_book_snippet_range_start(
                                snippet,
                                (nuint)at
                            ),
                            RorolalaBinding.rola_book_snippet_range_length(
                                snippet,
                                (nuint)at
                            )
                        )
                    );
                }

                snippets.Add(
                    new BookSnippet(
                        NativeText.Taken(
                            RorolalaBinding.rola_book_snippet_field(snippet)
                        ),
                        NativeText.Taken(RorolalaBinding.rola_book_snippet_text(snippet)),
                        ranges
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_book_snippet(snippet);
            }
        }

        return snippets;
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}
