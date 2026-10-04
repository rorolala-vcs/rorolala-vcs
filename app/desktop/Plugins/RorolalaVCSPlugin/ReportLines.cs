using System.Text;
using Avalonia.Controls;
using Avalonia.Controls.Documents;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;

namespace RorolalaVCSPlugin;

/// <summary>Which of the three a reported line is, or that it is not one of them.</summary>
internal enum ReportKind
{
    /// <summary>What the command wrote as a result, which is drawn as it stands.</summary>
    Plain,

    /// <summary>Something that has gone wrong.</summary>
    Error,

    /// <summary>Something to be careful of.</summary>
    Warning,

    /// <summary>Something that can be done about it.</summary>
    Help,
}

/// <summary>One thing said, which is one line or several lines of one kind.</summary>
/// <remarks>
/// Lines of one kind together rather than one line each, because a message is often several lines and a report
/// that drew a mark on every one of them would read as several things said rather than one.
/// </remarks>
/// <param name="Kind">Which of the three it is.</param>
/// <param name="Lines">What it says, line by line.</param>
internal sealed record ReportBlock(ReportKind Kind, IReadOnlyList<string> Lines);

/// <summary>
/// A command's own report, read the way the command wrote it.
/// </summary>
/// <remarks>
/// The command is asked for the form a program reads — see <c>--theme-choice</c> in the command line — and what
/// comes back is read here: the level of a reported line is a word at the head of it, and what it says follows
/// in the same colour language and markdown the terminal is drawn from. Nothing is rewritten on the way: what
/// the command said is what the window says.
/// </remarks>
internal static class ReportLines
{
    /// <summary>The word a line that has gone wrong is headed with.</summary>
    private const string ErrorWord = "err: ";

    /// <summary>The word a line to be careful of is headed with.</summary>
    private const string WarningWord = "warn: ";

    /// <summary>The word a line about what can be done is headed with.</summary>
    private const string HelpWord = "help: ";

    /// <summary>
    /// Reads a command's output into what it said.
    /// </summary>
    /// <remarks>
    /// Lines of one kind that follow each other are one thing said: the command marks every line of a message
    /// with the level, since a line is all it knows at the time, and it is here that they are known to belong
    /// together. A line of another kind between two of a kind is what parts them, blank lines included — a blank
    /// line is not nothing, it is the space somebody put between two paragraphs.
    /// </remarks>
    /// <param name="said">What the command wrote, both streams together.</param>
    /// <returns>What it said, in the order it said it.</returns>
    public static IReadOnlyList<ReportBlock> Parse(string said)
    {
        ArgumentNullException.ThrowIfNull(said);

        var blocks = new List<ReportBlock>();
        var kind = ReportKind.Plain;
        var lines = new List<string>();

        void Settle()
        {
            if (lines.Count > 0)
            {
                blocks.Add(new ReportBlock(kind, [.. lines]));
                lines.Clear();
            }
        }

        foreach (var line in Lines(said))
        {
            var (found, rest) = Kind(line);

            if (found != kind)
            {
                Settle();
                kind = found;
            }

            lines.Add(rest);
        }

        Settle();

        return blocks;
    }

    /// <summary>
    /// Draws one thing said, in the language the command wrote it in.
    /// </summary>
    /// <remarks>
    /// A part of what the terminal renderer does, for the marks a command's report is written with: a colour or a
    /// link, bold or italic, a code span or a fenced block, and a backslash that takes the meaning away from the
    /// character after it. What a report is not read for — a heading, a quotation, a rule, a table, a named alert
    /// — is drawn as it was written, which is what a reader sees rather than nothing.
    /// </remarks>
    /// <param name="into">Where the words go.</param>
    /// <param name="text">What was written.</param>
    /// <param name="ink">The colour or theme resource a named colour in it is drawn in, or nothing to leave it as it stands.</param>
    public static void Draw(TextBlock into, string text, Func<string, string?> ink)
    {
        ArgumentNullException.ThrowIfNull(into);
        ArgumentNullException.ThrowIfNull(text);
        ArgumentNullException.ThrowIfNull(ink);

        var inlines = into.Inlines ??= new InlineCollection();
        var fenced = false;

        foreach (var line in Lines(text))
        {
            if (inlines.Count > 0)
            {
                inlines.Add(new LineBreak());
            }

            var fence = Fence(line);

            if (fence || fenced)
            {
                fenced = !fenced;
                inlines.Add(new Run(line) { FontFamily = Mono });

                continue;
            }

            Words(into, line, null, ink);
        }
    }

