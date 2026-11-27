using System.Diagnostics.CodeAnalysis;

using RolaSharp;

namespace RolaSharp.Write;

/// <summary>The one-shot write workflows a Workspace's C ABI exposes.</summary>
public sealed partial class WorkspaceWriter
{
    /// <summary>Brings what a Vault holds and this Layout does not into it.</summary>
    /// <param name="request">The references to bring in, and where they land.</param>
    /// <param name="progress">Where the run says what it is doing, or nothing to make one here.</param>
    /// <param name="cancellation">The flag the caller sets to stop the run, or nothing to make one here.</param>
    /// <returns>What came in, and what could not.</returns>
    /// <exception cref="RolaWriteError">The run failed, and the failure says why.</exception>
    public BringResult Bring(
        BringRequest request,
        Progress? progress = null,
        Cancellation? cancellation = null
    )
    {
        ArgumentNullException.ThrowIfNull(request);

        using var memory = new NativeRequest();
        var native = memory.Pointer(
            new RolaWriteBringRequest
            {
                common = memory.Common(request, _workspace.Root.FullName),
                refs = memory.Span(request.Refs),
                to = memory.Text(request.To),
                by_name = NativeRequest.Flag(request.ByName),
            }
        );

        var answer = Calls.Call(RorolalaBinding.rola_write_bring, native, progress, cancellation);
        try
        {
            return Brings.Read(answer);
        }
        finally
        {
            RorolalaBinding.free_rola_write_bring(answer);
        }
    }

    /// <summary>Brings a variant in to be joined into a file this Layout names.</summary>
    /// <param name="request">The variant, the file it is joined into, and where its file is to lie.</param>
    /// <param name="progress">Where the run says what it is doing, or nothing to make one here.</param>
    /// <param name="cancellation">The flag the caller sets to stop the run, or nothing to make one here.</param>
    /// <returns>The file it waits to be joined into, the variant, and where its file was written.</returns>
    /// <exception cref="RolaWriteError">
    /// The run failed, or refused the merge, and the failure says why. A refusal is a decision the
    /// merge made rather than a failure of the write, and the native side answers one with
    /// <see cref="RolaWriteErrorKind.NotFound"/> or <see cref="RolaWriteErrorKind.Precondition"/>.
    /// </exception>
    public BringVariantResult BringVariant(
        BringVariantRequest request,
        Progress? progress = null,
        Cancellation? cancellation = null
    )
    {
        ArgumentNullException.ThrowIfNull(request);

        using var memory = new NativeRequest();
        var native = memory.Pointer(
            new RolaWriteBringRequest
            {
                common = memory.Common(request, _workspace.Root.FullName),
                to = memory.Text(request.To),
                join = memory.Text(request.Join),
                variant = memory.Text(request.Variant),
                file = memory.Text(request.File),
                tree_merge = NativeRequest.Flag(request.TreeMerge),
            }
        );

        var answer = Calls.Call(RorolalaBinding.rola_write_bring, native, progress, cancellation);
        try
        {
            return Brings.ReadVariant(answer);
        }
        finally
        {
            RorolalaBinding.free_rola_write_bring(answer);
        }
    }

    /// <summary>Moves a path's record to a version, or one version back when none is named.</summary>
    /// <param name="request">The file, what it is to record, and the flags that shape the move.</param>
    /// <param name="progress">Where the run says what it is doing, or nothing to make one here.</param>
    /// <param name="cancellation">The flag the caller sets to stop the run, or nothing to make one here.</param>
    /// <returns>What the record moved to.</returns>
    /// <exception cref="RolaWriteError">The run failed, and the failure says why.</exception>
    public RetrackResult Retrack(
        RetrackRequest request,
        Progress? progress = null,
        Cancellation? cancellation = null
    )
    {
        ArgumentNullException.ThrowIfNull(request);

        using var memory = new NativeRequest();
        var native = memory.Pointer(
            new RolaWriteRetrackRequest
            {
                common = memory.Common(request, _workspace.Root.FullName),
                file = memory.Text(request.File),
                record = memory.Text(request.Record),
                only_index = NativeRequest.Flag(request.OnlyIndex),
                deny_jump = NativeRequest.Flag(request.DenyJump),
                fork = NativeRequest.Flag(request.Fork),
            }
        );

        var answer = Calls.Call(RorolalaBinding.rola_write_retrack, native, progress, cancellation);
        try
        {
            return Retracks.Read(answer);
        }
        finally
        {
            RorolalaBinding.free_rola_write_retrack(answer);
        }
    }

