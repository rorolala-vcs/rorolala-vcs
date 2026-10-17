using System.Globalization;
using System.Text;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using Avalonia.Threading;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace TerminalPlugin;

/// <summary>
/// One terminal: the screen a shell draws on, and the keys and scrolling it is driven by.
/// </summary>
/// <remarks>
/// What is drawn is the cell grid <see cref="TerminalScreen"/> keeps, grouped into runs of one style so that
/// a line of text is one drawing rather than one per character. The bytes come from a pseudo-terminal — see
/// <see cref="RolaDesktopPty"/> — and are polled on the window's own timer: reading is a question with an
/// answer that is already there, so there is no thread and no lock between the two halves.
/// <para>
/// The font is monospaced and every cell is one advance wide, which is what makes a run drawable at its
/// column's own position; a wide character is two cells and the font's own advance for it is expected to
/// be two as well.
/// </para>
/// </remarks>
internal sealed class TerminalView : Control
{
    /// <summary>How often the terminal is asked whether it has printed anything.</summary>
    private static readonly TimeSpan Every = TimeSpan.FromMilliseconds(16);

    /// <summary>How many lines one wheel notch scrolls back.</summary>
    private const int WheelStep = 3;

    /// <summary>What text is drawn in, and what stands behind it.</summary>
    private static readonly Color Ground = Color.FromRgb(0x1E, 0x1E, 0x1E);
    private static readonly Color Ink = Color.FromRgb(0xD4, 0xD4, 0xD4);
    private static readonly Color CursorInk = Color.FromRgb(0xE5, 0xE5, 0xE5);

    /// <summary>The screen the shell draws on.</summary>
    private readonly TerminalScreen _screen = new(80, 24);

    /// <summary>The terminal the shell is in, or nothing when it could not be started.</summary>
    private RolaDesktopPty? _pty;

    /// <summary>Where the shell was started, said when it could not be.</summary>
    private readonly string _why;

    /// <summary>What the timer reads into, so that polling allocates nothing.</summary>
    private readonly byte[] _buffer = new byte[16 * 1024];

    /// <summary>The font family and size the screen is drawn in.</summary>
    private readonly FontFamily _family;
    private readonly double _size;

    /// <summary>The four faces a cell may be drawn in, which is bold and italic in every combination.</summary>
    private readonly Typeface _regular;
    private readonly Typeface _bold;
    private readonly Typeface _italic;
    private readonly Typeface _boldItalic;

    /// <summary>How large one cell is, measured once from the font.</summary>
    private readonly double _cellWidth;
    private readonly double _cellHeight;

    /// <summary>The brushes colours are drawn with, kept so that a colour is made once.</summary>
    private readonly Dictionary<uint, IBrush> _brushes = [];

    /// <summary>The brush the ground and the ordinary ink are drawn with.</summary>
    private readonly IBrush _groundBrush = new SolidColorBrush(Ground);
    private readonly IBrush _inkBrush = new SolidColorBrush(Ink);
    private readonly IBrush _cursorBrush = new SolidColorBrush(CursorInk);

    /// <summary>The timer that polls the terminal while the view is on screen.</summary>
    private readonly DispatcherTimer _timer = new() { Interval = Every };

    /// <summary>How far back the view is scrolled, which is zero at the screen the shell is drawing on.</summary>
    private int _scrolled;

    /// <summary>Whether the shell has ended, which stops the polling and the cursor.</summary>
    private bool _ended;

    /// <summary>Makes one terminal.</summary>
    /// <param name="log">Where a shell that could not be started is said.</param>
    /// <param name="program">The program to run.</param>
    /// <param name="arguments">Its arguments.</param>
    /// <param name="directory">Where it is run.</param>
    /// <param name="font">The font family it is drawn in.</param>
    /// <param name="fontSize">The size it is drawn at.</param>
    public TerminalView(
        ILog log,
        string program,
        IReadOnlyList<string> arguments,
        string directory,
        string font,
        double fontSize
    )
    {
        _family = new FontFamily(font);
        _size = fontSize;
        _regular = new Typeface(_family);
        _bold = new Typeface(_family, FontStyle.Normal, FontWeight.Bold);
        _italic = new Typeface(_family, FontStyle.Italic);
        _boldItalic = new Typeface(_family, FontStyle.Italic, FontWeight.Bold);

        var probe = new FormattedText(
            "W",
            CultureInfo.InvariantCulture,
            FlowDirection.LeftToRight,
            _regular,
            _size,
            _inkBrush
        );

        _cellWidth = Math.Max(1, probe.WidthIncludingTrailingWhitespace);
        _cellHeight = Math.Max(1, probe.Height);

        Focusable = true;
        ClipToBounds = true;
        Cursor = new Cursor(StandardCursorType.Ibeam);

        _pty = Started(log, program, arguments, directory);
        _why = RolaI18N.Get("rorolala_terminal.cannot_start", program);

        _timer.Tick += (_, _) => Pump();
    }

