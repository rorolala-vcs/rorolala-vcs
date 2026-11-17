using System.Runtime.InteropServices;

namespace RolaSharp.V2;

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
}
