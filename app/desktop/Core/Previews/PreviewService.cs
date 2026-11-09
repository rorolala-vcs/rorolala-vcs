using System.Collections.Concurrent;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Previews;

/// <summary>
/// The program's previews: the system's sources before the plugins', with the answers kept for a while.
/// </summary>
/// <remarks>
/// A preview costs a read and a decode, so a request that has been answered is kept by path, write time and size:
/// a file that changes gets a new key rather than a stale picture. A request that no source can answer is kept as
/// nothing for the same reason, so a text file is not asked about on every frame it is scrolled past.
/// <para>
/// The sources are asked in the order they were registered, the host's own before any plugin's. A source that
/// throws is a source with no answer rather than a failed preview: the next one is asked, and a tile that no source
/// can draw keeps its type icon.
/// </para>
/// </remarks>
public sealed class PreviewService : IPreviewService, IPreviewRegistry, IDisposable
{
    /// <summary>How many answers are kept before the oldest are dropped.</summary>
    private const int Kept = 256;

    /// <summary>How long one source may take before the host stops waiting for it.</summary>
    private static readonly TimeSpan Patience = TimeSpan.FromSeconds(10);

    /// <summary>How long a preview batch may stay quiet before its run ends.</summary>
    private static readonly TimeSpan Settling = TimeSpan.FromMilliseconds(500);

    /// <summary>The sources, the system's first, in the order they are asked.</summary>
    private readonly List<IPreviewSource> _sources = [];

    /// <summary>What has been answered, by path and the file it was answered for.</summary>
    private readonly ConcurrentDictionary<PreviewKey, Lazy<Task<PreviewImage?>>> _cache = new();

    /// <summary>How many previews may be made at once.</summary>
    private readonly SemaphoreSlim _workers = new(Math.Clamp(Environment.ProcessorCount / 2, 1, 4));

    /// <summary>Where a source that could not answer is said.</summary>
    private readonly ILog _log;

    /// <summary>What a batch is reported to.</summary>
    private readonly IProgressHub _progress;

    /// <summary>The run a batch is reporting to, while one is.</summary>
    private IProgressRun? _run;

    /// <summary>The task in that run.</summary>
    private ProgressTask? _task;

    /// <summary>How many previews the batch has finished.</summary>
    private int _done;

    /// <summary>Cancels the wait that would end the run.</summary>
    private CancellationTokenSource? _settling;

    /// <summary>Makes the service over its system sources.</summary>
    /// <param name="log">Where a source that could not answer is said.</param>
    /// <param name="progress">What a batch is reported to.</param>
    /// <param name="system">The host's own sources, asked before any plugin's.</param>
    public PreviewService(ILog log, IProgressHub progress, params IPreviewSource[] system)
    {
        ArgumentNullException.ThrowIfNull(system);

        _log = log;
        _progress = progress;

        foreach (var source in system)
        {
            Register(source);
        }
    }

    /// <summary>Adds the host's own sources, which are asked before any plugin's.</summary>
    /// <param name="sources">The sources to add.</param>
    public void RegisterSystem(params IPreviewSource[] sources)
    {
        ArgumentNullException.ThrowIfNull(sources);

        foreach (var source in sources)
        {
            Register(source);
        }
    }

    /// <inheritdoc />
    public void Register(IPreviewSource source)
    {
        ArgumentNullException.ThrowIfNull(source);

        if (_sources.Any(other => string.Equals(other.Id, source.Id, StringComparison.Ordinal)))
        {
            return;
        }

        _sources.Add(source);
    }

