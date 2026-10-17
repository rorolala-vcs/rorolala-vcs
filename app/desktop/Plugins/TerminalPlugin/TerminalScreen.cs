using System.Text;

namespace TerminalPlugin;

/// <summary>What a cell is drawn with, over its colour: the marks a terminal may set on a character.</summary>
[Flags]
internal enum CellMarks : byte
{
    /// <summary>Nothing beyond the colours.</summary>
    None = 0,

    /// <summary>Brighter than the ordinary face.</summary>
    Bold = 1,

    /// <summary>Fainter than the ordinary face.</summary>
    Dim = 2,

    /// <summary>Slanted.</summary>
    Italic = 4,

    /// <summary>Lined underneath.</summary>
    Underline = 8,

    /// <summary>Drawn with its two colours exchanged.</summary>
    Reverse = 16,
}

/// <summary>One character of the screen, with the colours and marks it is drawn in.</summary>
internal struct TerminalCell
{
    /// <summary>The character, as a codepoint; -1 is the right half of a wide one, and 0 is blank.</summary>
    public int Codepoint;

    /// <summary>What the character is drawn in, when it is not the terminal's own colour.</summary>
    public uint Foreground;

    /// <summary>What stands behind the character, when it is not the terminal's own ground.</summary>
    public uint Background;

    /// <summary>Whether <see cref="Foreground"/> was set rather than defaulted.</summary>
    public bool HasForeground;

    /// <summary>Whether <see cref="Background"/> was set rather than defaulted.</summary>
    public bool HasBackground;

    /// <summary>The marks the character wears.</summary>
    public CellMarks Marks;

    /// <summary>Whether the cell holds nothing, which is drawn as a space.</summary>
    public readonly bool Blank => Codepoint == 0;

    /// <summary>Whether the cell is the second half of a wide character.</summary>
    public readonly bool Continuation => Codepoint < 0;
}

/// <summary>
/// What a terminal has been told to show: a grid of cells, the cursor in it, and the lines that have
/// scrolled off.
/// </summary>
/// <remarks>
/// The bytes a pseudo-terminal gives back are not text — they are text mixed with the sequences that
/// move the cursor, set a colour, clear a line and so on — so what is kept is the state those bytes
/// leave behind. The sequences understood are the ones a shell, <c>less</c>, <c>top</c> and <c>vim</c>
/// actually send; anything else is read and dropped rather than drawn as text.
/// <para>
/// The screen is not a view: it holds cells and knows nothing about Avalonia, so what a view does is
/// read them and draw. That is also what lets the scrollback be a list of cells rather than a second
/// screen.
/// </para>
/// </remarks>
internal sealed class TerminalScreen
{
    /// <summary>What a blank cell is, which is the codepoint nothing is drawn at.</summary>
    private const int Blank = 0;

    /// <summary>How many lines are kept above the screen before the oldest is let go.</summary>
    private const int ScrollbackLimit = 5000;

    /// <summary>The sixteen colours a terminal names by number, which is the palette every program knows.</summary>
    internal static readonly uint[] Palette =
    [
        0x000000, 0xCD3131, 0x0DBC79, 0xE5E510, 0x2472C8, 0xBC3FBC, 0x11A8CD, 0xE5E5E5,
        0x666666, 0xF14C4C, 0x23D18B, 0xF5F543, 0x3B8EEA, 0xD670D6, 0x29B8DB, 0xE5E5E5,
    ];

    /// <summary>How many columns and rows the screen is.</summary>
    private int _columns;
    private int _rows;

    /// <summary>What is on the screen, row by row.</summary>
    private TerminalCell[] _cells;

    /// <summary>The lines that scrolled off the top, oldest first.</summary>
    private readonly List<TerminalCell[]> _scrollback = [];

    /// <summary>Where the next character is written.</summary>
    private int _cursorX;
    private int _cursorY;

    /// <summary>
    /// Whether the last character ended the line but the cursor has not moved yet.
    /// </summary>
    /// <remarks>
    /// A terminal does not wrap the moment the last column is filled: the cursor stays there and the wrap
    /// waits for the next character, so that a line ending in a newline does not become two lines. Every
    /// movement, a carriage return and a newline all settle it.
    /// </remarks>
    private bool _wrapPending;

    /// <summary>Where a saved cursor went, for the sequence that restores it.</summary>
    private int _savedX;
    private int _savedY;

