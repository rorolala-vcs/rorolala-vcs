using RolaSharp;

namespace RolaSharp.Write;

/// <summary>The calls the writers share: one native answer in, one payload or failure out.</summary>
/// <remarks>
/// Every write workflow goes through the same three steps: the handles a caller left out are made
/// for the call, the answer's tag is read, and the payload is handed on while a failure becomes the
/// exception the caller catches. They live here so two writers cannot read an answer differently.
/// </remarks>
internal static class Calls
{
    /// <summary>A native call that takes a request, a progress handle and a cancellation handle.</summary>
    /// <param name="request">The native request.</param>
    /// <param name="progress">The progress the run says itself to.</param>
    /// <param name="cancellation">The flag the run checks.</param>
    /// <returns>The native answer, which the caller owns.</returns>
    public delegate RorolalaResult RequestCall(nint request, nint progress, nint cancellation);

    /// <summary>Runs a request call, making the handles a caller left out.</summary>
    /// <param name="call">The native call.</param>
    /// <param name="request">The native request.</param>
    /// <param name="progress">The caller's progress, or nothing.</param>
    /// <param name="cancellation">The caller's cancellation, or nothing.</param>
    /// <returns>The answer handle, which the caller releases with the call's own `free_`.</returns>
    /// <exception cref="RolaWriteError">The native call answered with a failure this takes over.</exception>
    public static nint Call(
        RequestCall call,
        nint request,
        Progress? progress,
        Cancellation? cancellation
    )
    {
        ArgumentNullException.ThrowIfNull(call);

        var (watching, stopping, ownedProgress, ownedCancellation) = Handles(progress, cancellation);
        try
        {
            var answer = call(request, watching.Handle, stopping.Handle);

            return Payload(answer);
        }
        finally
        {
            ownedProgress?.Dispose();
            ownedCancellation?.Dispose();
        }
    }

    /// <summary>Runs a call that takes no handles, making none.</summary>
    /// <param name="call">The native call.</param>
    /// <param name="request">The native request, or zero when the call takes none.</param>
    /// <returns>The answer handle, which the caller releases with the call's own `free_`.</returns>
    /// <exception cref="RolaWriteError">The native call answered with a failure this takes over.</exception>
    public static nint Plain(Func<nint, RorolalaResult> call, nint request)
    {
        ArgumentNullException.ThrowIfNull(call);

        return Payload(call(request));
    }

    /// <summary>The handles a call runs with, making either one a caller left out.</summary>
    /// <param name="progress">The caller's progress, or nothing.</param>
    /// <param name="cancellation">The caller's cancellation, or nothing.</param>
    /// <returns>The two handles and the ones this made and must give back.</returns>
    public static (
        Progress Watching,
        Cancellation Stopping,
        Progress? OwnedProgress,
        Cancellation? OwnedCancellation
    ) Handles(Progress? progress, Cancellation? cancellation)
    {
        var ownedProgress = progress is null ? Progress.New() : null;
        var watching = progress ?? ownedProgress!;
        var ownedCancellation = cancellation is null ? Cancellation.New() : null;
        var stopping = cancellation ?? ownedCancellation!;

        // The native side refuses a null handle rather than reading it as "no progress", so a
        // caller that asked for neither still hands in a real one it never reads back.
        return (watching, stopping, ownedProgress, ownedCancellation);
    }

    /// <summary>The payload of an answer, or the failure it answered with.</summary>
    /// <param name="answer">The native answer.</param>
    /// <returns>The payload handle.</returns>
    /// <exception cref="RolaWriteError">The answer is a failure.</exception>
    public static nint Payload(RorolalaResult answer)
    {
        if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
        {
            throw WriteResults.From(answer.payload);
        }

        return answer.payload;
    }
}
