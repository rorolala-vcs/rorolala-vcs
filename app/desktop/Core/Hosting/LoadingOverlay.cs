using Avalonia;
using Avalonia.Animation;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// The black a window is drawn under until the work behind it is done.
/// </summary>
/// <remarks>
/// It covers the client area and takes the press: the window is on the screen before there is anything to do
/// in it, and the first frame being the window rather than a half-made shell is the whole of what this is for.
/// What it cannot cover is the window's own buttons, which the frame draws over the client area rather than in
/// it — a frame that belongs to the desktop's window is not the covering's to hide.
/// <para>
/// What is on it is a mark that turns, and nothing else. The work it is over is short enough that a line of
/// copy would be read at most once and often not finished, so there is nothing here to read: the mark says
/// that something is happening and that the window is not hung, and that is all a wait this short needs.
/// </para>
/// <para>
/// What ends it is the assembly's own definition of done: every step of the work has returned and a frame has
/// been drawn. It is not "every dock has its data": a file listing and a version state are read asynchronously
/// by design, and a covering that waited for them would be a covering over a window the user can already work
/// in.
/// </para>
/// </remarks>
internal sealed class LoadingOverlay : Border
{
    /// <summary>How long the covering takes to go once the work is done, matching the look's own fades.</summary>
    private static readonly TimeSpan Fade = TimeSpan.FromMilliseconds(200);

    /// <summary>
    /// The ink the mark is drawn in.
    /// </summary>
    /// <remarks>
    /// A colour of its own rather than one of the look's: the look is drawn for a window, and this is drawn over
    /// one before the look has anything to draw. What is wanted is the one colour that is read on black.
    /// </remarks>
    private static readonly IBrush Ink = new SolidColorBrush(Color.FromRgb(0xE6, 0xE6, 0xE6));

    /// <summary>How wide the mark is, which is as wide as a wait this short wants to be looked at.</summary>
    private const double Diameter = 26;

    /// <summary>The mark that turns while the work goes on.</summary>
    private readonly TurningMark _mark;

    /// <summary>Whether the covering has been asked to go, so that it goes once.</summary>
    private bool _going;

    /// <summary>Makes the covering, with its mark already turning.</summary>
    public LoadingOverlay()
    {
        Background = Brushes.Black;
        HorizontalAlignment = HorizontalAlignment.Stretch;
        VerticalAlignment = VerticalAlignment.Stretch;

        // The covering goes rather than being taken away in one turn: what is under it arriving is what the
        // fade is for, and the window that was black is seen becoming the window.
        Transitions = new Transitions { new DoubleTransition { Property = OpacityProperty, Duration = Fade } };

        _mark = new TurningMark(Ink, Diameter);

        Child = new Grid { Children = { _mark } };
    }

    /// <summary>Ends the covering: it goes, and what it was over is seen.</summary>
    /// <remarks>
    /// Idempotent, because the work that ends it and the window being closed at the same moment are both things
    /// that happen: a covering already going is one with nothing left to start.
    /// </remarks>
    public void Finish()
    {
        if (_going)
        {
            return;
        }

        _going = true;

        Opacity = 0;

        // Taken out of the window once it has gone rather than at once: a covering hidden in the same turn it
        // starts to fade would be a flash of the work rather than the work arriving, and a transparent covering
        // left over the window would go on taking its presses. The mark goes with it, which is what stops it
        // turning for a window that is no longer loading anything.
        var gone = new DispatcherTimer { Interval = Fade };

        gone.Tick += (_, _) =>
        {
            gone.Stop();

            Child = null;
            IsVisible = false;
            IsHitTestVisible = false;
        };

        gone.Start();
    }
}