    /// <summary>The rows a line feed scrolls within, which is the whole screen unless a program says otherwise.</summary>
    private int _scrollTop;
    private int _scrollBottom;

    /// <summary>What is set on the next character, as the last colour and marks asked for.</summary>
    private uint _foreground;
    private uint _background;
    private bool _hasForeground;
    private bool _hasBackground;
    private CellMarks _marks;

    /// <summary>Where the parser is, which is what says how the next character is read.</summary>
    private Parser _state = Parser.Ground;

    /// <summary>The parameters of the sequence being read.</summary>
    private readonly List<int> _parameters = [];

    /// <summary>The number being read, and whether there is one.</summary>
    private int _number;
    private bool _numbered;

    /// <summary>The character that opened a sequence, which tells `?`-prefixed modes from plain ones.</summary>
    private char _prefix;

    /// <summary>The character between a sequence's parameters and its final one, where there is one.</summary>
    private char _intermediate;

    /// <summary>Decodes the bytes into characters, since one character may arrive in several bytes.</summary>
    private readonly Decoder _decoder = Encoding.UTF8.GetDecoder();

    /// <summary>
    /// The rows that have been erased since the cursor was last put somewhere.
    /// </summary>
    /// <remarks>
    /// A screen cleared by a program is cleared by erasing each line in turn — PowerShell's
    /// <c>Clear-Host</c> is exactly that — so a cursor put somewhere after every row has been erased is
    /// a screen that was cleared, and the lines above it are not the reader's to keep.
    /// </remarks>
    private readonly HashSet<int> _erased = [];

    /// <summary>What the decoder made of the bytes it was given.</summary>
    private readonly char[] _text = new char[8192];

    /// <summary>Makes a screen of a size.</summary>
    /// <param name="columns">How many columns.</param>
    /// <param name="rows">How many rows.</param>
    public TerminalScreen(int columns, int rows)
    {
        _columns = Math.Max(1, columns);
        _rows = Math.Max(1, rows);
        _cells = new TerminalCell[_columns * _rows];
        _scrollBottom = _rows - 1;
    }

    /// <summary>What a program wrote to ask for a sequence back, such as where the cursor is.</summary>
    public event Action<string>? Respond;

    /// <summary>How many lines are above the screen, which is how far back a view may scroll.</summary>
    public int Scrolled => _scrollback.Count;

    /// <summary>How many columns the screen is.</summary>
    public int Columns => _columns;

    /// <summary>How many rows the screen is.</summary>
    public int Rows => _rows;

    /// <summary>Where the cursor is, as a column.</summary>
    public int CursorX => _cursorX;

    /// <summary>Where the cursor is, as a row.</summary>
    public int CursorY => _cursorY;

    /// <summary>Whether the cursor is shown, which a program may turn off while it draws.</summary>
    public bool CursorVisible { get; private set; } = true;

    /// <summary>Which row of the screen a view shows, from the top of the window into the scrollback.</summary>
    /// <param name="row">The row of the window, from zero.</param>
    /// <param name="scrolled">How far back the window is, which is zero at the screen itself.</param>
    /// <returns>The cells of that row.</returns>
    public ReadOnlySpan<TerminalCell> Line(int row, int scrolled)
    {
        var back = Math.Clamp(scrolled, 0, _scrollback.Count);
        var index = _scrollback.Count - back + row;

        if (index < 0 || index >= _scrollback.Count + _rows)
        {
            return [];
        }

        return index < _scrollback.Count
            ? _scrollback[index]
            : _cells.AsSpan((index - _scrollback.Count) * _columns, _columns);
    }

    /// <summary>Says how large the view is, keeping what was on the screen.</summary>
    /// <param name="columns">How many columns.</param>
    /// <param name="rows">How many rows.</param>
    public void Resize(int columns, int rows)
    {
        var next = Math.Max(1, columns);
        var height = Math.Max(1, rows);

        if (next == _columns && height == _rows)
        {
            return;
        }

        var cells = new TerminalCell[next * height];

        for (var row = 0; row < Math.Min(height, _rows); row++)
        {
            Array.Copy(_cells, row * _columns, cells, row * next, Math.Min(_columns, next));
        }

        _cells = cells;
        _columns = next;
        _rows = height;

        _cursorX = Math.Min(_cursorX, _columns - 1);
        _cursorY = Math.Min(_cursorY, _rows - 1);
        _scrollTop = 0;
        _scrollBottom = _rows - 1;
        _erased.Clear();
    }

