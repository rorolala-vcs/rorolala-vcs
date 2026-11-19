namespace RolaSharp.Write;

/// <summary>A flag a caller sets to stop a write at its next step.</summary>
public sealed class Cancellation : IDisposable
{
    private nint _handle;

    private Cancellation(nint handle) => _handle = handle;

    /// <summary>The native cancellation, for the reads that take one.</summary>
    internal nint Handle
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return _handle;
        }
    }

    /// <summary>Makes a cancellation nobody has set.</summary>
    /// <returns>The handle, which the caller releases.</returns>
    public static Cancellation New()
    {
        var handle = RorolalaBinding.rola_cancellation_new();
        if (handle == 0)
        {
            throw new RolaWriteError(
                RolaWriteErrorKind.Internal,
                null,
                "the native cancellation could not be made"
            );
        }

        return new Cancellation(handle);
    }

    /// <summary>Asks the write to stop.</summary>
    public void Cancel()
    {
        ObjectDisposedException.ThrowIf(_handle == 0, this);
        RorolalaBinding.rola_cancellation_cancel(_handle);
    }

    /// <summary>Whether the write has been asked to stop.</summary>
    public bool IsCancelled
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return RorolalaBinding.rola_cancellation_is_cancelled(_handle);
        }
    }

    /// <summary>Gives the native cancellation back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_cancellation(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }
}
