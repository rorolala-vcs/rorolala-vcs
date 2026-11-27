using RolaSharp;

namespace RolaSharp.Write;

/// <summary>The history workflows: folding, cutting, undoing and adjusting a chain.</summary>
/// <remarks>
/// Every method runs the same `librorolala` workflow the command line runs, over the Workspace this
/// writer was made over. A run that changes a chain also records what it did in the Workspace's log,
/// which is the stack <see cref="Undo"/> and <see cref="UndoPlan"/> read.
/// </remarks>
public sealed class HistoryWriter : Writer
{
    private readonly WorkspaceScope _workspace;

    /// <summary>Makes a writer over a Workspace.</summary>
    /// <param name="workspace">The Workspace, which must have been read.</param>
    public HistoryWriter(WorkspaceScope workspace) =>
        _workspace = workspace ?? throw new ArgumentNullException(nameof(workspace));

    /// <summary>Folds the local versions above the tracked Vault's version into one.</summary>
    /// <param name="request">The path, the target and what is kept apart from the fold.</param>
    /// <param name="progress">Where the run says what it is doing, or nothing to make one here.</param>
    /// <param name="cancellation">The flag the caller sets to stop the run, or nothing to make one here.</param>
    /// <returns>What the fold wrote.</returns>
    /// <exception cref="RolaWriteError">The run failed, or refused the fold, and the failure says why.</exception>
    public Collapsed Collapse(
        CollapseRequest request,
        Progress? progress = null,
        Cancellation? cancellation = null
    )
    {
        ArgumentNullException.ThrowIfNull(request);

        using var memory = new NativeRequest();
        var native = memory.Pointer(
            new RolaWriteCollapseRequest
            {
                common = memory.Common(request, _workspace.Root.FullName),
                file = memory.Text(request.File),
                target = memory.Text(request.TargetVersion),
                keep = memory.Span(request.Keep),
                keep_join = NativeRequest.Flag(request.KeepJoin),
            }
        );

        var answer = Calls.Call(RorolalaBinding.rola_write_collapse, native, progress, cancellation);
        try
        {
            return Collapses.Read(answer);
        }
        finally
        {
            RorolalaBinding.free_rola_write_collapse(answer);
        }
    }

    /// <summary>Cuts a file's old history away and starts it again at a kept version.</summary>
    /// <param name="request">The path, the target, and whether the `Uuid` and the merges are kept.</param>
    /// <param name="progress">Where the run says what it is doing, or nothing to make one here.</param>
    /// <param name="cancellation">The flag the caller sets to stop the run, or nothing to make one here.</param>
    /// <returns>What the cut wrote.</returns>
    /// <exception cref="RolaWriteError">The run failed, or refused the cut, and the failure says why.</exception>
    public CutResult Cut(
        CutRequest request,
        Progress? progress = null,
        Cancellation? cancellation = null
    )
    {
        ArgumentNullException.ThrowIfNull(request);

        using var memory = new NativeRequest();
        var native = memory.Pointer(
            new RolaWriteCutRequest
            {
                common = memory.Common(request, _workspace.Root.FullName),
                file = memory.Text(request.File),
                target = memory.Text(request.TargetVersion),
                keep_uuid = NativeRequest.Flag(request.KeepUuid),
                strip_join = NativeRequest.Flag(request.StripJoin),
            }
        );

        var answer = Calls.Call(RorolalaBinding.rola_write_cut, native, progress, cancellation);
        try
        {
            return Cuts.Read(answer);
        }
        finally
        {
            RorolalaBinding.free_rola_write_cut(answer);
        }
    }

    /// <summary>Reads the operations a run would undo, newest first, and how each stands.</summary>
    /// <param name="request">How many operations to consider, and whether only the record moves.</param>
    /// <returns>Each operation the run considered, and whether every change it made still stands.</returns>
    /// <exception cref="RolaWriteError">
    /// Nothing is recorded to undo, the count names no operation, or a file stands where the
    /// operation cannot be refused past.
    /// </exception>
    public UndoPlanReading UndoPlan(UndoRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        using var memory = new NativeRequest();
        var native = memory.Pointer(UndoFields(memory, request));

        var answer = Calls.Payload(RorolalaBinding.rola_write_undo_plan(native));
        try
        {
            return UndoPlans.Read(answer);
        }
        finally
        {
            RorolalaBinding.free_rola_write_undo_plan(answer);
        }
    }