    /// <summary>Reads what a batch recording would record, without writing anything.</summary>
    /// <param name="request">The names to record, and the flags that shape the recording.</param>
    /// <param name="progress">Where the run says what it is doing, or nothing to make one here.</param>
    /// <param name="cancellation">The flag the caller sets to stop the run, or nothing to make one here.</param>
    /// <returns>What the run would record, and what stops it.</returns>
    /// <exception cref="RolaWriteError">The run failed, and the failure says why.</exception>
    public TrackPlan PlanTrack(
        TrackRequest request,
        Progress? progress = null,
        Cancellation? cancellation = null
    )
    {
        ArgumentNullException.ThrowIfNull(request);

        using var memory = new NativeRequest();
        var native = memory.Pointer(TrackFields(memory, request));

        var answer = Calls.Call(RorolalaBinding.rola_write_track_plan, native, progress, cancellation);
        try
        {
            return Tracks.ReadPlan(answer);
        }
        finally
        {
            RorolalaBinding.free_rola_write_track_plan(answer);
        }
    }

    /// <summary>Records what a plan named, once the caller has said what each subject did.</summary>
    /// <param name="request">The names to record, and the flags that shape the recording.</param>
    /// <param name="plan">The plan a caller agreed to, which must be one <see cref="PlanTrack"/> made.</param>
    /// <param name="messages">What each subject's version says, by the path the plan names it by.</param>
    /// <param name="progress">Where the run says what it is doing, or nothing to make one here.</param>
    /// <param name="cancellation">The flag the caller sets to stop the run, or nothing to make one here.</param>
    /// <returns>The subjects recorded, and what was refused.</returns>
    /// <exception cref="RolaWriteError">
    /// The plan did not come from <see cref="PlanTrack"/>, or the run failed, and the failure says why.
    /// </exception>
    [SuppressMessage(
        "Usage",
        "CA1062:Validate arguments of public methods",
        Justification = "a null progress or cancellation is the caller asking this side to make one, which Call does"
    )]
    public TrackResult Track(
        TrackRequest request,
        TrackPlan plan,
        IReadOnlyDictionary<string, string> messages,
        Progress? progress = null,
        Cancellation? cancellation = null
    )
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(plan);
        ArgumentNullException.ThrowIfNull(messages);

        // The native side rereads the plan's subjects and their digests against the work; a plan
        // built by hand holds no subjects it could send, and the messages alone would say nothing
        // about which content each one is for.
        if (!plan.Agreed)
        {
            throw new RolaWriteError(
                RolaWriteErrorKind.Precondition,
                null,
                "the plan was not made by PlanTrack"
            );
        }

        using var memory = new NativeRequest();
        var native = memory.Pointer(TrackFields(memory, request));
        var subjects = memory.Array(
            plan.Subjects
                .Select(subject => new RolaWriteTrackAgreed
                {
                    path = memory.Text(subject.Path),
                    digest = memory.Text(subject.Digest),
                })
                .ToList()
        );
        var words = memory.Array(
            messages
                .Select(pair => new RolaWriteTrackMessage
                {
                    path = memory.Text(pair.Key),
                    text = memory.Text(pair.Value),
                })
                .ToList()
        );

