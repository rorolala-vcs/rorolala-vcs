using System.Diagnostics.CodeAnalysis;
using System.IO.Pipes;
using System.Net.Sockets;
using System.Security.Cryptography;
using System.Text;
using RorolalaDesktop.Configuration;

namespace RorolalaDesktop;

/// <summary>
/// How a run of this program relates to the runs of it that are already going.
/// </summary>
/// <param name="IsFirst">
/// Whether this run is the one that owns the program: it is the first, and the only one there is.
/// </param>
/// <param name="Called">
/// Whether a run that was already going was asked to come forward, which is what happened instead of this
/// run opening a window of its own.
/// </param>
internal readonly record struct Start(bool IsFirst, bool Called);

/// <summary>
/// The one running copy of the program, and the way another start of it reaches that copy.
/// </summary>
/// <remarks>
/// There is one program per user, not one per start: a second start is a caller asking for the window rather
/// than a second window, so it hands over where it was made and ends. What it ends with says which of the two
/// happened — it was the first and the program is now its own, or it was a call and the program that was
/// already up has been brought forward — because a caller that waits for a program is entitled to its answer.
/// <para>
/// Two things are needed for that, and they are told apart because they do different work: a mutex says
/// whether a run is already going, which is the question that cannot be raced, and a channel carries what the
/// second run has to say — where it was made — to the first. What is carried is a directory and nothing else,
/// because a directory is the whole of what a window opened there is about.
/// </para>
/// <para>
/// The channel itself is the platform's: a named pipe on Windows, and a socket with a path on a Unix, which
/// are two different kinds of thing rather than one thing asked for two ways. What the two have in common is
/// only what this file uses — something to wait on, and a stream of bytes to read — and that is all the
/// abstraction here is.
/// </para>
/// </remarks>
internal static class SingleInstance
{
    /// <summary>How long a caller waits for the program that is going to answer.</summary>
    private static readonly TimeSpan Wait = TimeSpan.FromSeconds(5);

    /// <summary>The name a run holds while it is the one running, as the machine knows it.</summary>
    private static readonly string Name = $"rorolala-desktop-{Fingerprint()}";

    /// <summary>
    /// The mutex this run holds for as long as it is the one running.
    /// </summary>
    /// <remarks>
    /// Held rather than merely checked: what makes a run the one running is holding this, and a run that let
    /// go of it would leave the next start thinking it was the first. Let go of only when the program ends,
    /// which is what the operating system does with it either way.
    /// </remarks>
    private static Mutex? _held;

    /// <summary>
    /// Whether this run is the first, and whether it reached one that was already going.
    /// </summary>
    /// <param name="directory">Where the run that is calling was made, or nothing.</param>
    /// <returns>Which of the two happened.</returns>
    public static Start Acquire(string? directory)
    {
        var mine = new Mutex(initiallyOwned: false, Name, out var first);

        if (first)
        {
            // Kept for the life of the process rather than disposed at the end of this scope: what it means
            // is that this program is running, and it stops meaning that when the program stops.
            _held = mine;

            return new Start(IsFirst: true, Called: false);
        }

        // UNWRAP: nothing here takes the mutex, so nothing here can be interrupted while holding it.
        mine.Dispose();

        return new Start(IsFirst: false, Called: Call(directory));
    }

    /// <summary>
    /// Listens for a later start of the program calling this one, for as long as the program runs.
    /// </summary>
    /// <remarks>
    /// A thread of its own, because waiting for a caller is waiting for something that may never happen: the
    /// window's own thread is where the program is drawn, and a program that waited there would be a program
    /// that stopped drawing. What arrives is handed to the window's thread, since what it is answered with is
    /// a window being brought forward.
    /// </remarks>
    /// <param name="answer">What to do for a caller, with the directory it was made in.</param>
    /// <param name="safer">Where a caller that could not be read is reported.</param>
    public static void Listen(Action<string?> answer, Logging.LogService safer)
    {
        ArgumentNullException.ThrowIfNull(answer);
        ArgumentNullException.ThrowIfNull(safer);

        var listening = new Thread(() =>
        {
            while (true)
            {
                try
                {
                    using var caller = Await();

                    if (Read(caller) is not { } said)
                    {
                        continue;
                    }

                    // Answered on the window's thread, because what a caller is answered with is a window
                    // being opened or brought forward, and only the thread that draws may touch one.
                    Avalonia.Threading.Dispatcher.UIThread.Post(() =>
                        answer(said.Length == 0 ? null : said)
                    );
                }
                catch (Exception error)
                    when (error
                        is IOException
                            or UnauthorizedAccessException
                            or ObjectDisposedException
                            or SocketException
                            or OperationCanceledException
                    )
                {
                    // A caller that went away between connecting and saying anything, or one that could not be
                    // read on the way, is not a reason to stop listening: the next one is a caller like any
                    // other and the program is still running. What happened is written down rather than
                    // swallowed, because a program that can no longer be reached from a second start is a
                    // program whose one way in has gone quiet.
                    safer.Record(Logging.LogLevel.Debug, Hosting.Core.Source, $"a call was not answered: {error.Message}");
                }
            }
        })
        {
            IsBackground = true,
            Name = "desktop-calls",
        };

        listening.Start();
    }