    /// <summary>What the three marks are written as, so that a line can be read for its level.</summary>
    /// <param name="line">The line as the command wrote it.</param>
    /// <returns>The level it is, and what it says without the mark.</returns>
    private static (ReportKind Kind, string Text) Kind(string line)
    {
        // Matched after the space a line may have been written with, and never trimmed off it: what a message
        // is indented with is part of the message, and a report of code is a report that needs its indentation.
        var at = line.TrimStart(' ', '\t');
        var indent = line[..(line.Length - at.Length)];

        if (at.StartsWith(ErrorWord, StringComparison.Ordinal))
        {
            return (ReportKind.Error, indent + at[ErrorWord.Length..]);
        }

        if (at.StartsWith(WarningWord, StringComparison.Ordinal))
        {
            return (ReportKind.Warning, indent + at[WarningWord.Length..]);
        }

        if (at.StartsWith(HelpWord, StringComparison.Ordinal))
        {
            return (ReportKind.Help, indent + at[HelpWord.Length..]);
        }

        return (ReportKind.Plain, line);
    }

    /// <summary>Whether a line opens or closes a fenced block of code.</summary>
    /// <param name="line">The line to read.</param>
    /// <returns>Whether it is a fence.</returns>
    private static bool Fence(string line)
    {
        var at = line.Trim();

        if (at.Length < 3 || (at[0] != '`' && at[0] != '~'))
        {
            return false;
        }

        return at.All(found => found == at[0]);
    }

    /// <summary>
    /// Writes one line of prose into the block, reading the marks inside it.
    /// </summary>
    /// <remarks>
    /// The same four marks the terminal is drawn from, read left to right so that what is inside a pair is the
    /// marked part and what is not is text: a pair of stars for bold, one for italic, a pair of brackets for a
    /// colour or a link. A mark with nothing to pair with is a character like any other, which is what makes a
    /// lone star safe to write.
    /// </remarks>
    /// <param name="into">Where the words go.</param>
    /// <param name="text">The line to read.</param>
    /// <param name="strong">The marks already in force around it, or nothing.</param>
    /// <param name="ink">The colour or the theme resource a named colour is drawn in.</param>
    private static void Words(TextBlock into, string text, Marks? strong, Func<string, string?> ink)
    {
        var inlines = into.Inlines ??= new InlineCollection();
        var written = new StringBuilder();

        void Settle()
        {
            if (written.Length == 0)
            {
                return;
            }

            var run = new Run(written.ToString());

            if (strong is { } marks)
            {
                marks.Applied(run, ink);
            }

            inlines.Add(run);
            written.Clear();
        }

        for (var at = 0; at < text.Length; at++)
        {
            var found = text[at];

            if (found == '\\' && at + 1 < text.Length)
            {
                written.Append(text[++at]);

                continue;
            }

            // A code span and a flag keep their markers, as a terminal is drawn them: what they say is that the
            // characters between them are to be taken as they are written.
            if (found == '`' && Pair(text, at, '`') is { } code)
            {
                Settle();
                inlines.Add(new Run(text[at..(code + 1)]) { FontFamily = Mono });
                at = code;

                continue;
            }

            if (found == '[' && text[at..].StartsWith("[[", StringComparison.Ordinal))
            {
                if (Directive(text, at) is { } directive)
                {
                    var inside = strong ?? new Marks();

                    inside = directive.Colour is { } colour
                        ? inside with { Colour = colour }
                        : inside with { Link = directive.Link };

                    Settle();
                    Words(into, directive.Body, inside, ink);
                    at = directive.End;

                    continue;
                }
            }

            if (found is '*' or '_')
            {
                var length = found == '*' && text[at..].StartsWith("**", StringComparison.Ordinal) ? 2 : 1;

                if (Pair(text, at + length - 1, found) is { } closed && closed > at + length)
                {
                    Settle();
                    Words(into, text[(at + length)..closed], (strong ?? new Marks()).And(found, length), ink);
                    at = closed;

                    continue;
                }
            }

            written.Append(found);
        }

        Settle();
    }

