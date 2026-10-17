using System.Runtime.InteropServices;
using System.Text;

namespace RolaSharp;

/// <summary>
/// A terminal, as the Desktop starts and drives one.
/// </summary>
/// <remarks>
/// It is the other half of a dock that draws one: <c>rola-desktop-pty</c> runs the program in a
/// pseudo-terminal, and this is what a view reads its output from, writes a key to, and resizes. What
/// crosses is bytes and nothing else — what the bytes mean is a parser's business, and that is the
/// window's — so this stays a handle and four calls.
/// <para>
/// Reading is a poll rather than a callback: the native side drains the terminal on a thread of its
/// own, and <see cref="Read"/> answers what has arrived since the last ask. Zero is "nothing yet" and
/// a negative answer is the program having ended.
/// </para>
/// </remarks>
public sealed class RolaDesktopPty : IDisposable
{
    /// <summary>How many bytes one poll is asked for at most.</summary>
    private const int Chunk = 16 * 1024;

    /// <summary>The native handle, until it is given back.</summary>
    private nint _handle;

    /// <summary>Where a read is taken before it is copied out, so that polling allocates nothing.</summary>
    private nint _buffer;

    /// <summary>Makes a session over a handle.</summary>
    /// <param name="handle">The native handle, which must not be null.</param>
    private RolaDesktopPty(nint handle)
    {
        _handle = handle;
        _buffer = Marshal.AllocHGlobal(Chunk);
    }

    /// <summary>Starts a program in a terminal of its own.</summary>
    /// <remarks>
    /// The arguments cross as the NUL-separated list the native side reads, which is what lets an
    /// argument hold a space or a quote without either side having to agree about quoting.
    /// </remarks>
    /// <param name="program">The program to run, as the system names it.</param>
    /// <param name="arguments">Its arguments, in order.</param>
    /// <param name="directory">Where it is run.</param>
    /// <param name="cols">How many columns the view is.</param>
    /// <param name="rows">How many rows the view is.</param>
    /// <returns>The session, or nothing when the program could not be started.</returns>
    public static RolaDesktopPty? Open(
        string program,
        IReadOnlyList<string> arguments,
        string directory,
        int cols,
        int rows
    )
    {
        ArgumentNullException.ThrowIfNull(program);
        ArgumentNullException.ThrowIfNull(arguments);
        ArgumentNullException.ThrowIfNull(directory);

        if (program.Length == 0)
        {
            return null;
        }

        var name = Marshal.StringToCoTaskMemUTF8(program);
        var at = Marshal.StringToCoTaskMemUTF8(directory);
        var list = Listed(arguments);

        try
        {
            var handle = RorolalaBinding.rola_desktop_pty_open(
                name,
                list,
                at,
                (ushort)Math.Max(1, Math.Min(cols, ushort.MaxValue)),
                (ushort)Math.Max(1, Math.Min(rows, ushort.MaxValue))
            );

            return handle == 0 ? null : new RolaDesktopPty(handle);
        }
        finally
        {
            Marshal.FreeCoTaskMem(list);
            Marshal.FreeCoTaskMem(at);
            Marshal.FreeCoTaskMem(name);
        }
    }

    /// <summary>Whether the handle is still open, which is what a second ask about it needs to know.</summary>
    public bool Running => _handle != 0;

    /// <summary>
    /// Takes what the program has printed since the last ask.
    /// </summary>
    /// <param name="into">Where the bytes go, up to its length.</param>
    /// <returns>
    /// How many bytes were written, zero when nothing has arrived yet, and a negative number when the
    /// program has ended.
    /// </returns>
    public int Read(byte[] into)
    {
        ArgumentNullException.ThrowIfNull(into);

        if (_handle == 0 || into.Length == 0)
        {
            return 0;
        }

        var read = RorolalaBinding.rola_desktop_pty_read(
            _handle,
            _buffer,
            (nuint)Math.Min(Chunk, into.Length)
        );

        if (read <= 0)
        {
            return (int)read;
        }

        Marshal.Copy(_buffer, into, 0, (int)read);

        return (int)read;
    }

    /// <summary>Sends bytes to the program, which is what a key or a pasted line is.</summary>
    /// <param name="data">The bytes, already encoded the way a terminal sends them.</param>
    /// <returns>Whether they were written.</returns>
    public bool Write(byte[] data)
    {
        ArgumentNullException.ThrowIfNull(data);

        if (_handle == 0 || data.Length == 0)
        {
            return false;
        }

        var bytes = Marshal.AllocHGlobal(data.Length);

        try
        {
            Marshal.Copy(data, 0, bytes, data.Length);

            return RorolalaBinding.rola_desktop_pty_write(_handle, bytes, (nuint)data.Length);
        }
        finally
        {
            Marshal.FreeHGlobal(bytes);
        }
    }

    /// <summary>Says how large the view is, which the program lays its screen out by.</summary>
    /// <param name="cols">How many columns.</param>
    /// <param name="rows">How many rows.</param>
    /// <returns>Whether it was said.</returns>
    public bool Resize(int cols, int rows)
    {
        if (_handle == 0)
        {
            return false;
        }

        return RorolalaBinding.rola_desktop_pty_resize(
            _handle,
            (ushort)Math.Max(1, Math.Min(cols, ushort.MaxValue)),
            (ushort)Math.Max(1, Math.Min(rows, ushort.MaxValue))
        );
    }

    /// <summary>Ends the program when the session is let go of without being closed.</summary>
    ~RolaDesktopPty() => Dispose();

    /// <summary>Ends the program and gives the terminal back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.rola_desktop_pty_close(_handle);
            _handle = 0;
        }

        if (_buffer != 0)
        {
            Marshal.FreeHGlobal(_buffer);
            _buffer = 0;
        }

        GC.SuppressFinalize(this);
    }

    /// <summary>The arguments as the NUL-separated list the native side reads, owned by the caller.</summary>
    /// <param name="arguments">The arguments, in order.</param>
    /// <returns>Where the list is, to be released with <see cref="Marshal.FreeCoTaskMem(IntPtr)"/>.</returns>
    private static nint Listed(IReadOnlyList<string> arguments)
    {
        var bytes = Encoding.UTF8.GetBytes(
            string.Join('\0', arguments) + (arguments.Count > 0 ? "\0\0" : "\0")
        );

        var list = Marshal.AllocCoTaskMem(bytes.Length);

        Marshal.Copy(bytes, 0, list, bytes.Length);

        return list;
    }
}
