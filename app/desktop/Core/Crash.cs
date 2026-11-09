using System.Globalization;

namespace RorolalaDesktop;

/// <summary>
/// The last report a run makes when it cannot go on.
/// </summary>
/// <remarks>
/// A window has no terminal of its own, and the program that starts it can only read what it writes.
/// What is written here is therefore both streams at once: the error stream for the host or the
/// terminal that is reading it, and a file for the reader who arrives after the pipe is gone. The
/// report is the exception's own text, which is its stack as well as its message.
/// <para>
/// The report is made once. An exception raised while another is being reported, which is what a
/// handler that itself fails looks like, must not replace the first report with its own.
/// </para>
/// </remarks>
internal static class Crash
{
    /// <summary>The code a crashed run ends with, which is not one of the refusals to start.</summary>
    public const int Code = 70;

    /// <summary>Whether a report has already been made.</summary>
    private static int _reported;

    /// <summary>Starts reporting what the runtime does not catch.</summary>
    public static void Install() =>
        AppDomain.CurrentDomain.UnhandledException += (_, failure) =>
            Report(failure.ExceptionObject as Exception);

    /// <summary>Writes the report, and ends the run with the crash code.</summary>
    /// <param name="failure">What went wrong, or nothing when the runtime did not say.</param>
    public static void Report(Exception? failure)
    {
        if (Interlocked.Exchange(ref _reported, 1) != 0)
        {
            return;
        }

        var report = failure?.ToString() ?? "The program ended with an exception it could not name.";

        try
        {
            Console.Error.WriteLine(report);
            Console.Error.Flush();
        }
        catch (Exception error) when (error is IOException or ObjectDisposedException)
        {
            // The stream is the host's to read; the file below is the report that outlives it.
        }

        try
        {
            var at = Path.Combine(
                Path.GetTempPath(),
                string.Create(
                    CultureInfo.InvariantCulture,
                    $"rorolala-desktop-crash-{Environment.ProcessId}.log"
                )
            );

            File.WriteAllText(at, report);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            // A report that cannot be filed is still the one on the error stream.
        }

        Environment.Exit(Code);
    }
}