    /// <summary>
    /// Reads a directive at a position: what it says, what it is drawn in, and where it closes.
    /// </summary>
    /// <param name="text">The line being read.</param>
    /// <param name="at">Where the directive opens.</param>
    /// <returns>What it is, or nothing when it is not a directive.</returns>
    private static (string Body, string? Colour, string? Link, int End)? Directive(string text, int at)
    {
        var closes = text.IndexOf("]]", at + 2, StringComparison.Ordinal);

        if (closes < 0)
        {
            return null;
        }

        var head = text[(at + 2)..closes];
        var body = text[(closes + 2)..];
        var end = body.IndexOf("[[/]]", StringComparison.Ordinal);

        if (end < 0)
        {
            return null;
        }

        // A link is written as the sequence a terminal follows; what a window draws of it is the words, which are
        // the part a reader wants and the address is not.
        return head.StartsWith('?')
            ? (body[..end], null, head[1..], closes + 7 + end)
            : (body[..end], head, null, closes + 7 + end);
    }

    /// <summary>Where the next unescaped instance of a character is, if there is one.</summary>
    /// <param name="text">The line being read.</param>
    /// <param name="from">Where to look from.</param>
    /// <param name="found">The character to look for.</param>
    /// <returns>Where it is, or nothing.</returns>
    private static int? Pair(string text, int from, char found)
    {
        for (var at = from + 1; at < text.Length; at++)
        {
            if (text[at] == '\\')
            {
                at++;

                continue;
            }

            if (text[at] == found)
            {
                return at;
            }
        }

        return null;
    }

    /// <summary>The lines of something written, however it was ended.</summary>
    /// <param name="text">What was written.</param>
    /// <returns>Its lines, with nothing left at the ends of them.</returns>
    private static IEnumerable<string> Lines(string text) =>
        text.Replace("\r\n", "\n", StringComparison.Ordinal)
            .Replace('\r', '\n')
            .Trim('\n')
            .Split('\n');

    /// <summary>What is drawn around a part of a line.</summary>
    private sealed record Marks
    {
        /// <summary>Whether the part is bold.</summary>
        public bool Bold { get; init; }

        /// <summary>Whether the part is italic.</summary>
        public bool Italic { get; init; }

        /// <summary>Whether the part is underlined.</summary>
        public bool Underlined { get; init; }

        /// <summary>The colour it was named in, or nothing.</summary>
        public string? Colour { get; init; }

        /// <summary>What it stands for, when it is a link.</summary>
        public string? Link { get; init; }

        /// <summary>The same marks with one more pair read.</summary>
        /// <param name="marker">The character the pair is made of.</param>
        /// <param name="length">How many of them the pair is.</param>
        /// <returns>The marks in force inside it.</returns>
        public Marks And(char marker, int length) =>
            this with
            {
                Bold = Bold || (marker == '*' && length == 2),
                Italic = Italic || (marker == '*' && length == 1),
                Underlined = Underlined || (marker == '_' && length == 1),
            };

        /// <summary>Draws a run in these marks.</summary>
        /// <param name="run">The run to draw.</param>
        /// <param name="ink">The colour or the theme resource a named colour is drawn in.</param>
        public void Applied(Run run, Func<string, string?> ink)
        {
            run.FontWeight = Bold ? FontWeight.SemiBold : FontWeight.Normal;
            run.FontStyle = Italic ? FontStyle.Italic : FontStyle.Normal;

            if (Link is not null || Underlined)
            {
                run.TextDecorations = TextDecorations.Underline;
            }

            if (Colour is not { Length: > 0 } colour || ink(colour) is not { } answer)
            {
                return;
            }

            // Six digits are the colour itself, and anything else is a name the look is asked for: read as a
            // resource rather than looked up here, so that a colour follows the theme the window is drawn in.
            if (answer.StartsWith('#'))
            {
                if (Color.TryParse(answer, out var written))
                {
                    run.Foreground = new SolidColorBrush(written);
                }

                return;
            }

            run[!TextElement.ForegroundProperty] = new DynamicResourceExtension(answer);
        }
    }

    /// <summary>What a code span or a fenced block is drawn in.</summary>
    private static readonly FontFamily Mono = new("Consolas, DejaVu Sans Mono, monospace");
}