    /// <summary>Reads bytes the program wrote, drawing and acting on them.</summary>
    /// <param name="bytes">The bytes.</param>
    /// <returns>Whether anything the view draws changed.</returns>
    public bool Feed(ReadOnlySpan<byte> bytes)
    {
        if (bytes.IsEmpty)
        {
            return false;
        }

        if (_text.Length < bytes.Length)
        {
            return FeedSlow(bytes);
        }

        var count = _decoder.GetChars(bytes, _text, false);

        return Read(_text.AsSpan(0, count));
    }

    /// <summary>Reads a batch too large for the decoder's own buffer, in pieces.</summary>
    /// <param name="bytes">The bytes.</param>
    /// <returns>Whether anything changed.</returns>
    private bool FeedSlow(ReadOnlySpan<byte> bytes)
    {
        var changed = false;

        while (!bytes.IsEmpty)
        {
            var take = Math.Min(_text.Length, bytes.Length);
            var count = _decoder.GetChars(bytes[..take], _text, false);

            changed |= Read(_text.AsSpan(0, count));
            bytes = bytes[take..];
        }

        return changed;
    }

    /// <summary>Reads the characters one at a time through the parser.</summary>
    /// <param name="text">The characters.</param>
    /// <returns>Whether anything changed.</returns>
    private bool Read(ReadOnlySpan<char> text)
    {
        var changed = false;

        for (var at = 0; at < text.Length; at++)
        {
            var character = text[at];

            // A character outside the basic plane is two units, and the pair is one character to print.
            if (char.IsHighSurrogate(character) && at + 1 < text.Length && char.IsLowSurrogate(text[at + 1]))
            {
                changed |= Take(char.ConvertToUtf32(character, text[++at]));

                continue;
            }

            changed |= Take(character);
        }

        return changed;
    }

    /// <summary>Takes one character, as the state says it is read.</summary>
    /// <param name="character">The character.</param>
    /// <returns>Whether anything changed.</returns>
    private bool Take(int character)
    {
        switch (_state)
        {
            case Parser.Ground:
                return Grounded(character);

            case Parser.Escape:
                return Escaped(character);

            case Parser.Csi:
                return Sequenced(character);

            case Parser.Osc:
                return Oscillated(character);

            case Parser.OscEscape:
                // An escape inside an operating-system command either terminates it or is a sequence that
                // was cut short; either way what is being read is over.
                _state = Parser.Ground;

                return false;

            default:
                _state = Parser.Ground;

                return false;
        }
    }

    /// <summary>Takes a character while nothing is being read but text.</summary>
    /// <param name="character">The character.</param>
    /// <returns>Whether anything changed.</returns>
    private bool Grounded(int character)
    {
        switch (character)
        {
            case '\u001b':
                _state = Parser.Escape;

                return false;

            case '\a':
                return false;

            case '\b':
                if (_wrapPending)
                {
                    _wrapPending = false;
                }
                else
                {
                    _cursorX = Math.Max(0, _cursorX - 1);
                }

                return true;

            case '\t':
                _cursorX = Math.Min(_columns - 1, ((_cursorX / 8) * 8) + 8);
                _wrapPending = false;

                return true;

            case '\n':
            case '\v':
            case '\f':
                if (_wrapPending)
                {
                    _cursorX = 0;
                    _wrapPending = false;
                }

                LineFeed();

                return true;

            case '\r':
                _cursorX = 0;
                _wrapPending = false;

                return true;

            case '\u000e':
            case '\u000f':
            case '\u0000':
                return false;

            default:
                if (character < 0x20)
                {
                    return false;
                }

                Print(character);

                return true;
        }
    }

    /// <summary>Takes the character after an escape.</summary>
    /// <param name="character">The character.</param>
    /// <returns>Whether anything changed.</returns>
    private bool Escaped(int character)
    {
        _state = Parser.Ground;
        _wrapPending = false;

        switch (character)
        {
            case '[':
                _state = Parser.Csi;
                _parameters.Clear();
                _number = 0;
                _numbered = false;
                _prefix = '\0';
                _intermediate = '\0';

                return false;

            case ']':
                _state = Parser.Osc;

                return false;

            case '(':
            case ')':
                _state = Parser.Charset;

                return false;

            case '7':
                _savedX = _cursorX;
                _savedY = _cursorY;

                return false;

            case '8':
                _cursorX = Math.Min(_savedX, _columns - 1);
                _cursorY = Math.Min(_savedY, _rows - 1);

                return true;

            case 'D':
                LineFeed();

                return true;

            case 'E':
                _cursorX = 0;
                LineFeed();

                return true;

            case 'M':
                ReverseIndex();

                return true;

            case 'c':
                Reset();

                return true;

            default:
                return false;
        }
    }

