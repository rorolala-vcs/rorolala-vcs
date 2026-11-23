using System.Runtime.InteropServices;

namespace RolaSharp.Write;

/// <summary>The unmanaged memory one native write request borrows for a call.</summary>
/// <remarks>
/// The native side reads every string and array a request holds only while the call runs, so this
/// keeps each allocation until <see cref="Dispose"/> is called. Every member that returns a
/// pointer hands back one this owns.
/// </remarks>
internal sealed class NativeRequest : IDisposable
{
    /// <summary>The allocations made for one call, freed in the order they were made.</summary>
    private readonly List<nint> _allocations = [];

    /// <summary>Holds <paramref name="text"/> for the call, answering where it lies.</summary>
    /// <param name="text">The text, or nothing for an empty one.</param>
    /// <returns>The UTF-8 text, or zero when there is none.</returns>
    public nint Text(string? text) =>
        string.IsNullOrEmpty(text) ? 0 : Hold(Marshal.StringToCoTaskMemUTF8(text));

    /// <summary>Holds a list of strings for the call.</summary>
    /// <param name="items">The strings.</param>
    /// <returns>The span the native side reads as the list.</returns>
    public RolaStrSpan Span(IReadOnlyList<string> items)
    {
        ArgumentNullException.ThrowIfNull(items);

        if (items.Count == 0)
        {
            return default;
        }

        var size = Marshal.SizeOf<nint>();
        var array = Hold(Marshal.AllocCoTaskMem(size * items.Count));
        for (var index = 0; index < items.Count; index++)
        {
            Marshal.WriteIntPtr(array, index * size, Text(items[index]));
        }

        return new RolaStrSpan { items = array, count = (nuint)items.Count };
    }

    /// <summary>Holds an array of native records for the call.</summary>
    /// <typeparam name="T">The native record the caller filled.</typeparam>
    /// <param name="values">The records.</param>
    /// <returns>Where the array lies and how many records it holds.</returns>
    public (nint Items, nuint Count) Array<T>(IReadOnlyList<T> values)
        where T : struct
    {
        ArgumentNullException.ThrowIfNull(values);

        if (values.Count == 0)
        {
            return (0, 0);
        }

        var size = Marshal.SizeOf<T>();
        var array = Hold(Marshal.AllocCoTaskMem(size * values.Count));
        for (var index = 0; index < values.Count; index++)
        {
            Marshal.StructureToPtr(values[index], array + (index * size), false);
        }

        return (array, (nuint)values.Count);
    }

    /// <summary>Holds one native record and answers where it lies.</summary>
    /// <typeparam name="T">The native record.</typeparam>
    /// <param name="value">The record.</param>
    /// <returns>The pointer the native side reads the record through.</returns>
    public nint Pointer<T>(T value)
        where T : struct
    {
        var pointer = Hold(Marshal.AllocCoTaskMem(Marshal.SizeOf<T>()));
        Marshal.StructureToPtr(value, pointer, false);

        return pointer;
    }

    /// <summary>The fields every write request carries.</summary>
    /// <param name="request">The request the fields come from.</param>
    /// <param name="workspace">The root of the Workspace the writer works over.</param>
    /// <returns>The native fields.</returns>
    public RolaWriteCommon Common(WriteRequest request, string workspace)
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(workspace);

