namespace RorolalaDesktop.Contract;

/// <summary>
/// Where in the open pipeline a hook runs.
/// </summary>
public enum OpenStage
{
    /// <summary>Each hook may reject.</summary>
    CanOpen,

    /// <summary>Each hook may transform the request or reject.</summary>
    BeforeOpen,

    /// <summary>Each hook is notified; it cannot reject.</summary>
    AfterOpen,
}

/// <summary>Whether an open proceeds or is refused.</summary>
public enum OpenVerdict
{
    /// <summary>The open proceeds.</summary>
    Continue,

    /// <summary>The open does not happen.</summary>
    Reject,
}

/// <summary>
/// One open, as it passes through the pipeline.
/// </summary>
/// <remarks>
/// A hook receives the request as its predecessor left it, so a request is both what is being opened
/// and the state the hooks pass forward. Rejection is final: a later hook cannot overturn it, and the
/// pipeline stops.
/// </remarks>
public sealed class OpenRequest
{
    /// <summary>The entry being opened.</summary>
    public required Entry Target { get; init; }

    /// <summary>Whether the open proceeds. A hook sets this to reject.</summary>
    public OpenVerdict Verdict { get; set; } = OpenVerdict.Continue;

    /// <summary>An i18n key explaining a rejection, or nothing.</summary>
    public string? RejectReasonKey { get; set; }

    /// <summary>A mutable bag hooks use to pass values forward.</summary>
    public IDictionary<string, object?> State { get; } = new Dictionary<string, object?>();
}

/// <summary>
/// What a plugin contributes to the open pipeline.
/// </summary>
public interface IOpenHook
{
    /// <summary>Which stage this hook runs in.</summary>
    OpenStage Stage { get; }

    /// <summary>
    /// Looks at, or changes, a request.
    /// </summary>
    /// <remarks>
    /// Returning nothing rejects the open, as does setting
    /// <see cref="OpenRequest.Verdict"/> to <see cref="OpenVerdict.Reject"/>. Throwing skips only
    /// this hook: the request passes on exactly as it was received, so a broken hook never blocks
    /// the user and never half-modifies a request. In <see cref="OpenStage.AfterOpen"/> the returned
    /// request is ignored, since the stage is a notification.
    /// <para>
    /// The answer is awaited because a hook may have to ask the user something before it can say
    /// whether the open may go on: a question the host shows, and an answer that takes as long as
    /// the user takes. The open waits for it, which is what makes the question part of the open.
    /// </para>
    /// </remarks>
    /// <param name="request">The request as its predecessor left it.</param>
    /// <returns>The request to pass on, or nothing to reject.</returns>
    Task<OpenRequest?> OnOpenAsync(OpenRequest request);
}

/// <summary>Where a plugin adds open hooks.</summary>
public interface IOpenHookRegistry
{
    /// <summary>
    /// Adds a hook, after the hooks already added.
    /// </summary>
    /// <remarks>Within a stage, hooks run in plugin load order.</remarks>
    /// <param name="hook">The hook to add.</param>
    void Add(IOpenHook hook);
}

/// <summary>
/// The pipeline an open passes through, as the host runs it.
/// </summary>
/// <remarks>
/// A plugin that opens an entry asks here rather than launching it directly, so that every plugin
/// that cares about an open has the same chance to speak. The stages run in the order
/// <see cref="OpenStage"/> declares them, and a rejection ends the pipeline.
/// </remarks>
public interface IOpenPipeline
{
    /// <summary>
    /// Runs the stages that come before an open and answers the request as they left it.
    /// </summary>
    /// <remarks>
    /// The answer is what the caller opens: a hook may have changed the request, and the last hook's
    /// answer is the one that stands. Nothing is raised in <see cref="OpenStage.AfterOpen"/> here,
    /// since the open has not happened yet.
    /// </remarks>
    /// <param name="request">The request to review.</param>
    /// <returns>The request to open, or nothing when a hook rejected it.</returns>
    Task<OpenRequest?> ReviewAsync(OpenRequest request);

    /// <summary>
    /// Tells the hooks that the open has happened.
    /// </summary>
    /// <remarks>
    /// This is the notification stage, so a hook's answer is ignored and no hook can reject the open
    /// that has already been made.
    /// </remarks>
    /// <param name="request">The request that was opened, as it was opened.</param>
    Task OpenedAsync(OpenRequest request);
}
