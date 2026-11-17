using System.Runtime.InteropServices;

namespace RolaSharp.V2;

/// <summary>A terminal program the Desktop drives: its input, output and size.</summary>
public sealed class Pty : IDisposable
{
    private nint _handle;

    private Pty(nint handle) => _handle = handle;

    /// <summary>Starts a program in a terminal of the size given.</summary>
    /// <param name="program">The program to start.</param>
    /// <param name="arguments">Its arguments, in the shape a shell would read.</param>
    /// <param name="directory">Where it starts.</param>
    /// <param name="columns">How many columns the terminal has.</param>
    /// <param name="rows">How many rows it has.</param>
    /// <returns>The terminal, or nothing when it could not be started.</returns>
    public static Pty? Open(
        string program,
        string arguments,
        string directory,
        ushort columns,
        ushort rows
    )
    {
        ArgumentNullException.ThrowIfNull(program);
        ArgumentNullException.ThrowIfNull(arguments);
        ArgumentNullException.ThrowIfNull(directory);

        var programText = Utf8(program);
        var argumentsText = Utf8(arguments);
        var directoryText = Utf8(directory);
        try
        {
            var handle = RorolalaBinding.rola_desktop_pty_open(
                programText,
                argumentsText,
                directoryText,
                columns,
                rows
            );

            return handle == 0 ? null : new Pty(handle);
        }
        finally
        {
            Marshal.FreeCoTaskMem(programText);
            Marshal.FreeCoTaskMem(argumentsText);
            Marshal.FreeCoTaskMem(directoryText);
        }
    }

    /// <summary>Reads what the program has written.</summary>
    /// <param name="buffer">Where to put the bytes.</param>
    /// <returns>How many bytes were read, or a negative number when nothing is waiting.</returns>
    public int Read(Span<byte> buffer)
    {
        ObjectDisposedException.ThrowIf(_handle == 0, this);
        if (buffer.Length == 0)
        {
            return 0;
        }

        var temporary = Marshal.AllocHGlobal(buffer.Length);
        try
        {
            var read = (int)
                RorolalaBinding.rola_desktop_pty_read(
                    _handle,
                    temporary,
                    (nuint)buffer.Length
                );
            if (read > 0)
            {
                var managed = new byte[read];
                Marshal.Copy(temporary, managed, 0, read);
                managed.AsSpan().CopyTo(buffer);
            }

            return read;
        }
        finally
        {
            Marshal.FreeHGlobal(temporary);
        }
    }

    /// <summary>Writes to the program's input.</summary>
    /// <param name="data">The bytes to write.</param>
    /// <returns>Whether they were written.</returns>
    public bool Write(ReadOnlySpan<byte> data)
    {
        ObjectDisposedException.ThrowIf(_handle == 0, this);
        if (data.Length == 0)
        {
            return true;
        }

        var temporary = Marshal.AllocHGlobal(data.Length);
        try
        {
            var managed = data.ToArray();
            Marshal.Copy(managed, 0, temporary, managed.Length);

            return RorolalaBinding.rola_desktop_pty_write(
                _handle,
                temporary,
                (nuint)data.Length
            );
        }
        finally
        {
            Marshal.FreeHGlobal(temporary);
        }
    }

    /// <summary>Tells the program its terminal has a new size.</summary>
    /// <param name="columns">How many columns it has.</param>
    /// <param name="rows">How many rows it has.</param>
    /// <returns>Whether the size was taken.</returns>
    public bool Resize(ushort columns, ushort rows)
    {
        ObjectDisposedException.ThrowIf(_handle == 0, this);

        return RorolalaBinding.rola_desktop_pty_resize(_handle, columns, rows);
    }

    /// <summary>Ends the terminal.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.rola_desktop_pty_close(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}
