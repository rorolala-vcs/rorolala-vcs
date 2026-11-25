using FileSystemPlugin;
using RorolalaDesktop.Contract;

namespace SightPlugin;

/// <summary>
/// What the dock is looking at: the chosen entry, the previewer that answers for it, and the read
/// that is in flight.
/// </summary>
/// <remarks>
/// Every answer is reached from the File System plugin's own choice, never from a listing of this
/// plugin's: <see cref="ChosenFiles"/> names the comparison when two files are chosen and
/// <see cref="ChosenEntry"/> names the file to preview otherwise. A read is cancelled when the
/// choice moves, so a slow file cannot draw over the one chosen after it.
/// </remarks>
internal sealed class SightController
{
    /// <summary>The host, whose log and file changes are read.</summary>
    private readonly IPluginHost _host;

    /// <summary>The dock's own view.</summary>
    private readonly SightView _view;

    /// <summary>What has already been read.</summary>
    private readonly SightCache _cache = new();

    /// <summary>The read in flight, so that a new choice can cancel it.</summary>
    private CancellationTokenSource? _reading;

    /// <summary>Whether the controller is following the File System's choice.</summary>
    private bool _attached;

    /// <summary>The previewer the user forced, or nothing for an automatic choice.</summary>
    private string? _forced;

    /// <summary>Makes a controller over one view.</summary>
    /// <param name="host">The host.</param>
    /// <param name="view">The view it draws into.</param>
    public SightController(IPluginHost host, SightView view)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(view);

        _host = host;
        _view = view;
    }

    /// <summary>The previewer the user forced, or nothing for an automatic choice.</summary>
    public string? Forced => _forced;

    /// <summary>Starts following the choice, and reads what is chosen now.</summary>
    public void Attach()
    {
        if (_attached)
        {
            return;
        }

        _attached = true;
        ChosenFiles.Changed += Reload;
        ChosenEntry.Changed += Reload;
        _host.Files.ChangedIn += Forgot;
        Reload();
    }

    /// <summary>Stops following the choice, and lets go of what was read.</summary>
    public void Detach()
    {
        if (!_attached)
        {
            return;
        }

        _attached = false;
        ChosenFiles.Changed -= Reload;
        ChosenEntry.Changed -= Reload;
        _host.Files.ChangedIn -= Forgot;
        _reading?.Cancel();
        _reading?.Dispose();
        _reading = null;
        _cache.Clear();
    }

    /// <summary>Forces one previewer, or hands the choice back to the automatic one.</summary>
    /// <param name="id">The previewer's stable id, or nothing for Auto.</param>
    public void Choose(string? id)
    {
        _forced = string.IsNullOrEmpty(id) ? null : id;
        Reload();
    }

    /// <summary>Reads the choice again, cancelling whatever was being read.</summary>
    private void Reload()
    {
        if (!_attached)
        {
            return;
        }

        _reading?.Cancel();
        _reading = null;

        if (Files() is not { } files)
        {
            _view.RefreshPreviewers(null, null, _forced);
            _view.ShowEmpty();

            return;
        }

        _view.RefreshPreviewers(files.Primary, files.Second, _forced);

        var choice = _forced ?? "auto";

        if (_cache.TryGet(files.Primary, files.Second, choice, out var kept) && kept is not null)
        {
            Draw(files.Primary, files.Second, kept, null);

            return;
        }

        var candidates = Candidates(files.Primary, files.Second);

        if (candidates.Count == 0)
        {
            _view.ShowUnsupported();

            return;
        }

        var reading = new CancellationTokenSource();
        _reading = reading;
        _view.ShowLoading();
        _ = Read(files.Primary, files.Second, candidates, choice, reading);
    }

    /// <summary>The previewers that may answer for these files, in the order they are tried.</summary>
    /// <param name="primary">The file chosen last.</param>
    /// <param name="second">The other chosen file, when two are chosen.</param>
    /// <returns>The previewers.</returns>
    internal IReadOnlyList<ISightPreviewer> Candidates(SightFile primary, SightFile? second)
    {
        if (_forced is { } id && SightPreviewers.ById(id) is { } forced)
        {
            return forced.Cares(primary, second) ? [forced] : [];
        }

        return SightPreviewers.Ordered(primary, second);
    }

    /// <summary>Reads through the previewers in turn until one answers.</summary>
    private async Task Read(
        SightFile primary,
        SightFile? second,
        IReadOnlyList<ISightPreviewer> candidates,
        string choice,
        CancellationTokenSource reading
    )
    {
        var cancel = reading.Token;

        try
        {
            foreach (var previewer in candidates)
            {
                var document = await previewer
                    .OpenAsync(new SightOpenRequest(primary, second, _host), cancel)
                    .ConfigureAwait(true);

                if (cancel.IsCancellationRequested)
                {
                    return;
                }

                if (document is null)
                {
                    continue;
                }

                if (document is SightTooLargeDocument tooLarge)
                {
                    _view.ShowTooLarge(tooLarge.Limit);

                    return;
                }

                if (document is not (SightImageDocument or SightPsdDocument))
                {
                    _cache.Keep(primary, second, choice, document);
                }

                Draw(primary, second, document, previewer);

                return;
            }

            _view.ShowUnsupported();
        }
        catch (OperationCanceledException)
        {
            // A newer choice took the view, or the dock was closed.
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _host.Log.Warn($"the preview of `{primary.Path}` could not be made: {error.Message}");
            _view.ShowError();
        }
        finally
        {
            if (ReferenceEquals(_reading, reading))
            {
                _reading = null;
            }

            reading.Dispose();
        }
    }

    /// <summary>Draws a document and the metadata the registered providers add to it.</summary>
    private void Draw(
        SightFile primary,
        SightFile? second,
        SightDocument document,
        ISightPreviewer? previewer
    )
    {
        var metadata = SightMetadataProviders.Read(
            new SightMetadataContext(primary, second, document, previewer)
        );

        _view.ShowDocument(document, metadata, previewer);
    }

    /// <summary>Lets go of what was read from a directory, and reads the choice again when it is in it.</summary>
    /// <param name="directory">The directory whose files may have changed.</param>
    private void Forgot(string directory)
    {
        _cache.ForgetDirectory(directory);

        if (Files() is { } files && InDirectory(files.Primary.Path, directory))
        {
            Reload();
        }
    }

    /// <summary>Whether a path sits directly in a directory.</summary>
    private static bool InDirectory(string path, string directory)
    {
        var at = path.LastIndexOfAny(['/', '\\']);

        return at > 0 && string.Equals(path[..at], directory, StringComparison.Ordinal);
    }

    /// <summary>The files the File System's choice names, or nothing when it names no file.</summary>
    private static (SightFile Primary, SightFile? Second)? Files()
    {
        var chosen = ChosenFiles.Current;

        if (chosen.Count == 2)
        {
            return (SightFile.Read(chosen[0].Path), SightFile.Read(chosen[1].Path));
        }

        if (ChosenEntry.Current is { Kind: EntryKind.File } entry)
        {
            return (SightFile.Read(entry.Path), null);
        }

        return null;
    }
}
