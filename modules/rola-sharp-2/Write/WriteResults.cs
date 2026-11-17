using System.Runtime.InteropServices;

using RolaSharp.V2;
using NativeWriteError = RolaSharp.V2.RolaWriteError;
using NativeWriteTag = RolaSharp.V2.RolaWriteErrorTag;

namespace RolaSharp.V2.Write;

/// <summary>Maps the native write failure onto the exception a caller catches.</summary>
internal static class WriteResults
{
    /// <summary>The failure a write carried.</summary>
    /// <param name="payload">The native error, which this takes ownership of.</param>
    /// <returns>The failure.</returns>
    public static RolaWriteError From(nint payload)
    {
        var error = Marshal.PtrToStructure<NativeWriteError>(payload);
        var (path, reason) = Read(error);
        var kind = error.tag switch
        {
            NativeWriteTag.RolaWriteError_NoWorkspace => RolaWriteErrorKind.NoWorkspace,
            NativeWriteTag.RolaWriteError_NoLayout => RolaWriteErrorKind.NoLayout,
            NativeWriteTag.RolaWriteError_NoStore => RolaWriteErrorKind.NoStore,
            NativeWriteTag.RolaWriteError_NoIndex => RolaWriteErrorKind.NoIndex,
            NativeWriteTag.RolaWriteError_NoAccount => RolaWriteErrorKind.NoAccount,
            NativeWriteTag.RolaWriteError_NoTrack => RolaWriteErrorKind.NoTrack,
            NativeWriteTag.RolaWriteError_NotCached => RolaWriteErrorKind.NotCached,
            NativeWriteTag.RolaWriteError_Offline => RolaWriteErrorKind.Offline,
            NativeWriteTag.RolaWriteError_Locked => RolaWriteErrorKind.Locked,
            NativeWriteTag.RolaWriteError_InvalidPath => RolaWriteErrorKind.InvalidPath,
            NativeWriteTag.RolaWriteError_NotFound => RolaWriteErrorKind.NotFound,
            NativeWriteTag.RolaWriteError_Precondition => RolaWriteErrorKind.Precondition,
            NativeWriteTag.RolaWriteError_Ownership => RolaWriteErrorKind.Ownership,
            NativeWriteTag.RolaWriteError_Cancelled => RolaWriteErrorKind.Cancelled,
            NativeWriteTag.RolaWriteError_Action => RolaWriteErrorKind.Action,
            NativeWriteTag.RolaWriteError_Layout => RolaWriteErrorKind.Layout,
            NativeWriteTag.RolaWriteError_Storage => RolaWriteErrorKind.Storage,
            NativeWriteTag.RolaWriteError_Index => RolaWriteErrorKind.Index,
            NativeWriteTag.RolaWriteError_Io => RolaWriteErrorKind.Io,
            NativeWriteTag.RolaWriteError_Json => RolaWriteErrorKind.Json,
            _ => RolaWriteErrorKind.Internal,
        };

        RorolalaBinding.free_rola_write_error(payload);

        return new RolaWriteError(kind, path, reason);
    }

    /// <summary>The failure a storage operation carried.</summary>
    /// <param name="payload">The native error, which this takes ownership of.</param>
    /// <returns>The failure.</returns>
    public static RolaWriteError FromStorage(nint payload)
    {
        var error = Marshal.PtrToStructure<RolaSharp.V2.RolaStorageError>(payload);
        var kind = error.tag switch
        {
            RolaSharp.V2.RolaStorageErrorTag.RolaStorageError_NoStore => RolaWriteErrorKind.NoStore,
            RolaSharp.V2.RolaStorageErrorTag.RolaStorageError_NotFound => RolaWriteErrorKind.NotFound,
            RolaSharp.V2.RolaStorageErrorTag.RolaStorageError_Malformed => RolaWriteErrorKind.Json,
            RolaSharp.V2.RolaStorageErrorTag.RolaStorageError_Exists => RolaWriteErrorKind.InvalidPath,
            _ => RolaWriteErrorKind.Storage,
        };
        var reason =
            error.tag is RolaSharp.V2.RolaStorageErrorTag.RolaStorageError_Read
                ? NativeText.Taken(error.payload.Read.reason)
                : string.Empty;

        RorolalaBinding.free_rola_storage_error(payload);

        return new RolaWriteError(kind, null, reason);
    }

    private static (string? Path, string Reason) Read(NativeWriteError error) =>
        error.tag switch
        {
            NativeWriteTag.RolaWriteError_Locked => (
                NativeText.Taken(error.payload.Locked.path),
                string.Empty
            ),
            NativeWriteTag.RolaWriteError_InvalidPath => (
                NativeText.Taken(error.payload.InvalidPath.path),
                NativeText.Taken(error.payload.InvalidPath.reason)
            ),
            NativeWriteTag.RolaWriteError_NotFound => (
                null,
                NativeText.Taken(error.payload.NotFound.what)
            ),
            NativeWriteTag.RolaWriteError_Precondition => (
                null,
                $"{NativeText.Taken(error.payload.Precondition.what)}: {NativeText.Taken(error.payload.Precondition.reason)}"
            ),
            NativeWriteTag.RolaWriteError_Ownership => (
                NativeText.Taken(error.payload.Ownership.path),
                $"{NativeText.Taken(error.payload.Ownership.what)}: {NativeText.Taken(error.payload.Ownership.reason)}"
            ),
            NativeWriteTag.RolaWriteError_Action => (
                null,
                NativeText.Taken(error.payload.Action.reason)
            ),
            NativeWriteTag.RolaWriteError_Layout => (
                null,
                NativeText.Taken(error.payload.Layout.reason)
            ),
            NativeWriteTag.RolaWriteError_Storage => (
                null,
                NativeText.Taken(error.payload.Storage.reason)
            ),
            NativeWriteTag.RolaWriteError_Index => (
                null,
                NativeText.Taken(error.payload.Index.reason)
            ),
            NativeWriteTag.RolaWriteError_Io => (
                NativeText.Taken(error.payload.Io.path),
                NativeText.Taken(error.payload.Io.reason)
            ),
            NativeWriteTag.RolaWriteError_Json => (
                null,
                NativeText.Taken(error.payload.Json.reason)
            ),
            NativeWriteTag.RolaWriteError_Internal => (
                null,
                NativeText.Taken(error.payload.Internal.reason)
            ),
            _ => (null, string.Empty),
        };
}
