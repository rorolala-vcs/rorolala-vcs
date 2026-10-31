namespace RorolalaDesktop;

/// <summary>
/// The one line this program writes when the window a caller asked for is up.
/// </summary>
/// <remarks>
/// A caller that starts the program to look at a window has nothing to watch while it starts: what it needs
/// is the one thing it cannot work out for itself — that there is something on the screen now — and the
/// window saying so is the only statement of that. It is said on standard output, the stream a caller that
/// ran <c>rola desktop</c> reads, and it is said once.
/// <para>
/// Asked for rather than always said: <c>--ready</c> is the run that asked, and a run started by a person — a
/// double click, or <c>rola desktop</c> with nothing written — keeps the silence it had, because a line on a
/// terminal is a line nobody asked for.
/// </para>
/// <para>
/// This is the only thing the program writes to standard output. A caller that has what it was waiting for
/// is free to go, and the channel it was reading closes with it: a line written after that fails, which is
/// why the write is wrapped rather than allowed to end the window. Everything else the program has to say is
/// said on standard error, where a reader is not one that may leave.
/// </para>
/// </remarks>
internal static class ReadySignal
{
    /// <summary>The line the window says, which the caller that asked for it reads.</summary>
    /// <remarks>
    /// A word of its own rather than anything around it: the command line program passes the standard output
    /// through untouched, so what the caller reads is this program's line exactly. The launcher spells the same
    /// word, which is the one thing the two programs have to keep in step by hand.
    /// </remarks>
    public const string Line = "rola-desktop-ready";

    /// <summary>Whether the line has been said, so that a second window does not say it again.</summary>
    private static bool _said;

    /// <summary>Says that the window is up, when a caller asked to be told.</summary>
    public static void Send()
    {
        if (!Program.ReadyAsked || _said)
        {
            return;
        }

        _said = true;

        try
        {
            Console.Out.WriteLine(Line);
            Console.Out.Flush();
        }
        catch (IOException)
        {
            // The caller went away before the window was up: it is a caller that is no longer waiting for the
            // answer rather than a program that has failed, and the window it asked for is the whole of what
            // it wanted. Nothing else writes here, so there is nothing left to fail.
        }
    }
}
