using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The host's open pipeline: which hooks run, in what order, and what a refusal does.
/// </summary>
/// <remarks>
/// The pipeline is what a plugin's open check is reached through, so what is checked here is the
/// contract's promise to a plugin: a rejection ends the pipeline, a hook that throws is skipped, and
/// the notification stage runs only after the open.
/// </remarks>
public sealed class OpenPipelineTests
{
    /// <summary>A hook whose answer the test decides.</summary>
    private sealed class Hook(OpenStage stage, Func<OpenRequest, Task<OpenRequest?>> answer)
        : IOpenHook
    {
        /// <inheritdoc />
        public OpenStage Stage { get; } = stage;

        /// <inheritdoc />
        public Task<OpenRequest?> OnOpenAsync(OpenRequest request) => answer(request);
    }

    /// <summary>A request for one file.</summary>
    private static OpenRequest Request() =>
        new() { Target = new Entry("/work/hero.psd", EntryKind.File) };

    /// <summary>Builds a pipeline and the registry its hooks are added to.</summary>
    /// <param name="registry">The registry the pipeline reads.</param>
    /// <returns>The pipeline.</returns>
    private static OpenPipeline Pipeline(out OpenHookRegistry registry)
    {
        registry = new OpenHookRegistry();

        return new OpenPipeline(registry, new RecordingLog());
    }

    /// <summary>A hook that answers nothing stops the stages after it.</summary>
    [Fact]
    public async Task ARejectingHookStopsTheStagesBeforeOpen()
    {
        var pipeline = Pipeline(out var registry);
        var calls = new List<string>();

        registry.Add(
            new PluginId("test.one"),
            1,
            new Hook(
                OpenStage.CanOpen,
                _ =>
                {
                    calls.Add("can");

                    return Task.FromResult<OpenRequest?>(null);
                }
            )
        );
        registry.Add(
            new PluginId("test.two"),
            2,
            new Hook(
                OpenStage.BeforeOpen,
                request =>
                {
                    calls.Add("before");

                    return Task.FromResult<OpenRequest?>(request);
                }
            )
        );

        var answer = await pipeline.ReviewAsync(Request());

        Assert.Null(answer);
        Assert.Equal(["can"], calls);
    }

    /// <summary>A hook that throws is skipped, and the request goes on as it was.</summary>
    [Fact]
    public async Task AThrowingHookIsSkipped()
    {
        var log = new RecordingLog();
        var registry = new OpenHookRegistry();
        var pipeline = new OpenPipeline(registry, log);
        var calls = new List<string>();

        registry.Add(
            new PluginId("test.broken"),
            1,
            new Hook(
                OpenStage.CanOpen,
                _ => throw new InvalidOperationException("the hook is broken")
            )
        );
        registry.Add(
            new PluginId("test.next"),
            2,
            new Hook(
                OpenStage.CanOpen,
                request =>
                {
                    calls.Add("next");

                    return Task.FromResult<OpenRequest?>(request);
                }
            )
        );

        var answer = await pipeline.ReviewAsync(Request());

        Assert.NotNull(answer);
        Assert.Equal(["next"], calls);
        Assert.Single(log.Warnings);
        Assert.Contains("test.broken", log.Warnings[0], StringComparison.Ordinal);
    }

    /// <summary>Hooks of one stage run in plugin load order rather than registration order.</summary>
    [Fact]
    public async Task HooksRunInPluginLoadOrder()
    {
        var pipeline = Pipeline(out var registry);
        var calls = new List<string>();

        registry.Add(
            new PluginId("test.second"),
            2,
            new Hook(
                OpenStage.CanOpen,
                request =>
                {
                    calls.Add("second");

                    return Task.FromResult<OpenRequest?>(request);
                }
            )
        );
        registry.Add(
            new PluginId("test.first"),
            1,
            new Hook(
                OpenStage.CanOpen,
                request =>
                {
                    calls.Add("first");

                    return Task.FromResult<OpenRequest?>(request);
                }
            )
        );

        _ = await pipeline.ReviewAsync(Request());

        Assert.Equal(["first", "second"], calls);
    }

    /// <summary>The notification stage does not run until the open has happened.</summary>
    [Fact]
    public async Task TheNotificationStageRunsOnlyWhenTheOpenHasHappened()
    {
        var pipeline = Pipeline(out var registry);
        var calls = new List<string>();

        registry.Add(
            new PluginId("test.after"),
            1,
            new Hook(
                OpenStage.AfterOpen,
                _ =>
                {
                    calls.Add("after");

                    return Task.FromResult<OpenRequest?>(null);
                }
            )
        );

        var request = Request();

        _ = await pipeline.ReviewAsync(request);
        Assert.Empty(calls);

        await pipeline.OpenedAsync(request);
        Assert.Equal(["after"], calls);
    }
}