        return new RolaWriteCommon
        {
            workspace = Text(workspace),
            account = Text(request.Account),
            target = Target(request.Target),
            offline = Flag(request.Offline),
            force = Flag(request.Force),
            concurrency = (nuint)Math.Max(request.Concurrency, 0),
            labels = Labels(request.Labels),
        };
    }

    /// <summary>The Vault a request reaches for.</summary>
    /// <param name="target">The target.</param>
    /// <returns>The native target.</returns>
    public RolaWriteTarget Target(WriteTarget target)
    {
        ArgumentNullException.ThrowIfNull(target);

        return target.Kind switch
        {
            WriteTargetKind.Named => new RolaWriteTarget
            {
                tag = RolaWriteTargetTag.RolaWriteTarget_Named,
                payload = new RolaWriteTargetPayload
                {
                    Named = new RolaWriteTargetNamed { name = Text(target.Name) },
                },
            },
            WriteTargetKind.Address => new RolaWriteTarget
            {
                tag = RolaWriteTargetTag.RolaWriteTarget_Address,
                payload = new RolaWriteTargetPayload
                {
                    Address = new RolaWriteTargetAddress { address = Text(target.Address) },
                },
            },
            _ => new RolaWriteTarget
            {
                tag = RolaWriteTargetTag.RolaWriteTarget_Tracked,
            },
        };
    }

    /// <summary>The words a write calls the work while it does it.</summary>
    /// <param name="labels">The words, or nothing for the native ones.</param>
    /// <returns>The native labels, every field empty when none were given.</returns>
    public RolaWriteLabels Labels(WriteLabels? labels) =>
        labels is null
            ? default
            : new RolaWriteLabels
            {
                reading = Text(labels.Reading),
                writing = Text(labels.Writing),
                fetching = Text(labels.Fetching),
                uploading = Text(labels.Uploading),
                downloading = Text(labels.Downloading),
                checking = Text(labels.Checking),
                applying = Text(labels.Applying),
            };

    /// <summary>What an alignment is to do with the one path a run named.</summary>
    /// <param name="mode">The mode.</param>
    /// <returns>The native mode.</returns>
    public RolaWriteAlignMode Mode(AlignMode mode)
    {
        ArgumentNullException.ThrowIfNull(mode);

        return mode.Kind switch
        {
            AlignModeKind.MoveTo => new RolaWriteAlignMode
            {
                tag = RolaWriteAlignModeTag.RolaWriteAlignMode_MoveTo,
                payload = new RolaWriteAlignModePayload
                {
                    MoveTo = new RolaWriteAlignModeMoveTo { path = Text(mode.Path) },
                },
            },
            AlignModeKind.Rename => Simple(AlignModeKind.Rename),
            AlignModeKind.Break => Simple(AlignModeKind.Break),
            AlignModeKind.RestoreMove => Simple(AlignModeKind.RestoreMove),
            AlignModeKind.RestoreModify => Simple(AlignModeKind.RestoreModify),
            AlignModeKind.RestoreStaged => Simple(AlignModeKind.RestoreStaged),
            AlignModeKind.RestoreDelete => Simple(AlignModeKind.RestoreDelete),
            AlignModeKind.Restore => Simple(AlignModeKind.Restore),
            _ => Simple(AlignModeKind.Delete),
        };
    }

    /// <summary>Gives every allocation back.</summary>
    public void Dispose()
    {
        foreach (var allocation in _allocations)
        {
            Marshal.FreeCoTaskMem(allocation);
        }

        _allocations.Clear();
    }

    /// <summary>The native mode for a mode that carries nothing.</summary>
    /// <param name="kind">The mode's kind.</param>
    /// <returns>The native mode.</returns>
    private static RolaWriteAlignMode Simple(AlignModeKind kind) =>
        new() { tag = kind switch
        {
            AlignModeKind.Rename => RolaWriteAlignModeTag.RolaWriteAlignMode_Rename,
            AlignModeKind.Break => RolaWriteAlignModeTag.RolaWriteAlignMode_Break,
            AlignModeKind.RestoreMove => RolaWriteAlignModeTag.RolaWriteAlignMode_RestoreMove,
            AlignModeKind.RestoreModify => RolaWriteAlignModeTag.RolaWriteAlignMode_RestoreModify,
            AlignModeKind.RestoreStaged => RolaWriteAlignModeTag.RolaWriteAlignMode_RestoreStaged,
            AlignModeKind.RestoreDelete => RolaWriteAlignModeTag.RolaWriteAlignMode_RestoreDelete,
            AlignModeKind.Restore => RolaWriteAlignModeTag.RolaWriteAlignMode_Restore,
            _ => RolaWriteAlignModeTag.RolaWriteAlignMode_Delete,
        } };

    /// <summary>A bool as the C `bool` the header declares.</summary>
    /// <param name="value">The bool.</param>
    /// <returns>One for true, zero for false.</returns>
    public static byte Flag(bool value) => (byte)(value ? 1 : 0);

    /// <summary>Keeps a pointer until the call is over.</summary>
    /// <param name="pointer">The pointer.</param>
    /// <returns>The same pointer.</returns>
    private nint Hold(nint pointer)
    {
        _allocations.Add(pointer);

        return pointer;
    }
}
