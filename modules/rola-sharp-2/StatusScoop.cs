namespace RolaSharp.V2;

/// <summary>How the working tree stands beside the Layout it works from.</summary>
public sealed class StatusScoop : Scoop, IDisposable
{
    private readonly string _root;
    private StatusReading? _status;

    /// <summary>Makes a scope over the status the Workspace holding a directory has.</summary>
    /// <param name="directory">Any directory inside the Workspace.</param>
    public StatusScoop(string directory) => _root = Path.GetFullPath(directory);

    /// <summary>The reading, once it is made.</summary>
    public StatusReading Status
    {
        get
        {
            if (!Loaded || _status is null)
            {
                throw new InvalidOperationException("call Read first");
            }

            return _status;
        }
    }

    /// <inheritdoc/>
    protected override void IsValid()
    {
        if (!Directory.Exists(_root))
        {
            throw new RolaReadError(
                RolaReadErrorKind.NoWorkspace,
                _root,
                "no Workspace holds the directory"
            );
        }
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        Dispose();
        cancellationToken.ThrowIfCancellationRequested();

        _status =
            StatusReading.Locate(_root)
            ?? throw new RolaReadError(
                RolaReadErrorKind.Unavailable,
                _root,
                "the Workspace has no status to read"
            );
    }

    /// <summary>Gives the native reading back.</summary>
    public void Dispose()
    {
        _status?.Dispose();
        _status = null;
        GC.SuppressFinalize(this);
    }
}