    /// <summary>Takes a character of a control sequence.</summary>
    /// <param name="character">The character.</param>
    /// <returns>Whether anything changed.</returns>
    private bool Sequenced(int character)
    {
        if (character is >= '0' and <= '9')
        {
            _number = (_number * 10) + (character - '0');
            _numbered = true;

            return false;
        }

        if (character == ';')
        {
            _parameters.Add(_numbered ? _number : 0);
            _number = 0;
            _numbered = false;

            return false;
        }

        if (character is '?' or '>' && _parameters.Count == 0 && !_numbered)
        {
            _prefix = (char)character;

            return false;
        }

        if (character is >= ' ' and <= '/')
        {
            _intermediate = (char)character;

            return false;
        }

        if (character is < '@' or > '~')
        {
            return false;
        }

        _parameters.Add(_numbered ? _number : 0);
        _state = Parser.Ground;
        _wrapPending = false;

        return Final((char)character);
    }

    /// <summary>Takes a character of an operating-system command, which is read and dropped.</summary>
    /// <param name="character">The character.</param>
    /// <returns>Whether anything changed.</returns>
    private bool Oscillated(int character)
    {
        if (character == '\a')
        {
            _state = Parser.Ground;
        }
        else if (character == '\u001b')
        {
            _state = Parser.OscEscape;
        }

        return false;
    }

    /// <summary>Acts on a control sequence that has been read in full.</summary>
    /// <param name="final">The character that ended it.</param>
    /// <returns>Whether anything changed.</returns>
    private bool Final(char final)
    {
        if (_prefix is '?' or '>')
        {
            return Mode(final);
        }

        switch (final)
        {
            case 'A':
                _cursorY = Math.Max(_scrollTop, _cursorY - Count(0, 1));

                return true;

            case 'B':
                _cursorY = Math.Min(_scrollBottom, _cursorY + Count(0, 1));

                return true;

            case 'C':
                _cursorX = Math.Min(_columns - 1, _cursorX + Count(0, 1));

                return true;

            case 'D':
                _cursorX = Math.Max(0, _cursorX - Count(0, 1));

                return true;

            case 'E':
                _cursorY = Math.Min(_scrollBottom, _cursorY + Count(0, 1));
                _cursorX = 0;

                return true;

            case 'F':
                _cursorY = Math.Max(_scrollTop, _cursorY - Count(0, 1));
                _cursorX = 0;

                return true;

            case 'G':
                _cursorX = Math.Clamp(Count(0, 1) - 1, 0, _columns - 1);

                return true;

            case 'H':
            case 'f':
                // A cursor moved after every row was erased is a screen that was cleared, and what was
                // above the screen before it is not what the reader is looking at any more.
                if (_erased.Count >= _rows)
                {
                    _scrollback.Clear();
                }

                _erased.Clear();
                _cursorY = Math.Clamp(Count(0, 1) - 1, 0, _rows - 1);
                _cursorX = Math.Clamp(Count(1, 1) - 1, 0, _columns - 1);

                return true;

            case 'd':
                _cursorY = Math.Clamp(Count(0, 1) - 1, 0, _rows - 1);

                return true;

            case 'J':
                EraseScreen(Count(0, 0));

                return true;

            case 'K':
                EraseLine(Count(0, 0));

                return true;

            case 'X':
                EraseCells(Count(0, 1));

                return true;

            case 'm':
                Style();

                return true;

            case 'r':
                _scrollTop = Math.Clamp(Count(0, 1) - 1, 0, _rows - 1);
                _scrollBottom = Math.Clamp(Count(1, _rows) - 1, _scrollTop, _rows - 1);
                _cursorX = 0;
                _cursorY = _scrollTop;

                return true;

            case 'S':
                ScrollUp(Count(0, 1));

                return true;

            case 'T':
                ScrollDown(Count(0, 1));

                return true;

            case 'L':
                InsertLines(Count(0, 1));

                return true;

            case 'M':
                DeleteLines(Count(0, 1));

                return true;

            case 'P':
                DeleteCharacters(Count(0, 1));

                return true;

            case '@':
                InsertCharacters(Count(0, 1));

                return true;

            case 's':
                _savedX = _cursorX;
                _savedY = _cursorY;

                return false;

            case 'u':
                _cursorX = Math.Min(_savedX, _columns - 1);
                _cursorY = Math.Min(_savedY, _rows - 1);

                return true;

            // A program asking where the cursor is gets the answer, which is how a shell works out a
            // prompt that wraps.
            case 'n' when Count(0, 0) == 6:
                Respond?.Invoke($"\u001b[{_cursorY + 1};{_cursorX + 1}R");

                return false;

            // A program asking what this terminal is gets a plain one, which is enough for anything that
            // only needs to know the answer is not nothing.
            case 'c' when _parameters.Count > 0 && Count(0, 0) == 0:
                Respond?.Invoke("\u001b[?6c");

                return false;

            default:
                return false;
        }
    }