    /// <summary>Starts the shell, saying in the log why it could not be when it could not.</summary>
    /// <param name="log">Where a shell that could not be started is said.</param>
    /// <param name="program">The program to run.</param>
    /// <param name="arguments">Its arguments.</param>
    /// <param name="directory">Where it is run.</param>
    /// <returns>The terminal, or nothing.</returns>
    private static RolaDesktopPty? Started(
        ILog log,
        string program,
        IReadOnlyList<string> arguments,
        string directory
    )
    {
        try
        {
            return RolaDesktopPty.Open(program, arguments, directory, 80, 24);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            // A machine without the native library beside the program is one no terminal can be opened on,
            // and the line in the middle of the dock is what a reader sees rather than a dock that failed to
            // open: what went wrong is said here, where it is looked for.
            log.Error($"the terminal `{program}` could not be started: {error.Message}");

            return null;
        }
    }

    /// <summary>Starts polling when the view is on screen, which is when there is something to draw.</summary>
    /// <param name="attached">What it was attached to.</param>
    protected override void OnAttachedToVisualTree(VisualTreeAttachmentEventArgs attached)
    {
        base.OnAttachedToVisualTree(attached);

        // A program that asks where the cursor is gets the answer written back, which is how a shell works
        // out where its prompt ends.
        _screen.Respond += Reply;
        _timer.Start();
        Focus();
    }

