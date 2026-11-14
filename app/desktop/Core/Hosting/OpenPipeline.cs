using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// The host's open pipeline: the stages an open passes through before and after it happens.
/// </summary>
/// <remarks>
/// The hooks are a plugin's contribution and the order is the host's. Each stage runs every hook that
/// joined it, in plugin load order, and a hook that answers nothing or rejects ends the stage at once:
/// what a hook refuses is not something a later hook can overturn.
/// <para>
/// A hook that throws is skipped rather than allowed to stop the open, which is the rule the contract
/// states: a broken plugin must not be the reason a file cannot be opened. The failure is written to
/// the log so that the plugin is not silent about it.
/// </para>
/// </remarks>
internal sealed class OpenPipeline : IOpenPipeline
{
    /// <summary>The stages that run before an open, in the order they run.</summary>
    private static readonly OpenStage[] Before =
    [
        OpenStage.CanOpen,
        OpenStage.BeforeOpen,
    ];

    /// <summary>What the plugins contributed.</summary>
    private readonly OpenHookRegistry _hooks;

    /// <summary>Where a hook that failed says so.</summary>
    private readonly ILog _log;

    /// <summary>Makes the pipeline over the hooks and the log.</summary>
    /// <param name="hooks">The hooks the plugins added.</param>
    /// <param name="log">Where a hook that failed is written.</param>
    public OpenPipeline(OpenHookRegistry hooks, ILog log)
    {
        ArgumentNullException.ThrowIfNull(hooks);
        ArgumentNullException.ThrowIfNull(log);

        _hooks = hooks;
        _log = log;
    }

    /// <inheritdoc />
    public async Task<OpenRequest?> ReviewAsync(OpenRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        foreach (var stage in Before)
        {
            foreach (var owned in _hooks.Hooks(stage))
            {
                OpenRequest? answer;

                try
                {
                    answer = await owned.Value.OnOpenAsync(request);
                }
                catch (Exception error) when (error is not OutOfMemoryException)
                {
                    Warn(owned, stage, error);

                    continue;
                }

                if (answer is null)
                {
                    return null;
                }

                request = answer;

                if (request.Verdict is OpenVerdict.Reject)
                {
                    return null;
                }
            }
        }

        return request;
    }

    /// <inheritdoc />
    public async Task OpenedAsync(OpenRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        foreach (var owned in _hooks.Hooks(OpenStage.AfterOpen))
        {
            try
            {
                // The answer is ignored: this stage is a notification, and what it would reject has
                // already happened.
                await owned.Value.OnOpenAsync(request);
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                Warn(owned, OpenStage.AfterOpen, error);
            }
        }
    }

    /// <summary>Says that one plugin's hook failed, naming the plugin and the stage.</summary>
    /// <param name="owned">The hook that failed and the plugin that added it.</param>
    /// <param name="stage">Which stage it was running in.</param>
    /// <param name="error">What it failed with.</param>
    private void Warn(Owned<IOpenHook> owned, OpenStage stage, Exception error) =>
        _log.Warn(
            $"the open hook of `{owned.Owner.Value}` failed in `{stage}`: {error.Message}"
        );
}