    /// <summary>Acts on a mode being set or taken back.</summary>
    /// <param name="final">The character that ended it, which is `h` to set and `l` to take back.</param>
    /// <returns>Whether anything changed.</returns>
    private bool Mode(char final)
    {
        var on = final == 'h';

        if (_prefix == '>' || _intermediate is not '\0')
        {
            return false;
        }

        foreach (var parameter in _parameters)
        {
            switch (parameter)
            {
                case 25:
                    CursorVisible = on;

                    break;

                // The alternate screen: what a program that fills the window draws on. It is not a
                // second buffer here — entering it clears, leaving it clears — which is enough for a
                // program that owns the whole screen while it runs.
                case 47:
                case 1047:
                case 1049:
                    Clear();
                    _cursorX = 0;
                    _cursorY = 0;

                    break;

                default:
                    break;
            }
        }

        return true;
    }

    /// <summary>Sets what the next characters are drawn in, from a sequence's parameters.</summary>
    private void Style()
    {
        for (var at = 0; at < _parameters.Count; at++)
        {
            var parameter = _parameters[at];

            switch (parameter)
            {
                case 0:
                    _foreground = 0;
                    _background = 0;
                    _hasForeground = false;
                    _hasBackground = false;
                    _marks = CellMarks.None;

                    break;

                case 1:
                    _marks |= CellMarks.Bold;

                    break;

                case 2:
                    _marks |= CellMarks.Dim;

                    break;

                case 3:
                    _marks |= CellMarks.Italic;

                    break;

                case 4:
                    _marks |= CellMarks.Underline;

                    break;

                case 7:
                    _marks |= CellMarks.Reverse;

                    break;

                case 21:
                case 22:
                    _marks &= ~(CellMarks.Bold | CellMarks.Dim);

                    break;

                case 23:
                    _marks &= ~CellMarks.Italic;

                    break;

                case 24:
                    _marks &= ~CellMarks.Underline;

                    break;

                case 27:
                    _marks &= ~CellMarks.Reverse;

                    break;

                case >= 30 and <= 37:
                    _foreground = Palette[parameter - 30];
                    _hasForeground = true;

                    break;

                case 39:
                    _hasForeground = false;

                    break;

                case >= 40 and <= 47:
                    _background = Palette[parameter - 40];
                    _hasBackground = true;

                    break;

                case 49:
                    _hasBackground = false;

                    break;

                case >= 90 and <= 97:
                    _foreground = Palette[parameter - 90 + 8];
                    _hasForeground = true;

                    break;

                case >= 100 and <= 107:
                    _background = Palette[parameter - 100 + 8];
                    _hasBackground = true;

                    break;

                // The two forms that name a colour by an index or by its three parts: the parts after the
                // introducer are the colour, so they are read here rather than as parameters.
                case 38 or 48:
                    at = Extended(parameter == 38, at);

                    break;

                default:
                    break;
            }
        }
    }

    /// <summary>Reads the colour an extended parameter names, and moves past it.</summary>
    /// <param name="foreground">Whether it is the text or the ground.</param>
    /// <param name="at">Where the introducer was.</param>
    /// <returns>Where the last part of the colour was.</returns>
    private int Extended(bool foreground, int at)
    {
        if (at + 1 >= _parameters.Count)
        {
            return at;
        }

        var form = _parameters[at + 1];

        if (form == 5 && at + 2 < _parameters.Count)
        {
            var index = _parameters[at + 2];
            var colour = index is >= 0 and < 16
                ? Palette[index]
                : index is >= 16 and < 232
                    ? Cube(index - 16)
                    : index is >= 232 and < 256
                        ? Grey(index - 232)
                        : 0;

            Set(foreground, colour);

            return at + 2;
        }

        if (form == 2 && at + 4 < _parameters.Count)
        {
            var colour = ((uint)(_parameters[at + 2] & 0xFF) << 16)
                | ((uint)(_parameters[at + 3] & 0xFF) << 8)
                | (uint)(_parameters[at + 4] & 0xFF);

            Set(foreground, colour);

            return at + 4;
        }

        return at + 1;
    }