    /// <summary>Waits for a caller, and answers the stream it may be read from.</summary>
    /// <returns>What the caller said can be read from.</returns>
    // The listening socket outlives this method in a way the analysis cannot follow: what is returned is a
    // stream over the connection it accepts, and the socket is held by nothing else once it has accepted one.
    [SuppressMessage(
        "Reliability",
        "CA2000:Dispose objects before losing scope",
        Justification = "the listening socket is bound for as long as the program runs, and the stream over the accepted connection is what the caller reads it through"
    )]
    private static Stream Await()
    {
        if (OperatingSystem.IsWindows())
        {
            var pipe = new NamedPipeServerStream(
                Name,
                PipeDirection.In,
                // One at a time: what a caller says is a directory, and two directories arriving at once
                // would be two windows asked for by two runs that are the same answer to the same question.
                maxNumberOfServerInstances: 1,
                PipeTransmissionMode.Byte,
                PipeOptions.None
            );

            pipe.WaitForConnection();

            return pipe;
        }

        var path = SocketPath();

        // A socket left behind by a run that ended badly is one that nothing is listening on, and a name
        // that cannot be listened on is a program no second start can reach. Removed before binding rather
        // than after, because at this point this run is the one running: the name is its own.
        File.Delete(path);

        var socket = Socket(AddressFamily.Unix);

        socket.Bind(new UnixDomainSocketEndPoint(path));
        socket.Listen(1);

        // The accepted connection is the stream the caller is read through, and it owns the socket it came
        // from, so the one that is listening is the program's for as long as the program listens.
        return new NetworkStream(socket.Accept(), ownsSocket: true);
    }

    /// <summary>Reads what a caller said, or nothing when it said nothing and went away.</summary>
    /// <param name="caller">What the caller said can be read from.</param>
    /// <returns>The line it said, or nothing.</returns>
    // The reader is left to be collected rather than disposed: what it wraps is the connection the caller
    // is heard through, which is disposed by the one that made it, and disposing the reader first would say
    // the same thing twice.
    [SuppressMessage(
        "Reliability",
        "CA2000:Dispose objects before losing scope",
        Justification = "the stream under the reader is the caller's and is disposed by the one that opened it"
    )]
    private static string? Read(Stream caller) =>
        new StreamReader(caller, Encoding.UTF8).ReadLine();

    /// <summary>Asks the run that is already going to come forward, and says whether it answered.</summary>
    /// <remarks>
    /// Tried more than once, because the run that holds the name may still be starting: the mutex is taken
    /// before Avalonia is started and the channel is listened on after, so there is a moment in which the
    /// program is the one running and cannot yet be reached. What is waited for is that moment, and not a
    /// program that is going away — one that is going away is not reached by waiting either.
    /// </remarks>
    /// <param name="directory">Where this run was made, or nothing.</param>
    /// <returns>Whether the run that is going was reached.</returns>
    private static bool Call(string? directory)
    {
        foreach (var pause in new[] { 0, 100, 250, 500, 1000 })
        {
            if (pause > 0)
            {
                Thread.Sleep(pause);
            }

            try
            {
                using var client = Reach();
                using var writer = new StreamWriter(client, Encoding.UTF8) { AutoFlush = true };

                writer.WriteLine(directory ?? string.Empty);

                return true;
            }
            catch (Exception error)
                when (error
                    is IOException
                        or UnauthorizedAccessException
                        or TimeoutException
                        or ObjectDisposedException
                        or SocketException
                )
            {
                // A run that holds the name and cannot be reached yet is one to try again: see the remarks
                // above, which is why this is a loop rather than one attempt.
            }
        }

        // Every attempt was refused, so the run that holds the name is one that is going away: this start
        // is then a start rather than a call, and the program it could not reach is not one to wait for.
        return false;
    }

    /// <summary>Reaches the program that is going.</summary>
    /// <returns>The stream it may be written to.</returns>
    // The socket is handed to the stream, which owns it: the stream is what the caller writes through and
    // what closes it, and the socket is not a second thing to let go of beside it.
    [SuppressMessage(
        "Reliability",
        "CA2000:Dispose objects before losing scope",
        Justification = "the stream takes the socket over, so the one the caller holds is what closes both"
    )]
    private static Stream Reach()
    {
        if (OperatingSystem.IsWindows())
        {
            var pipe = new NamedPipeClientStream(".", Name, PipeDirection.Out, PipeOptions.None);

            pipe.Connect((int)Wait.TotalMilliseconds);

            return pipe;
        }

        var socket = Socket(AddressFamily.Unix);

        socket.Connect(new UnixDomainSocketEndPoint(SocketPath()));

        return new NetworkStream(socket, ownsSocket: true);
    }

    /// <summary>A socket of one address family.</summary>
    /// <param name="family">The family, which is the platform's own for a channel between two runs.</param>
    /// <returns>The socket.</returns>
    private static Socket Socket(AddressFamily family) =>
        new(family, SocketType.Stream, ProtocolType.Unspecified);

    /// <summary>Where this program's socket sits, which is beside its own files.</summary>
    /// <returns>The path the socket is bound to.</returns>
    private static string SocketPath()
    {
        Directory.CreateDirectory(ConfigPaths.Root);

        return Path.Combine(ConfigPaths.Root, "instance.sock");
    }

    /// <summary>
    /// A name that is this user's own, so that two people on one machine have one program each.
    /// </summary>
    /// <remarks>
    /// Taken from where the program keeps its files, which is the one path that is already this user's and
    /// this program's: a name the program picked itself would either be the same for everybody or have to be
    /// written somewhere, and this is a thing that is already written somewhere.
    /// </remarks>
    /// <returns>The name, as a machine's channel name may be written.</returns>
    private static string Fingerprint()
    {
        var digest = SHA256.HashData(Encoding.UTF8.GetBytes(ConfigPaths.Root));

        return Convert.ToHexString(digest)[..16].ToLowerInvariant();
    }
}
