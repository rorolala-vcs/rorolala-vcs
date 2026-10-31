namespace RorolalaLauncher.Rola;

/// <summary>
/// What waiting for a start of the Desktop program came to.
/// </summary>
/// <param name="Ready">Whether there is a window to look at, which is what the wait was for.</param>
/// <param name="Ended">
/// The code the program ended with, when it ended without a window; nothing when it did not end at all but was
/// waited for only so long.
/// </param>
internal readonly record struct LaunchOutcome(bool Ready, int? Ended);

/// <summary>
/// What a start of the Desktop program has to say for itself, as a question with one answer.
/// </summary>
/// <remarks>
/// Two things answer it, and both are the other programs' own statements rather than this window's guess: the
/// window writes one line on the standard output this program reads, and a start that found the program already
/// going ends with the one exit code that means so. Nothing here is about a process, so it is the part of the
/// wait a test can drive without starting anything.
/// <para>
/// The line and the exit code are spelled here and in the command line program, which is the one place the two
/// agree by hand rather than by sharing a type: the exit code is `EC_DESKTOP_CALLED` in
/// `app/cli/rola/src/exit_codes.rs`, and the line is `ReadySignal.Line` in the Desktop program.
/// </para>
/// </remarks>
internal sealed class Readiness
{
    /// <summary>The line the Desktop program writes when its window is up.</summary>
    public const string Line = "rola-desktop-ready";

    /// <summary>
    /// The code a start ends with when it found the program already going.
    /// </summary>
    /// <remarks>
    /// The one ending that is a window without being a start: the run that was already up was asked to come
    /// forward, and this caller has as much a window to look at as one that opened it.
    /// </remarks>
    public const int DesktopCalled = 174;

    /// <summary>How the answer arrives, whichever of the two ways it arrives by first.</summary>
    private readonly TaskCompletionSource<bool> _answer = new(TaskCreationOptions.RunContinuationsAsynchronously);

    /// <summary>The code the program ended with, or nothing while it has not ended.</summary>
    private int? _ended;

    /// <summary>Takes a line the program wrote.</summary>
    /// <param name="line">What it wrote.</param>
    public void Said(string line)
    {
        ArgumentNullException.ThrowIfNull(line);

        if (string.Equals(line.Trim(), Line, StringComparison.Ordinal))
        {
            _answer.TrySetResult(true);
        }
    }

    /// <summary>Takes what the program ended with.</summary>
    /// <param name="code">The code it ended with.</param>
    public void Ended(int code)
    {
        _ended = code;

        _answer.TrySetResult(code == DesktopCalled);
    }

    /// <summary>Waits for the answer, or for as long as this window is willing to.</summary>
    /// <remarks>
    /// A wait that ends with nothing said is not a start that failed: the window may still be coming, and what
    /// was waited for is this window's own patience rather than the program's end.
    /// </remarks>
    /// <param name="timeout">How long to wait.</param>
    /// <returns>Whether there is a window, and how the program ended when it ended without one.</returns>
    public async Task<LaunchOutcome> WaitAsync(TimeSpan timeout)
    {
        var waited = await Task.WhenAny(_answer.Task, Task.Delay(timeout));

        if (waited != _answer.Task)
        {
            return new LaunchOutcome(false, null);
        }

        return new LaunchOutcome(await _answer.Task, _ended);
    }
}
