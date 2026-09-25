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
    /// </remarks>
    /// <param name="request">The request as its predecessor left it.</param>
    /// <returns>The request to pass on, or nothing to reject.</returns>
    OpenRequest? OnOpen(OpenRequest request);
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
