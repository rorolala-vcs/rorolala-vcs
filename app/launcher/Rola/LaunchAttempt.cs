using System.Diagnostics;

namespace RorolalaLauncher.Rola;

/// <summary>
/// A start of the Desktop program that is being waited for: the process, and the reading of what it says.
/// </summary>
/// <remarks>
/// The process is kept rather than waited for and let go: what the wait is for is a window, and the program that
/// ends when the window does may outlive this one by hours. Nothing here ends it — a wait that gives up is this
/// window giving up rather than the other program being wrong — and <see cref="Dispose"/> is how this window
/// lets go of it, which is taking the handles rather than the program.
/// <para>
/// Both of the streams are drained while it runs. The output is where the line the window writes arrives; the
/// error is drained as well, so that a program saying a great deal on it cannot fill a pipe and block against
/// itself. Once this window is gone the other end of the error stream is nobody's to read, which is the Desktop
/// program's own to survive and it does: what it writes there after its window is up is nothing, and the line it
/// writes on the output is wrapped rather than allowed to end it.
/// </para>
/// </remarks>
internal sealed class LaunchAttempt : IDisposable
{
    /// <summary>The Desktop program's own start, as the machine knows it.</summary>
    private readonly Process _process;

    /// <summary>What the process has said about its window.</summary>
    private readonly Readiness _readiness = new();

    /// <summary>Makes the attempt over a process that is already going.</summary>
    /// <param name="process">The process, with both of its streams redirected.</param>
    public LaunchAttempt(Process process)
    {
        ArgumentNullException.ThrowIfNull(process);

        _process = process;

        _ = Read(_process.StandardOutput, _readiness);
        _ = Drain(_process.StandardError);

        _process.EnableRaisingEvents = true;

        _process.Exited += (_, _) =>
        {
            try
            {
                _readiness.Ended(_process.ExitCode);
            }
            catch (InvalidOperationException)
            {
                // This window let go of the process as it was ending: there is no longer a start to have an
                // answer about, and the window it did open is none of this attempt's business.
            }
        };

        // The end may already have happened between the start and the handler, and the toolkit raises no event
        // for a process that is already gone: what it says about itself is then read rather than waited for.
        if (_process.HasExited)
        {
            _readiness.Ended(_process.ExitCode);
        }
    }

    /// <summary>Waits for the window, or for as long as this window is willing to.</summary>
    /// <param name="timeout">How long to wait.</param>
    /// <returns>Whether there is a window, and what the program ended with when there is none.</returns>
    public Task<LaunchOutcome> WaitAsync(TimeSpan timeout) => _readiness.WaitAsync(timeout);

    /// <inheritdoc />
    public void Dispose() => _process.Dispose();

    /// <summary>Reads what the program writes where results go, a line at a time.</summary>
    /// <param name="output">What it writes.</param>
    /// <param name="readiness">What the lines are told to.</param>
    /// <returns>A task that ends when the stream does.</returns>
    private static async Task Read(StreamReader output, Readiness readiness)
    {
        try
        {
            while (await output.ReadLineAsync() is { } line)
            {
                readiness.Said(line);
            }
        }
        catch (Exception error) when (error is IOException or InvalidOperationException)
        {
            // The stream closed under the reading: either the process ended or this window let go of it, and
            // there is nothing left to read either way.
        }
    }

    /// <summary>Reads what the program says where errors go, and says nothing about it.</summary>
    /// <param name="error">What it says.</param>
    /// <returns>A task that ends when the stream does.</returns>
    private static async Task Drain(StreamReader error)
    {
        try
        {
            _ = await error.ReadToEndAsync();
        }
        catch (Exception reading) when (reading is IOException or InvalidOperationException)
        {
            // The same as above: nothing is read out of this stream, so what is left of it is left.
        }
    }
}