    /// <summary>Stops polling when the view is taken off screen, keeping the shell for when it comes back.</summary>
    /// <param name="detached">What it was detached from.</param>
    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs detached)
    {
        _timer.Stop();
        _screen.Respond -= Reply;

        base.OnDetachedFromVisualTree(detached);
    }

    /// <summary>What the terminal wants to be: a terminal's own eighty by twenty-four.</summary>
    /// <param name="availableSize">What there is.</param>
    /// <returns>The size asked for.</returns>
    protected override Size MeasureOverride(Size availableSize) =>
        new(80 * _cellWidth, 24 * _cellHeight);

    /// <summary>Says how large the view is, which is what the shell lays its screen out by.</summary>
    /// <param name="finalSize">The size it was given.</param>
    /// <returns>What it takes, which is all of it.</returns>
    protected override Size ArrangeOverride(Size finalSize)
    {
        var columns = Math.Max(1, (int)(finalSize.Width / _cellWidth));
        var rows = Math.Max(1, (int)(finalSize.Height / _cellHeight));

        if (columns != _screen.Columns || rows != _screen.Rows)
        {
            _screen.Resize(columns, rows);
            _pty?.Resize(columns, rows);
            InvalidateVisual();
        }

        return finalSize;
    }

    /// <summary>Draws the cells, the ground behind them, and the cursor.</summary>
    /// <param name="context">What to draw into.</param>
    public override void Render(DrawingContext context)
    {
        context.FillRectangle(_groundBrush, new Rect(Bounds.Size));

        if (_pty is null)
        {
            context.DrawText(
                new FormattedText(
                    _why,
                    CultureInfo.InvariantCulture,
                    FlowDirection.LeftToRight,
                    _regular,
                    _size,
                    _inkBrush
                ),
                new Point(4, 4)
            );

            return;
        }

        var rows = Math.Min(_screen.Rows, (int)(Bounds.Height / _cellHeight) + 1);

        for (var row = 0; row < rows; row++)
        {
            var line = _screen.Line(row, _scrolled);
            var ground = row * _cellHeight;

            Grounds(context, line, ground);

            for (var at = 0; at < line.Length;)
            {
                var cell = line[at];

                // The right half of a wide character is drawn by the character it belongs to.
                if (cell.Continuation)
                {
                    at++;

                    continue;
                }

                var start = at;
                var face = Face(cell.Marks);
                var (fore, _) = Colours(cell);

                var run = new StringBuilder();
                run.Append(Character(cell));

                at++;

                while (at < line.Length)
                {
                    var next = line[at];

                    if (next.Continuation || !Same(next, cell))
                    {
                        break;
                    }

                    run.Append(Character(next));
                    at++;
                }

                if (at == start + 1 && cell.Blank && ReferenceEquals(Colours(cell).Back, _groundBrush))
                {
                    continue;
                }

                context.DrawText(
                    new FormattedText(
                        run.ToString(),
                        CultureInfo.InvariantCulture,
                        FlowDirection.LeftToRight,
                        face,
                        _size,
                        fore
                    ),
                    new Point(start * _cellWidth, ground)
                );

                if (cell.Marks.HasFlag(CellMarks.Underline))
                {
                    context.FillRectangle(
                        fore,
                        new Rect(
                            start * _cellWidth,
                            ground + _cellHeight - 1,
                            (at - start) * _cellWidth,
                            1
                        )
                    );
                }
            }
        }

        if (_screen.CursorVisible && _scrolled == 0 && !_ended)
        {
            context.DrawRectangle(
                null,
                new Pen(_cursorBrush, 1.5),
                new Rect(
                    _screen.CursorX * _cellWidth,
                    _screen.CursorY * _cellHeight,
                    _cellWidth,
                    _cellHeight
                )
            );
        }
    }

    /// <summary>Draws the ground behind the cells that have one of their own.</summary>
    /// <param name="context">What to draw into.</param>
    /// <param name="line">The row.</param>
    /// <param name="top">Where the row starts.</param>
    private void Grounds(DrawingContext context, ReadOnlySpan<TerminalCell> line, double top)
    {
        for (var at = 0; at < line.Length;)
        {
            var cell = line[at];

            if (cell.Continuation)
            {
                at++;

                continue;
            }

            var (_, back) = Colours(cell);

            if (ReferenceEquals(back, _groundBrush))
            {
                at++;

                continue;
            }

            var start = at;

            while (at < line.Length && !line[at].Continuation && SameGround(line[at], cell))
            {
                at++;
            }

            context.FillRectangle(
                back,
                new Rect(start * _cellWidth, top, (at - start) * _cellWidth, _cellHeight)
            );
        }
    }

    /// <summary>Whether two cells are drawn in the same ground.</summary>
    /// <param name="cell">One cell.</param>
    /// <param name="of">The other.</param>
    /// <returns>Whether the ground is the same.</returns>
    private bool SameGround(TerminalCell cell, in TerminalCell of) =>
        ReferenceEquals(Colours(cell).Back, Colours(of).Back);

    /// <summary>Whether two cells are drawn the same, which is what makes one run of them.</summary>
    /// <param name="cell">One cell.</param>
    /// <param name="of">The other.</param>
    /// <returns>Whether they are one run.</returns>
    private bool Same(TerminalCell cell, in TerminalCell of)
    {
        var (fore, back) = Colours(cell);
        var (otherFore, otherBack) = Colours(of);

        return cell.Marks == of.Marks
            && ReferenceEquals(fore, otherFore)
            && ReferenceEquals(back, otherBack);
    }

    /// <summary>What a cell's character is, as text.</summary>
    /// <param name="cell">The cell.</param>
    /// <returns>The character, or a space for a blank one.</returns>
    private static string Character(in TerminalCell cell) =>
        cell.Blank ? " " : char.ConvertFromUtf32(cell.Codepoint);

    /// <summary>
    /// The ink and the ground a cell is drawn in.
    /// </summary>
    /// <remarks>
    /// Both are resolved here rather than at the cell, because a cell holds what the program asked for and
    /// what is drawn is that plus the terminal's own default — and a reversed cell asks for the two
    /// exchanged, which is a pair and not a colour.
    /// </remarks>
    /// <param name="cell">The cell.</param>
    /// <returns>The brush of the text and the brush behind it.</returns>
    private (IBrush Fore, IBrush Back) Colours(in TerminalCell cell)
    {
        var fore = cell.HasForeground ? Brush(cell.Foreground) : _inkBrush;
        var back = cell.HasBackground ? Brush(cell.Background) : _groundBrush;

        return cell.Marks.HasFlag(CellMarks.Reverse) ? (back, fore) : (fore, back);
    }

    /// <summary>A brush for a colour, kept so that one colour is made once.</summary>
    /// <param name="colour">The colour, as the terminal names it.</param>
    /// <returns>The brush.</returns>
    private IBrush Brush(uint colour)
    {
        if (!_brushes.TryGetValue(colour, out var brush))
        {
            brush = new SolidColorBrush(Color.FromRgb(
                (byte)((colour >> 16) & 0xFF),
                (byte)((colour >> 8) & 0xFF),
                (byte)(colour & 0xFF)
            ));
            _brushes[colour] = brush;
        }

        return brush;
    }

    /// <summary>The face a cell is drawn in, from the marks it wears.</summary>
    /// <param name="marks">The marks.</param>
    /// <returns>The face.</returns>
    private Typeface Face(CellMarks marks) =>
        (marks.HasFlag(CellMarks.Bold), marks.HasFlag(CellMarks.Italic)) switch
        {
            (true, true) => _boldItalic,
            (true, false) => _bold,
            (false, true) => _italic,
            _ => _regular,
        };

    /// <summary>Reads what the shell has printed and draws it.</summary>
    private void Pump()
    {
        if (_pty is null || _ended)
        {
            return;
        }

        var changed = false;
        var before = _screen.Scrolled;

        while (true)
        {
            var read = _pty.Read(_buffer);

            if (read < 0)
            {
                _ended = true;
                _timer.Stop();

                break;
            }

            if (read == 0)
            {
                break;
            }

            changed |= _screen.Feed(_buffer.AsSpan(0, read));
        }

        // A reader looking at what has already scrolled past keeps looking at it while the shell goes on:
        // the window into the scrollback is moved by every line that is added, which is what the count it is
        // measured from does.
        if (_scrolled > 0)
        {
            _scrolled += _screen.Scrolled - before;
        }

        if (changed)
        {
            InvalidateVisual();
        }
    }

    /// <summary>Writes an answer a program asked for back to it.</summary>
    /// <param name="text">The answer.</param>
    private void Reply(string text) => _pty?.Write(Encoding.UTF8.GetBytes(text));

    /// <summary>Sends what a user typed to the shell.</summary>
    /// <param name="typed">The input.</param>
    protected override void OnTextInput(TextInputEventArgs typed)
    {
        base.OnTextInput(typed);

        if (typed.Text is { Length: > 0 } text && _pty is not null && !_ended)
        {
            _pty.Write(Encoding.UTF8.GetBytes(text));
            typed.Handled = true;

            ScrolledTo(0);
        }
    }

    /// <summary>Sends the keys that are not text: the arrows, the function keys, and a control chord.</summary>
    /// <param name="pressed">The key.</param>
    protected override void OnKeyDown(KeyEventArgs pressed)
    {
        base.OnKeyDown(pressed);

        if (_ended || _pty is null)
        {
            return;
        }

        var sequence = Sequence(pressed);

        if (sequence is not { Length: > 0 })
        {
            return;
        }

        _pty.Write(Encoding.UTF8.GetBytes(sequence));
        ScrolledTo(0);
        pressed.Handled = true;
    }

    /// <summary>
    /// What a key sends, as a terminal sends it.
    /// </summary>
    /// <remarks>
    /// A key that produces a character is left to the text input; what is here is the keys that produce a
    /// sequence instead. A control chord is the letter's own number, which is what <c>Ctrl-C</c> and the rest
    /// of them are.
    /// </remarks>
    /// <param name="pressed">The key.</param>
    /// <returns>The bytes, as text, or nothing for a key that is not one of these.</returns>
    private static string? Sequence(KeyEventArgs pressed)
    {
        var control = pressed.KeyModifiers.HasFlag(KeyModifiers.Control);

        if (control && pressed.Key is >= Key.A and <= Key.Z)
        {
            return new string((char)((pressed.Key - Key.A) + 1), 1);
        }

        return pressed.Key switch
        {
            Key.Enter => "\r",
            Key.Tab => pressed.KeyModifiers.HasFlag(KeyModifiers.Shift) ? "\u001b[Z" : "\t",
            Key.Back => "\u007f",
            Key.Escape => "\u001b",
            Key.Up => "\u001b[A",
            Key.Down => "\u001b[B",
            Key.Right => "\u001b[C",
            Key.Left => "\u001b[D",
            Key.Home => "\u001b[H",
            Key.End => "\u001b[F",
            Key.Insert => "\u001b[2~",
            Key.Delete => "\u001b[3~",
            Key.PageUp => "\u001b[5~",
            Key.PageDown => "\u001b[6~",
            Key.F1 => "\u001bOP",
            Key.F2 => "\u001bOQ",
            Key.F3 => "\u001bOR",
            Key.F4 => "\u001bOS",
            Key.F5 => "\u001b[15~",
            Key.F6 => "\u001b[17~",
            Key.F7 => "\u001b[18~",
            Key.F8 => "\u001b[19~",
            Key.F9 => "\u001b[20~",
            Key.F10 => "\u001b[21~",
            Key.F11 => "\u001b[23~",
            Key.F12 => "\u001b[24~",
            _ => null,
        };
    }

    /// <summary>Takes the keyboard when the pointer lands on the terminal.</summary>
    /// <param name="pressed">The press.</param>
    protected override void OnPointerPressed(PointerPressedEventArgs pressed)
    {
        base.OnPointerPressed(pressed);

        Focus();
        pressed.Handled = true;
    }

    /// <summary>Scrolls the view back and forward through what the shell has printed.</summary>
    /// <param name="wheeled">The wheel.</param>
    protected override void OnPointerWheelChanged(PointerWheelEventArgs wheeled)
    {
        base.OnPointerWheelChanged(wheeled);

        var step = wheeled.Delta.Y > 0 ? WheelStep : -WheelStep;

        ScrolledTo(_scrolled + step);
        wheeled.Handled = true;
    }

    /// <summary>Scrolls to a place in the scrollback, which is the screen at zero.</summary>
    /// <param name="scrolled">How far back.</param>
    private void ScrolledTo(int scrolled)
    {
        var next = Math.Clamp(scrolled, 0, _screen.Scrolled);

        if (next == _scrolled)
        {
            return;
        }

        _scrolled = next;
        InvalidateVisual();
    }
}