    /// <summary>Sets the text or the ground to a colour.</summary>
    /// <param name="foreground">Whether it is the text.</param>
    /// <param name="colour">The colour.</param>
    private void Set(bool foreground, uint colour)
    {
        if (foreground)
        {
            _foreground = colour;
            _hasForeground = true;
        }
        else
        {
            _background = colour;
            _hasBackground = true;
        }
    }

    /// <summary>A colour of the six-by-six-by-six cube the palette's upper half is.</summary>
    /// <param name="index">The index into the cube.</param>
    /// <returns>The colour.</returns>
    private static uint Cube(int index)
    {
        static uint Level(int part) => (uint)(part == 0 ? 0 : 55 + (part * 40));

        return (Level(index / 36) << 16) | (Level((index / 6) % 6) << 8) | Level(index % 6);
    }

    /// <summary>One of the greys the palette's last twenty-four are.</summary>
    /// <param name="index">The index into the greys.</param>
    /// <returns>The colour.</returns>
    private static uint Grey(int index)
    {
        var level = (uint)(8 + (index * 10));

        return (level << 16) | (level << 8) | level;
    }

    /// <summary>Puts a character at the cursor and moves on.</summary>
    /// <param name="codepoint">The character.</param>
    private void Print(int codepoint)
    {
        var width = Width(codepoint);

        // A wide character does not fit in the last column, so the line is ended rather than split.
        if (_wrapPending || (width == 2 && _cursorX >= _columns - 1))
        {
            _cursorX = 0;
            _wrapPending = false;
            LineFeed();
        }

        Put(_cursorX, _cursorY, codepoint);

        if (width == 2 && _cursorX + 1 < _columns)
        {
            Put(_cursorX + 1, _cursorY, -1);
        }

        _cursorX += width;

        if (_cursorX >= _columns)
        {
            _cursorX = _columns - 1;
            _wrapPending = true;
        }
    }

    /// <summary>Writes one cell with what is currently set.</summary>
    /// <param name="x">The column.</param>
    /// <param name="y">The row.</param>
    /// <param name="codepoint">The character.</param>
    private void Put(int x, int y, int codepoint)
    {
        if (x < 0 || x >= _columns || y < 0 || y >= _rows)
        {
            return;
        }

        _cells[(y * _columns) + x] = new TerminalCell
        {
            Codepoint = codepoint,
            Foreground = _foreground,
            Background = _background,
            HasForeground = _hasForeground,
            HasBackground = _hasBackground,
            Marks = _marks,
        };
    }

    /// <summary>Clears one cell, keeping no mark or colour.</summary>
    /// <param name="x">The column.</param>
    /// <param name="y">The row.</param>
    private void Wipe(int x, int y)
    {
        if (x < 0 || x >= _columns || y < 0 || y >= _rows)
        {
            return;
        }

        _cells[(y * _columns) + x] = new TerminalCell { Codepoint = Blank };
    }

    /// <summary>Moves down a line, scrolling at the bottom of the region.</summary>
    private void LineFeed()
    {
        if (_cursorY == _scrollBottom)
        {
            ScrollUp(1);
        }
        else if (_cursorY < _rows - 1)
        {
            _cursorY++;
        }
    }

    /// <summary>Moves up a line, scrolling back at the top of the region.</summary>
    private void ReverseIndex()
    {
        if (_cursorY == _scrollTop)
        {
            ScrollDown(1);
        }
        else if (_cursorY > 0)
        {
            _cursorY--;
        }
    }

    /// <summary>Moves lines in the region up, letting the top ones go.</summary>
    /// <param name="count">How many lines.</param>
    private void ScrollUp(int count)
    {
        _erased.Clear();

        for (var move = 0; move < count; move++)
        {
            // Only a scroll of the whole screen goes into the scrollback: a program that scrolls inside a
            // region is drawing a window of its own, and those lines are gone when it moves on.
            if (_scrollTop == 0 && _scrollBottom == _rows - 1)
            {
                _scrollback.Add(_cells[.._columns]);

                if (_scrollback.Count > ScrollbackLimit)
                {
                    _scrollback.RemoveAt(0);
                }
            }

            Array.Copy(
                _cells,
                (_scrollTop + 1) * _columns,
                _cells,
                _scrollTop * _columns,
                (_scrollBottom - _scrollTop) * _columns
            );

            WipeRow(_scrollBottom);
        }
    }

