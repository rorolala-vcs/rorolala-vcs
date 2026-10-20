using System.Diagnostics.CodeAnalysis;

using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Controls;

/// <summary>
/// The lines the launcher says about itself, written a letter at a time, with the caret a writer's cursor draws.
/// </summary>
/// <remarks>
/// A line is typed rather than shown because it is what the window says about itself before anything has been
/// done in it: a sentence arriving a letter at a time is read, where a sentence sitting there is looked past.
/// The caret blinks once the line is whole, which is what says the window is ready rather than still writing.
/// <para>
/// There is more than one line, and they take turns: a line is written, held for a moment, and then the next
/// is written in its place. The first one is picked at random rather than the list being read in order, so
/// that two runs of the program do not open with the same sentence.
/// </para>
/// <para>
/// The whole line is measured before any of it is written, and the writing happens in that width: a line
/// grown from nothing in the middle of the window would move its own left edge with every letter, and what is
/// typed is read from a still left edge or not read at all.
/// </para>
/// </remarks>
internal sealed class SloganView : UserControl
{
    /// <summary>How long a letter takes to arrive.</summary>
    private static readonly TimeSpan Letter = TimeSpan.FromMilliseconds(42);

    /// <summary>How long the caret is shown for, and then hidden for.</summary>
    private static readonly TimeSpan Blink = TimeSpan.FromMilliseconds(530);

    /// <summary>How long one whole line is held before the next is written.</summary>
    private static readonly TimeSpan Hold = TimeSpan.FromMilliseconds(3200);

    /// <summary>What is written, which is revealed a letter at a time.</summary>
    private readonly TextBlock _writing = new()
    {
        FontSize = 15,
        VerticalAlignment = VerticalAlignment.Center,
        MaxWidth = 640,
        TextWrapping = TextWrapping.NoWrap,
        TextTrimming = TextTrimming.CharacterEllipsis,
    };

    /// <summary>The whole line, which is what the writing is measured against.</summary>
    private readonly TextBlock _reserve = new()
    {
        FontSize = 15,
        MaxWidth = 640,
        TextWrapping = TextWrapping.NoWrap,
        TextTrimming = TextTrimming.CharacterEllipsis,
        IsHitTestVisible = false,
        Opacity = 0,
    };

    /// <summary>The caret after the last letter written.</summary>
    private readonly Border _caret = new()
    {
        Width = 2,
        Height = 16,
        Margin = new Thickness(3, 0, 0, 0),
        CornerRadius = new CornerRadius(0),
        VerticalAlignment = VerticalAlignment.Center,
    };

    /// <summary>The clock the letters arrive on.</summary>
    private readonly DispatcherTimer _typing = new() { Interval = Letter };

    /// <summary>The clock the caret blinks on.</summary>
    private readonly DispatcherTimer _blinking = new() { Interval = Blink };

    /// <summary>The clock a whole line waits on before the next one takes its place.</summary>
    private readonly DispatcherTimer _holding = new() { Interval = Hold };

    /// <summary>Every line there is to say, in the order the file states them.</summary>
    private IReadOnlyList<string> _lines = [];

    /// <summary>Which of them is being said.</summary>
    private int _which;

    /// <summary>What is being said, which is the line <see cref="_which"/> names.</summary>
    private string _said = string.Empty;

    /// <summary>How much of it has arrived.</summary>
    private int _arrived;

    /// <summary>Makes the line, saying nothing yet.</summary>
    /// <param name="skin">The colours in force.</param>
    public SloganView(Skin skin)
    {
        ArgumentNullException.ThrowIfNull(skin);

        _writing.Foreground = skin.Front;
        _reserve.Foreground = skin.Front;
        _caret.Background = skin.Front;

        _typing.Tick += (_, _) => Arrive();
        _blinking.Tick += (_, _) => _caret.Opacity = _caret.Opacity > 0.5 ? 0 : 1;
        _holding.Tick += (_, _) => Next();

        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Center,
        };

        row.Children.Add(_writing);
        row.Children.Add(_caret);

        // The line being written and the line it will become are laid over one another, so that the space the
        // sentence will take is kept from the first letter rather than opening up as it is written.
        var measured = new Grid { HorizontalAlignment = HorizontalAlignment.Center };

        measured.Children.Add(_reserve);
        measured.Children.Add(row);

        Content = measured;
    }

    /// <summary>
    /// Says the lines there are, beginning with one of them at random.
    /// </summary>
    /// <param name="lines">The lines to say.</param>
    [SuppressMessage(
        "Security",
        "CA5394:Do not use insecure randomness",
        Justification = "the choice is which sentence is said first, which is not a secret and must not block on entropy"
    )]
    public void Say(IReadOnlyList<string> lines)
    {
        ArgumentNullException.ThrowIfNull(lines);

        Stop();

        _lines = lines;
        _which = lines.Count == 0 ? 0 : Random.Shared.Next(lines.Count);

        Begin();
    }

    /// <summary>Stops every clock, which is what being taken off the window means.</summary>
    private void Stop()
    {
        _typing.Stop();
        _blinking.Stop();
        _holding.Stop();
    }

    /// <summary>Puts the line up and starts writing it.</summary>
    private void Begin()
    {
        _said = _lines.Count == 0 ? string.Empty : _lines[_which];
        _arrived = 0;
        _reserve.Text = _said;
        _writing.Text = string.Empty;
        _caret.Opacity = 1;

        if (_said.Length == 0)
        {
            _blinking.Start();

            return;
        }

        _typing.Start();
    }

    /// <summary>Writes the next letter, and holds the line whole once there are no more.</summary>
    private void Arrive()
    {
        _arrived = Math.Min(_arrived + 1, _said.Length);
        _writing.Text = _said[.._arrived];

        if (_arrived < _said.Length)
        {
            return;
        }

        _typing.Stop();
        _blinking.Start();

        // One line is not a rotation: it is the line, and writing it again would be the window repeating
        // itself rather than saying something else.
        if (_lines.Count > 1)
        {
            _holding.Start();
        }
    }

    /// <summary>Takes the next line, in the order the file states them and around again.</summary>
    private void Next()
    {
        _holding.Stop();
        _blinking.Stop();

        _which = (_which + 1) % _lines.Count;

        Begin();
    }

    /// <inheritdoc />
    protected override void OnDetachedFromVisualTree(VisualTreeAttachmentEventArgs e)
    {
        Stop();

        base.OnDetachedFromVisualTree(e);
    }
}
