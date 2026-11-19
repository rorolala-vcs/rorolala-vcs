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

    /// <summary>The next event as JSON, or nothing when the write has not said anything new.</summary>
    /// <returns>The event, or nothing.</returns>
    public string? NextJson()
    {
        ObjectDisposedException.ThrowIf(_handle == 0, this);

        var json = NativeText.Taken(RorolalaBinding.rola_progress_next_json(_handle));

        return json is "" or "null" ? null : json;
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
}