    /// <summary>Puts back what the last recorded operations changed, newest first.</summary>
    /// <param name="request">
    /// How many operations to undo, whether only the record moves, and what to do with an operation
    /// that stands only in part.
    /// </param>
    /// <returns>The commands whose operations were undone.</returns>
    /// <exception cref="RolaWriteError">
    /// Nothing is recorded to undo, the count names no operation, a file changed after the operation,
    /// or the two ways of undoing were asked for together.
    /// </exception>
    public Undone Undo(UndoRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        using var memory = new NativeRequest();
        var native = memory.Pointer(UndoFields(memory, request));

        var answer = Calls.Payload(RorolalaBinding.rola_write_undo(native));
        try
        {
            return Undos.Read(answer);
        }
        finally
        {
            RorolalaBinding.free_rola_write_undone(answer);
        }
    }

    /// <summary>Applies a sheet to a path's chain, rebuilding what follows the point adjusted.</summary>
    /// <param name="request">The path and the sheet the editor would have given back.</param>
    /// <param name="progress">Where the run says what it is doing, or nothing to make one here.</param>
    /// <param name="cancellation">The flag the caller sets to stop the run, or nothing to make one here.</param>
    /// <returns>What the run wrote.</returns>
    /// <exception cref="RolaWriteError">The run failed, or the sheet does not fit the chain.</exception>
    public Tweaked Tweak(
        TweakRequest request,
        Progress? progress = null,
        Cancellation? cancellation = null
    )
    {
        ArgumentNullException.ThrowIfNull(request);

        using var memory = new NativeRequest();
        var native = memory.Pointer(
            new RolaWriteTweakRequest
            {
                common = memory.Common(request, _workspace.Root.FullName),
                file = memory.Text(request.File),
                sheet = memory.Text(request.Sheet),
            }
        );

        var answer = Calls.Call(RorolalaBinding.rola_write_tweak, native, progress, cancellation);
        try
        {
            return Tweaks.Read(answer);
        }
        finally
        {
            RorolalaBinding.free_rola_write_tweak(answer);
        }
    }

    /// <summary>Drops the plan waiting for a path.</summary>
    /// <param name="request">The path whose pending plan is dropped.</param>
    /// <exception cref="RolaWriteError">
    /// No plan waits for the path, or the Layout records no entry there.
    /// </exception>
    public void CancelTweak(TweakCancelRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        using var memory = new NativeRequest();
        var native = memory.Pointer(
            new RolaWriteTweakCancelRequest
            {
                common = memory.Common(request, _workspace.Root.FullName),
                file = memory.Text(request.File),
            }
        );

        Calls.Payload(RorolalaBinding.rola_write_tweak_cancel(native));
    }

    /// <summary>The fields an undo sends, which a plan and an apply share.</summary>
    /// <param name="memory">Where the request's strings are held.</param>
    /// <param name="request">The undo.</param>
    /// <returns>The native request.</returns>
    private RolaWriteUndoRequest UndoFields(NativeRequest memory, UndoRequest request) =>
        new()
        {
            common = memory.Common(request, _workspace.Root.FullName),
            count = (uint)Math.Max(request.Count, 0),
            soft = NativeRequest.Flag(request.Soft),
            on_partial = Partial(request.OnPartial),
        };

    /// <summary>The native word for how a partly matched operation is taken.</summary>
    private static RolaWriteUndoPartial Partial(UndoPartial partial) =>
        partial switch
        {
            UndoPartial.Continue => RolaWriteUndoPartial.RolaWriteUndoPartial_Continue,
            _ => RolaWriteUndoPartial.RolaWriteUndoPartial_Stop,
        };
}