        var answer = Call(
            RorolalaBinding.rola_write_track_apply,
            native,
            subjects.Items,
            subjects.Count,
            words.Items,
            words.Count,
            progress,
            cancellation
        );
        try
        {
            return Tracks.ReadApply(answer);
        }
        finally
        {
            RorolalaBinding.free_rola_write_track_apply(answer);
        }
    }

    /// <summary>Settles one path against the Layout it works from.</summary>
    /// <param name="request">The path, the mode, and the flags that shape the run.</param>
    /// <param name="progress">Where the run says what it is doing, or nothing to make one here.</param>
    /// <param name="cancellation">The flag the caller sets to stop the run, or nothing to make one here.</param>
    /// <returns>What the run did.</returns>
    /// <exception cref="RolaWriteError">
    /// The run failed, or refused to act. A refusal is reported as
    /// <see cref="RolaWriteErrorKind.Precondition"/> with the reasons the refusal names, since a
    /// refusal is a decision the path made rather than a failure of the write.
    /// </exception>
    public AlignResult Align(
        AlignRequest request,
        Progress? progress = null,
        Cancellation? cancellation = null
    )
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(request.Mode);

        using var memory = new NativeRequest();
        var native = memory.Pointer(
            new RolaWriteAlignRequest
            {
                common = memory.Common(request, _workspace.Root.FullName),
                path = memory.Text(request.Path),
                mode = memory.Mode(request.Mode),
                completely = NativeRequest.Flag(request.Completely),
                include_ignored = NativeRequest.Flag(request.IncludeIgnored),
            }
        );

        var answer = Calls.Call(RorolalaBinding.rola_write_align, native, progress, cancellation);
        try
        {
            return Aligns.Read(answer);
        }
        finally
        {
            RorolalaBinding.free_rola_write_align(answer);
        }
    }

    /// <summary>Reads every gate an ownership move has to pass, without moving any.</summary>
    /// <param name="request">Which way the ownership moves, and the names it is to move for.</param>
    /// <param name="progress">Where the run says what it is doing, or nothing to make one here.</param>
    /// <param name="cancellation">The flag the caller sets to stop the run, or nothing to make one here.</param>
    /// <returns>What a move would act on, what it refused, and what it skipped.</returns>
    /// <exception cref="RolaWriteError">The run failed, and the failure says why.</exception>
    public OwnershipPlan PlanOwnership(
        OwnershipPlanRequest request,
        Progress? progress = null,
        Cancellation? cancellation = null
    )
    {
        ArgumentNullException.ThrowIfNull(request);

        using var memory = new NativeRequest();
        var native = memory.Pointer(
            new RolaWriteOwnershipRequest
            {
                common = memory.Common(request, _workspace.Root.FullName),
                kind =
                    request.Kind == OwnershipKind.Giveup
                        ? RolaWriteOwnershipKind.RolaWriteOwnershipKind_Giveup
                        : RolaWriteOwnershipKind.RolaWriteOwnershipKind_Hold,
                paths = memory.Span(request.Paths),
                allow_partial = NativeRequest.Flag(request.AllowPartial),
            }
        );

        var answer = Calls.Call(
            RorolalaBinding.rola_write_ownership_plan,
            native,
            progress,
            cancellation
        );
        try
        {
            return Ownerships.ReadPlan(answer);
        }
        finally
        {
            RorolalaBinding.free_rola_write_ownership_plan(answer);
        }
    }

    /// <summary>Moves the ownership a plan named, once every gate has been read again.</summary>
    /// <param name="request">Which way the ownership moves, and the names it is to move for.</param>
    /// <param name="plan">The plan a caller agreed to, or nothing to re-make and apply one.</param>
    /// <param name="progress">Where the run says what it is doing, or nothing to make one here.</param>
    /// <param name="cancellation">The flag the caller sets to stop the run, or nothing to make one here.</param>
    /// <returns>The entries the Vault moved, and what was refused.</returns>
    /// <exception cref="RolaWriteError">The run failed, and the failure says why.</exception>
    [SuppressMessage(
        "Usage",
        "CA1062:Validate arguments of public methods",
        Justification = "a null progress or cancellation is the caller asking this side to make one, which Call does"
    )]
    public OwnershipResult ApplyOwnership(
        OwnershipPlanRequest request,
        OwnershipPlan? plan = null,
        Progress? progress = null,
        Cancellation? cancellation = null
    )
    {
        ArgumentNullException.ThrowIfNull(request);

        using var memory = new NativeRequest();
        var native = memory.Pointer(
            new RolaWriteOwnershipRequest
            {
                common = memory.Common(request, _workspace.Root.FullName),
                kind =
                    request.Kind == OwnershipKind.Giveup
                        ? RolaWriteOwnershipKind.RolaWriteOwnershipKind_Giveup
                        : RolaWriteOwnershipKind.RolaWriteOwnershipKind_Hold,
                paths = memory.Span(request.Paths),
                allow_partial = NativeRequest.Flag(request.AllowPartial),
            }
        );

        // The native side reads only the entries a plan promised to move; its refusals and its skip
        // count are the caller's to keep, and sending them back would only be a copy it never reads.
        var agreed =
            plan is null
                ? (Items: 0, Count: (nuint)0)
                : memory.Array(
                    plan.Wanted
                        .Select(wanted => new RolaWriteOwnershipAgreed
                        {
                            path = memory.Text(wanted.Path),
                            uuid = memory.Text(wanted.Uuid),
                        })
                        .ToList()
                );

        var answer = Call(
            RorolalaBinding.rola_write_ownership_apply,
            native,
            agreed.Items,
            agreed.Count,
            progress,
            cancellation
        );
        try
        {
            return Ownerships.ReadApply(answer);
        }
        finally
        {
            RorolalaBinding.free_rola_write_ownership_apply(answer);
        }
    }

    /// <summary>A native call that takes a request, an agreed array and both handles.</summary>
    /// <param name="request">The native request.</param>
    /// <param name="agreed">The caller's array.</param>
    /// <param name="agreedLen">How many records the array holds.</param>
    /// <param name="progress">The progress the run says itself to.</param>
    /// <param name="cancellation">The flag the run checks.</param>
    /// <returns>The native answer, which the caller owns.</returns>
    private delegate RorolalaResult AgreedCall(
        nint request,
        nint agreed,
        nuint agreedLen,
        nint progress,
        nint cancellation
    );

    /// <summary>A native call that takes a request, two arrays and both handles.</summary>
    /// <param name="request">The native request.</param>
    /// <param name="subjects">The caller's subjects.</param>
    /// <param name="subjectsLen">How many subjects there are.</param>
    /// <param name="messages">The caller's messages.</param>
    /// <param name="messagesLen">How many messages there are.</param>
    /// <param name="progress">The progress the run says itself to.</param>
    /// <param name="cancellation">The flag the run checks.</param>
    /// <returns>The native answer, which the caller owns.</returns>
    private delegate RorolalaResult TrackCall(
        nint request,
        nint subjects,
        nuint subjectsLen,
        nint messages,
        nuint messagesLen,
        nint progress,
        nint cancellation
    );

    /// <summary>Runs a call with one agreed array, making the handles a caller left out.</summary>
    /// <param name="call">The native call.</param>
    /// <param name="request">The native request.</param>
    /// <param name="agreed">The caller's array.</param>
    /// <param name="agreedLen">How many records the array holds.</param>
    /// <param name="progress">The caller's progress, or nothing.</param>
    /// <param name="cancellation">The caller's cancellation, or nothing.</param>
    /// <returns>The answer handle, which the caller releases with the call's own `free_`.</returns>
    /// <exception cref="RolaWriteError">The native call answered with a failure this takes over.</exception>
    private static nint Call(
        AgreedCall call,
        nint request,
        nint agreed,
        nuint agreedLen,
        Progress? progress,
        Cancellation? cancellation
    )
    {
        var (watching, stopping, ownedProgress, ownedCancellation) = Calls.Handles(progress, cancellation);
        try
        {
            var answer = call(request, agreed, agreedLen, watching.Handle, stopping.Handle);

            return Calls.Payload(answer);
        }
        finally
        {
            ownedProgress?.Dispose();
            ownedCancellation?.Dispose();
        }
    }

    /// <summary>Runs a track apply, making the handles a caller left out.</summary>
    /// <param name="call">The native call.</param>
    /// <param name="request">The native request.</param>
    /// <param name="subjects">The caller's subjects.</param>
    /// <param name="subjectsLen">How many subjects there are.</param>
    /// <param name="messages">The caller's messages.</param>
    /// <param name="messagesLen">How many messages there are.</param>
    /// <param name="progress">The caller's progress, or nothing.</param>
    /// <param name="cancellation">The caller's cancellation, or nothing.</param>
    /// <returns>The answer handle, which the caller releases with the call's own `free_`.</returns>
    /// <exception cref="RolaWriteError">The native call answered with a failure this takes over.</exception>
    private static nint Call(
        TrackCall call,
        nint request,
        nint subjects,
        nuint subjectsLen,
        nint messages,
        nuint messagesLen,
        Progress? progress,
        Cancellation? cancellation
    )
    {
        var (watching, stopping, ownedProgress, ownedCancellation) = Calls.Handles(progress, cancellation);
        try
        {
            var answer = call(
                request,
                subjects,
                subjectsLen,
                messages,
                messagesLen,
                watching.Handle,
                stopping.Handle
            );

            return Calls.Payload(answer);
        }
        finally
        {
            ownedProgress?.Dispose();
            ownedCancellation?.Dispose();
        }
    }

    /// <summary>The fields a batch recording sends, which a plan and an apply share.</summary>
    /// <param name="memory">Where the request's strings are held.</param>
    /// <param name="request">The recording.</param>
    /// <returns>The native request.</returns>
    private RolaWriteTrackRequest TrackFields(NativeRequest memory, TrackRequest request) =>
        new()
        {
            common = memory.Common(request, _workspace.Root.FullName),
            files = memory.Span(request.Files),
            replace = NativeRequest.Flag(request.Replace),
            policy = memory.Text(request.Policy),
            include_ignored = NativeRequest.Flag(request.IncludeIgnored),
        };
}