    /// <summary>Moves lines in the region down, letting the bottom ones go.</summary>
    /// <param name="count">How many lines.</param>
    private void ScrollDown(int count)
    {
        for (var move = 0; move < count; move++)
        {
            Array.Copy(
                _cells,
                _scrollTop * _columns,
                _cells,
                (_scrollTop + 1) * _columns,
                (_scrollBottom - _scrollTop) * _columns
            );

            WipeRow(_scrollTop);
        }
    }

    /// <summary>Clears a row.</summary>
    /// <param name="row">The row.</param>
    private void WipeRow(int row)
    {
        for (var x = 0; x < _columns; x++)
        {
            Wipe(x, row);
        }
    }

    /// <summary>Clears what a display-erase sequence names.</summary>
    /// <param name="what">The form: to the end, to the start, or the whole screen.</param>
    private void EraseScreen(int what)
    {
        switch (what)
        {
            case 0:
                for (var x = _cursorX; x < _columns; x++)
                {
                    Wipe(x, _cursorY);
                }

                _erased.Add(_cursorY);

                for (var y = _cursorY + 1; y < _rows; y++)
                {
                    WipeRow(y);
                    _erased.Add(y);
                }

                break;

            case 1:
                for (var y = 0; y < _cursorY; y++)
                {
                    WipeRow(y);
                    _erased.Add(y);
                }

                for (var x = 0; x <= _cursorX && x < _columns; x++)
                {
                    Wipe(x, _cursorY);
                }

                _erased.Add(_cursorY);

                break;

            // Two is the screen and three is the lines above it, which a shell's `clear` sends to say that
            // what was read is not there to scroll back to: two is answered with both, since a screen
            // cleared under a reader is a screen whose history went with it.
            case 2:
                Clear();
                _scrollback.Clear();

                break;

            case 3:
                _scrollback.Clear();

                break;

            default:
                for (var x = _cursorX; x < _columns; x++)
                {
                    Wipe(x, _cursorY);
                }

                for (var y = _cursorY + 1; y < _rows; y++)
                {
                    WipeRow(y);
                }

                break;
        }
    }

    /// <summary>Clears what a line-erase sequence names.</summary>
    /// <param name="what">The form: to the end, to the start, or the whole line.</param>
    private void EraseLine(int what)
    {
        switch (what)
        {
            case 0:
                for (var x = _cursorX; x < _columns; x++)
                {
                    Wipe(x, _cursorY);
                }

                break;

            case 1:
                for (var x = 0; x <= _cursorX && x < _columns; x++)
                {
                    Wipe(x, _cursorY);
                }

                break;

            default:
                WipeRow(_cursorY);

                break;
        }

        _erased.Add(_cursorY);
    }

    /// <summary>Clears characters from the cursor on, without moving anything.</summary>
    /// <param name="count">How many.</param>
    private void EraseCells(int count)
    {
        for (var at = 0; at < count && _cursorX + at < _columns; at++)
        {
            Wipe(_cursorX + at, _cursorY);
        }
    }

    /// <summary>Opens room above the cursor, moving lines down.</summary>
    /// <param name="count">How many lines.</param>
    private void InsertLines(int count)
    {
        if (_cursorY < _scrollTop || _cursorY > _scrollBottom)
        {
            return;
        }

        for (var move = 0; move < count; move++)
        {
            Array.Copy(
                _cells,
                _cursorY * _columns,
                _cells,
                (_cursorY + 1) * _columns,
                (_scrollBottom - _cursorY) * _columns
            );

            WipeRow(_cursorY);
        }
    }

    /// <summary>Takes lines away at the cursor, moving the ones below up.</summary>
    /// <param name="count">How many lines.</param>
    private void DeleteLines(int count)
    {
        if (_cursorY < _scrollTop || _cursorY > _scrollBottom)
        {
            return;
        }

        for (var move = 0; move < count; move++)
        {
            Array.Copy(
                _cells,
                (_cursorY + 1) * _columns,
                _cells,
                _cursorY * _columns,
                (_scrollBottom - _cursorY) * _columns
            );

            WipeRow(_scrollBottom);
        }
    }

