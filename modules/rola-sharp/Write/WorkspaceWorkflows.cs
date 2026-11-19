using System.Diagnostics.CodeAnalysis;
using System.Runtime.InteropServices;
using System.Text.Json;

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

        using var document = JsonDocument.Parse(
            Invoke(
                JsonSerializer.Serialize(request with { Workspace = _workspace.Root.FullName }),
                progress,
                cancellation,
                RorolalaBinding.rola_write_bring
            )
        );

        return Brings.Parse(document.RootElement);
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

        using var document = JsonDocument.Parse(
            Invoke(
                JsonSerializer.Serialize(request with { Workspace = _workspace.Root.FullName }),
                progress,
                cancellation,
                RorolalaBinding.rola_write_bring
            )
        );

        return Brings.ParseVariant(document.RootElement);
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

        using var document = JsonDocument.Parse(
            Invoke(
                JsonSerializer.Serialize(request with { Workspace = _workspace.Root.FullName }),
                progress,
                cancellation,
                RorolalaBinding.rola_write_retrack
            )
        );

        return Retracks.Parse(document.RootElement);
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

        var json = Invoke(
            JsonSerializer.Serialize(request with { Workspace = _workspace.Root.FullName }),
            progress,
            cancellation,
            RorolalaBinding.rola_write_track_plan
        );

        using var document = JsonDocument.Parse(json);

        return Tracks.ParsePlan(document.RootElement) with { Json = json };
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
        Justification = "a null progress or cancellation is the caller asking this side to make one, which Invoke does"
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
        if (string.IsNullOrEmpty(plan.Json))
        {
            throw new RolaWriteError(
                RolaWriteErrorKind.Precondition,
                null,
                "the plan was not made by PlanTrack"
            );
        }

        using var document = JsonDocument.Parse(
            Invoke(
                JsonSerializer.Serialize(request with { Workspace = _workspace.Root.FullName }),
                plan.Json,
                JsonSerializer.Serialize(messages),
                progress,
                cancellation,
                RorolalaBinding.rola_write_track_apply
            )
        );

        return Tracks.ParseApply(document.RootElement);
    }

    /// <summary>Settles one path against the Layout it works from.</summary>
    /// <param name="request">The path, the mode, and the flags that shape the run.</param>
    /// <param name="progress">Where the run says what it is doing, or nothing to make one here.</param>
    /// <param name="cancellation">The flag the caller sets to stop the run, or nothing to make one here.</param>
    /// <returns>What the run did.</returns>
    /// <exception cref="RolaWriteError">
    /// The run failed, or refused to act. A refusal is reported as
    /// <see cref="RolaWriteErrorKind.Precondition"/> with the refusal's own JSON as its reason, since a
    /// refusal is a decision the path made rather than a failure of the write.
    /// </exception>
    public AlignResult Align(
        AlignRequest request,
        Progress? progress = null,
        Cancellation? cancellation = null
    )
    {
        ArgumentNullException.ThrowIfNull(request);

        using var document = JsonDocument.Parse(
            Invoke(
                JsonSerializer.Serialize(request with { Workspace = _workspace.Root.FullName }),
                progress,
                cancellation,
                RorolalaBinding.rola_write_align
            )
        );

        return Aligns.Parse(document.RootElement);
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

        using var document = JsonDocument.Parse(
            Invoke(
                JsonSerializer.Serialize(request with { Workspace = _workspace.Root.FullName }),
                progress,
                cancellation,
                RorolalaBinding.rola_write_ownership_plan
            )
        );

        return Ownerships.ParsePlan(document.RootElement);
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
        Justification = "a null progress or cancellation is the caller asking this side to make one, which Invoke does"
    )]
    public OwnershipResult ApplyOwnership(
        OwnershipPlanRequest request,
        OwnershipPlan? plan = null,
        Progress? progress = null,
        Cancellation? cancellation = null
    )
    {
        ArgumentNullException.ThrowIfNull(request);

        // The native side reads only the entries a plan promised to move; its refusals and its skip
        // count are the caller's to keep, and writing them back would only be a copy it never reads.
        var agreed =
            plan is null ? string.Empty : JsonSerializer.Serialize(new { wanted = plan.Wanted });

        using var document = JsonDocument.Parse(
            Invoke(
                JsonSerializer.Serialize(request with { Workspace = _workspace.Root.FullName }),
                agreed,
                progress,
                cancellation,
                RorolalaBinding.rola_write_ownership_apply
            )
        );

        return Ownerships.ParseApply(document.RootElement);
    }

    /// <summary>A native call that takes a request, a progress handle and a cancellation handle.</summary>
    /// <param name="request">The UTF-8 request document.</param>
    /// <param name="progress">The progress the run says itself to.</param>
    /// <param name="cancellation">The flag the run checks.</param>
    /// <returns>The native answer, which the caller owns.</returns>
    private delegate RorolalaResult RequestCall(nint request, nint progress, nint cancellation);

    /// <summary>A native call that takes a request, a plan and both handles.</summary>
    /// <param name="request">The UTF-8 request document.</param>
    /// <param name="plan">The UTF-8 plan document, or zero when the run is to make its own.</param>
    /// <param name="progress">The progress the run says itself to.</param>
    /// <param name="cancellation">The flag the run checks.</param>
    /// <returns>The native answer, which the caller owns.</returns>
    private delegate RorolalaResult PlanCall(
        nint request,
        nint plan,
        nint progress,
        nint cancellation
    );

    /// <summary>A native call that takes a request, a plan, a message map and both handles.</summary>
    /// <param name="request">The UTF-8 request document.</param>
    /// <param name="plan">The UTF-8 plan document, or zero when the run is to make its own.</param>
    /// <param name="messages">The UTF-8 message map, or zero when the run takes none.</param>
    /// <param name="progress">The progress the run says itself to.</param>
    /// <param name="cancellation">The flag the run checks.</param>
    /// <returns>The native answer, which the caller owns.</returns>
    private delegate RorolalaResult MessageCall(
        nint request,
        nint plan,
        nint messages,
        nint progress,
        nint cancellation
    );

    /// <summary>Runs a request call, making the handles a caller left out.</summary>
    /// <param name="request">The request document.</param>
    /// <param name="progress">The caller's progress, or nothing.</param>
    /// <param name="cancellation">The caller's cancellation, or nothing.</param>
    /// <param name="call">The native call.</param>
    /// <returns>The answer JSON, which the native side owned.</returns>
    private static string Invoke(
        string request,
        Progress? progress,
        Cancellation? cancellation,
        RequestCall call
    ) =>
        Invoke(
            request,
            null,
            null,
            progress,
            cancellation,
            (asked, _, _, watching, stopping) => call(asked, watching, stopping)
        );

    /// <summary>Runs a plan call, making the handles a caller left out.</summary>
    /// <param name="request">The request document.</param>
    /// <param name="plan">The plan document, or nothing for a call that takes none.</param>
    /// <param name="progress">The caller's progress, or nothing.</param>
    /// <param name="cancellation">The caller's cancellation, or nothing.</param>
    /// <param name="call">The native call.</param>
    /// <returns>The answer JSON, which the native side owned.</returns>
    private static string Invoke(
        string request,
        string? plan,
        Progress? progress,
        Cancellation? cancellation,
        PlanCall call
    ) =>
        Invoke(
            request,
            plan,
            null,
            progress,
            cancellation,
            (asked, given, _, watching, stopping) => call(asked, given, watching, stopping)
        );

    /// <summary>Runs a call, making the handles a caller left out and releasing what this owned.</summary>
    /// <param name="request">The request document.</param>
    /// <param name="plan">The plan document, or nothing for a call that takes none.</param>
    /// <param name="messages">The message map, or nothing for a call that takes none.</param>
    /// <param name="progress">The caller's progress, or nothing.</param>
    /// <param name="cancellation">The caller's cancellation, or nothing.</param>
    /// <param name="call">The native call.</param>
    /// <returns>The answer JSON, which the native side owned.</returns>
    /// <exception cref="RolaWriteError">The native call answered with a failure this takes over.</exception>
    private static string Invoke(
        string request,
        string? plan,
        string? messages,
        Progress? progress,
        Cancellation? cancellation,
        MessageCall call
    )
    {
        Progress? ownedProgress = null;
        Cancellation? ownedCancellation = null;
        nint requestText = 0;
        nint planText = 0;
        nint messagesText = 0;
        try
        {
            var watching = progress;
            if (watching is null)
            {
                ownedProgress = Progress.New();
                watching = ownedProgress;
            }

            // The native side refuses a null handle rather than reading it as "no progress", so a
            // caller that asked for neither still hands in a real one it never reads back.
            var stopping = cancellation;
            if (stopping is null)
            {
                ownedCancellation = Cancellation.New();
                stopping = ownedCancellation;
            }

            requestText = Utf8(request);
            planText = plan is null ? 0 : Utf8(plan);
            messagesText = messages is null ? 0 : Utf8(messages);

            var answer = call(
                requestText,
                planText,
                messagesText,
                watching.Handle,
                stopping.Handle
            );
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw WriteResults.From(answer.payload);
            }

            return NativeText.Taken(answer.payload);
        }
        finally
        {
            if (requestText != 0)
            {
                Marshal.FreeCoTaskMem(requestText);
            }

            if (planText != 0)
            {
                Marshal.FreeCoTaskMem(planText);
            }

            if (messagesText != 0)
            {
                Marshal.FreeCoTaskMem(messagesText);
            }

            ownedProgress?.Dispose();
            ownedCancellation?.Dispose();
        }
    }
}
