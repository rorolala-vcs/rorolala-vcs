using System.Runtime.InteropServices;
using System.Text.Json;

using RolaSharp;

namespace RolaSharp.Write;

/// <summary>What a write has said about itself, waiting to be read.</summary>
public sealed class Progress : IDisposable
{
    private nint _handle;

    private Progress(nint handle) => _handle = handle;

    /// <summary>The native progress, for the writes that take one.</summary>
    internal nint Handle
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return _handle;
        }
    }

    /// <summary>Makes a progress handle with nothing said yet.</summary>
    /// <returns>The handle, which the caller releases.</returns>
    public static Progress New()
    {
        var handle = RorolalaBinding.rola_progress_new();
        if (handle == 0)
        {
            throw new RolaWriteError(
                RolaWriteErrorKind.Internal,
                null,
                "the native progress could not be made"
            );
        }

        return new Progress(handle);
    }

    /// <summary>The next event, as the record form the desktop's progress stack reads.</summary>
    /// <remarks>
    /// The ABI hands the event over as the tagged union a write says, and this turns it into the one
    /// record form the stack already reads, so a caller that draws a bar is not handed a second
    /// vocabulary for it.
    /// </remarks>
    /// <returns>The event, or nothing when the write has not said anything new.</returns>
    public string? NextJson()
    {
        ObjectDisposedException.ThrowIf(_handle == 0, this);

        var said = RorolalaBinding.rola_progress_next(_handle);
        if (said == 0)
        {
            return null;
        }

        try
        {
            return Record(said);
        }
        finally
        {
            RorolalaBinding.free_rola_progress_event(said);
        }
    }

    /// <summary>Gives the native progress back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_progress(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }

    private static string? Record(nint said)
    {
        var signal = RorolalaBinding.rola_progress_event_signal(said);

        switch (signal.tag)
        {
            case RolaProgressSignalTag.RolaProgressSignal_Begin:
                var begin = signal.payload.Begin;
                var what = NativeText.Taken(begin.what);
                var direction = NativeText.Taken(begin.direction);

                return JsonSerializer.Serialize(
                    new
                    {
                        signal = "begin",
                        id = begin.id,
                        parent = begin.parent_present != 0 ? begin.parent : (ulong?)null,
                        what,
                        direction = direction.Length == 0 ? null : direction,
                        total = begin.total_present != 0 ? begin.total : (ulong?)null,
                    }
                );

            case RolaProgressSignalTag.RolaProgressSignal_Advance:
                var advance = signal.payload.Advance;

                return JsonSerializer.Serialize(
                    new
                    {
                        signal = "advance",
                        id = advance.id,
                        done = advance.done,
                    }
                );

            case RolaProgressSignalTag.RolaProgressSignal_Finish:
                var finish = signal.payload.Finish;

                return JsonSerializer.Serialize(new { signal = "finish", id = finish.id });

            default:
                return null;
        }
    }
}