    /// <summary>Opens room inside a line, moving what is after the cursor on.</summary>
    /// <param name="count">How many cells.</param>
    private void InsertCharacters(int count)
    {
        var room = Math.Min(count, _columns - _cursorX);

        if (room <= 0)
        {
            return;
        }

        Array.Copy(
            _cells,
            (_cursorY * _columns) + _cursorX,
            _cells,
            (_cursorY * _columns) + _cursorX + room,
            _columns - _cursorX - room
        );

        for (var at = 0; at < room; at++)
        {
            Wipe(_cursorX + at, _cursorY);
        }
    }

    /// <summary>Takes cells out of a line, moving what is after the cursor back.</summary>
    /// <param name="count">How many cells.</param>
    private void DeleteCharacters(int count)
    {
        var gone = Math.Min(count, _columns - _cursorX);

        if (gone <= 0)
        {
            return;
        }

        Array.Copy(
            _cells,
            (_cursorY * _columns) + _cursorX + gone,
            _cells,
            (_cursorY * _columns) + _cursorX,
            _columns - _cursorX - gone
        );

        for (var at = 0; at < gone; at++)
        {
            Wipe(_columns - 1 - at, _cursorY);
        }
    }

    /// <summary>Clears the screen and puts the cursor at the top left.</summary>
    private void Clear()
    {
        _cells = new TerminalCell[_columns * _rows];
        _scrollTop = 0;
        _scrollBottom = _rows - 1;
        _wrapPending = false;
        _erased.Clear();
    }

    /// <summary>Puts everything back as a terminal starts.</summary>
    private void Reset()
    {
        Clear();
        _cursorX = 0;
        _cursorY = 0;
        _savedX = 0;
        _savedY = 0;
        _foreground = 0;
        _background = 0;
        _hasForeground = false;
        _hasBackground = false;
        _marks = CellMarks.None;
        CursorVisible = true;
        _state = Parser.Ground;
    }

    /// <summary>A parameter of a sequence, or the value it stands for when it was left out.</summary>
    /// <param name="at">Which parameter.</param>
    /// <param name="fallback">What it means when it was left out or zero.</param>
    /// <returns>The parameter.</returns>
    private int Count(int at, int fallback) =>
        at < _parameters.Count && _parameters[at] > 0 ? _parameters[at] : fallback;

    /// <summary>
    /// How many columns a character takes.
    /// </summary>
    /// <remarks>
    /// A terminal's cells are not one per character: the scripts of East Asia draw two cells wide, and a
    /// program lays its screen out by that. The ranges here are the ones a terminal is expected to know,
    /// which is enough for a shell, an editor and a pager; anything outside them is one cell.
    /// </remarks>
    /// <param name="codepoint">The character.</param>
    /// <returns>One or two.</returns>
    internal static int Width(int codepoint) =>
        (codepoint >= 0x1100 && codepoint <= 0x115F)
        || (codepoint >= 0x2E80 && codepoint <= 0xA4CF)
        || (codepoint >= 0xA960 && codepoint <= 0xA97F)
        || (codepoint >= 0xAC00 && codepoint <= 0xD7A3)
        || (codepoint >= 0xF900 && codepoint <= 0xFAFF)
        || (codepoint >= 0xFE10 && codepoint <= 0xFE19)
        || (codepoint >= 0xFE30 && codepoint <= 0xFE6F)
        || (codepoint >= 0xFF00 && codepoint <= 0xFF60)
        || (codepoint >= 0xFFE0 && codepoint <= 0xFFE6)
        || (codepoint >= 0x1F300 && codepoint <= 0x1F64F)
        || (codepoint >= 0x1F900 && codepoint <= 0x1F9FF)
        || (codepoint >= 0x20000 && codepoint <= 0x3FFFD)
            ? 2
            : 1;

    /// <summary>Where the parser is between bytes.</summary>
    private enum Parser
    {
        /// <summary>Reading text.</summary>
        Ground,

        /// <summary>Read the escape, waiting for what it begins.</summary>
        Escape,

        /// <summary>Reading a control sequence.</summary>
        Csi,

        /// <summary>Reading an operating-system command.</summary>
        Osc,

        /// <summary>Read an escape inside an operating-system command.</summary>
        OscEscape,

        /// <summary>Reading the one character that names a character set.</summary>
        Charset,
    }
}