    /// <inheritdoc />
    public async Task<PreviewImage?> Get(PreviewRequest request, CancellationToken cancel)
    {
        ArgumentNullException.ThrowIfNull(request);

        var key = Key(request);

        if (!_cache.TryGetValue(key, out var lazy))
        {
            // An entry over the limit is dropped whole rather than by a second clock: the cache is a convenience,
            // and a preview that has to be made again is a preview, while a second policy to keep in step is not.
            if (_cache.Count >= Kept)
            {
                _cache.Clear();
            }

            lazy = _cache.GetOrAdd(
                key,
                _ => new Lazy<Task<PreviewImage?>>(
                    () => Render(request),
                    LazyThreadSafetyMode.ExecutionAndPublication
                )
            );

            Started();
        }

        try
        {
            return await lazy.Value.WaitAsync(cancel).ConfigureAwait(false);
        }
        catch (OperationCanceledException)
        {
            return null;
        }
    }

    /// <inheritdoc />
    public void Forget(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        foreach (var key in _cache.Keys.Where(key => string.Equals(key.Path, path, StringComparison.Ordinal)))
        {
            _cache.TryRemove(key, out _);
        }
    }

    /// <summary>Asks the sources in order, and returns the first answer.</summary>
    /// <param name="request">The request.</param>
    /// <returns>The preview, or nothing.</returns>
    private async Task<PreviewImage?> Render(PreviewRequest request)
    {
        foreach (var source in _sources)
        {
            if (!source.Cares(request))
            {
                continue;
            }

            await _workers.WaitAsync().ConfigureAwait(false);

            try
            {
                var image = await source
                    .RenderAsync(request, CancellationToken.None)
                    .AsTask()
                    .WaitAsync(Patience)
                    .ConfigureAwait(false);

                if (image is not null)
                {
                    Finished();

                    return image;
                }
            }
            catch (OperationCanceledException)
            {
                // A source that gave up is a source with no answer, and the next one may have one.
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                _log.Debug($"the preview source `{source.Id}` could not answer for `{request.Path}`: {error.Message}");
            }
            finally
            {
                _workers.Release();
            }
        }

        Finished();

        return null;
    }

    /// <summary>Says that a batch has begun, and opens its run and task once.</summary>
    private void Started()
    {
        if (_run is not null)
        {
            return;
        }

        _run = _progress.Begin("preview.generate", null);
        _task = _run.Records.Open("previews");
        _done = 0;

        _run.Touch();
    }

    /// <summary>Says that one preview of the batch is done, and ends the run once the batch has settled.</summary>
    private void Finished()
    {
        var task = Interlocked.Increment(ref _done);

        if (_task is { } counted)
        {
            counted.Advance(task);
        }

        _run?.Touch();

        _settling?.Cancel();
        _settling?.Dispose();
        _settling = new CancellationTokenSource();

        var token = _settling.Token;

        _ = Task.Run(async () =>
        {
            try
            {
                await Task.Delay(Settling, token).ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                return;
            }

            if (_task is { } finished)
            {
                finished.Finish();
            }

            _run?.Touch();
            _run?.End();
            _run = null;
            _task = null;
            _done = 0;
        });
    }

    /// <summary>The key an answer is kept under: the path, the file it was read from, and the size.</summary>
    /// <param name="request">The request.</param>
    /// <returns>The key.</returns>
    private static PreviewKey Key(PreviewRequest request)
    {
        var file = new FileInfo(request.Path);

        return new PreviewKey(
            request.Path,
            file.Exists ? file.LastWriteTimeUtc.Ticks : 0,
            file.Exists ? file.Length : 0,
            request.Size.Width,
            request.Size.Height
        );
    }

    /// <summary>Stops the workers and the wait that ends a batch.</summary>
    public void Dispose()
    {
        _settling?.Cancel();
        _settling?.Dispose();
        _workers.Dispose();
    }

    /// <summary>What one answer is kept under.</summary>
    /// <param name="Path">The file.</param>
    /// <param name="WriteTicks">When it was last written.</param>
    /// <param name="Length">How long it is.</param>
    /// <param name="Width">The width asked for.</param>
    /// <param name="Height">The height asked for.</param>
    private sealed record PreviewKey(string Path, long WriteTicks, long Length, int Width, int Height);
}
