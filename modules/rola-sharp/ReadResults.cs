using System.Runtime.InteropServices;

namespace RolaSharp;

/// <summary>Maps the native error payloads onto the read failure a caller catches.</summary>
internal static class ReadResults
{
    /// <summary>The failure a storage read carried.</summary>
    /// <param name="payload">The native error, which this takes ownership of.</param>
    /// <returns>The failure.</returns>
    public static RolaReadError Storage(nint payload)
    {
        var error = Marshal.PtrToStructure<RolaStorageError>(payload);
        var reason = error.tag switch
        {
            RolaStorageErrorTag.RolaStorageError_Read => NativeText.Taken(error.payload.Read.reason),
            _ => string.Empty,
        };
        var kind = error.tag switch
        {
            RolaStorageErrorTag.RolaStorageError_NoStore => RolaReadErrorKind.NoVault,
            RolaStorageErrorTag.RolaStorageError_NotFound => RolaReadErrorKind.NotFound,
            RolaStorageErrorTag.RolaStorageError_Malformed => RolaReadErrorKind.Marshalling,
            RolaStorageErrorTag.RolaStorageError_Exists => RolaReadErrorKind.InvalidArgument,
            _ => RolaReadErrorKind.Unreadable,
        };

        RorolalaBinding.free_rola_storage_error(payload);

        return new RolaReadError(kind, null, reason);
    }

    /// <summary>The failure a Layout read carried.</summary>
    /// <param name="payload">The native error, which this takes ownership of.</param>
    /// <returns>The failure.</returns>
    public static RolaReadError Layout(nint payload)
    {
        var error = Marshal.PtrToStructure<RolaLayoutError>(payload);
        var reason =
            error.tag is RolaLayoutErrorTag.RolaLayoutError_Read
                ? NativeText.Taken(error.payload.Read.reason)
                : string.Empty;
        var kind = error.tag switch
        {
            RolaLayoutErrorTag.RolaLayoutError_NoWorkspace => RolaReadErrorKind.NoWorkspace,
            RolaLayoutErrorTag.RolaLayoutError_NoLayout => RolaReadErrorKind.NoLayout,
            RolaLayoutErrorTag.RolaLayoutError_NotCached => RolaReadErrorKind.NotCached,
            RolaLayoutErrorTag.RolaLayoutError_Missing => RolaReadErrorKind.Missing,
            _ => RolaReadErrorKind.Unreadable,
        };

        RorolalaBinding.free_rola_layout_error(payload);

        return new RolaReadError(kind, null, reason);
    }

    /// <summary>The failure a VCS index read carried.</summary>
    /// <param name="payload">The native error, which this takes ownership of.</param>
    /// <returns>The failure.</returns>
    public static RolaReadError VcsIndex(nint payload)
    {
        var error = Marshal.PtrToStructure<RolaVCSIndexReadingError>(payload);
        var kind = error.tag switch
        {
            RolaVCSIndexReadingErrorTag.RolaVCSIndexReadingError_NotFound =>
                RolaReadErrorKind.NotFound,
            RolaVCSIndexReadingErrorTag.RolaVCSIndexReadingError_Malformed =>
                RolaReadErrorKind.Marshalling,
            RolaVCSIndexReadingErrorTag.RolaVCSIndexReadingError_Corrupt =>
                RolaReadErrorKind.Unreadable,
            _ => RolaReadErrorKind.Unreadable,
        };
        var reason =
            error.tag is RolaVCSIndexReadingErrorTag.RolaVCSIndexReadingError_Io
                ? NativeText.Taken(error.payload.Io)
                : string.Empty;

        RorolalaBinding.free_rola_vcs_index_reading_error(payload);

        return new RolaReadError(kind, null, reason);
    }

    /// <summary>The failure an inverse-index read carried.</summary>
    /// <param name="payload">The native error, which this takes ownership of.</param>
    /// <returns>The failure.</returns>
    public static RolaReadError Inverse(nint payload)
    {
        var error = Marshal.PtrToStructure<RolaInverseIndexReadingError>(payload);
        var kind = error.tag switch
        {
            RolaInverseIndexReadingErrorTag.RolaInverseIndexReadingError_NotFound =>
                RolaReadErrorKind.NotFound,
            RolaInverseIndexReadingErrorTag.RolaInverseIndexReadingError_Malformed =>
                RolaReadErrorKind.Marshalling,
            _ => RolaReadErrorKind.Unreadable,
        };
        var reason =
            error.tag is RolaInverseIndexReadingErrorTag.RolaInverseIndexReadingError_Read
                ? NativeText.Taken(error.payload.Read.cause)
                : string.Empty;

        RorolalaBinding.free_rola_inverse_index_reading_error(payload);

        return new RolaReadError(kind, null, reason);
    }

    /// <summary>The failure building an inverse index carried.</summary>
    /// <param name="payload">The native error, which this takes ownership of.</param>
    /// <returns>The failure.</returns>
    public static RolaReadError InverseBuild(nint payload)
    {
        var error = Marshal.PtrToStructure<RolaInverseIndexError>(payload);
        var reason =
            error.tag is RolaInverseIndexErrorTag.RolaInverseIndexError_Io
                ? NativeText.Taken(error.payload.Io)
                : string.Empty;

        RorolalaBinding.free_rola_inverse_index_error(payload);

        return new RolaReadError(RolaReadErrorKind.Unreadable, null, reason);
    }

    /// <summary>The failure a book read carried.</summary>
    /// <param name="payload">The native error, which this takes ownership of.</param>
    /// <returns>The failure.</returns>
    public static RolaReadError Book(nint payload)
    {
        var error = Marshal.PtrToStructure<RolaBookError>(payload);
        var reason = error.tag switch
        {
            RolaBookErrorTag.RolaBookError_Malformed => NativeText.Taken(
                error.payload.Malformed.detail
            ),
            RolaBookErrorTag.RolaBookError_Duplicate => NativeText.Taken(
                error.payload.Duplicate.name
            ),
            _ => string.Empty,
        };

        RorolalaBinding.free_rola_book_error(payload);

        return new RolaReadError(RolaReadErrorKind.Unreadable, null, reason);
    }
}
