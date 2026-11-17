namespace RolaSharp.V2;

/// <summary>The variants around the entries a request names, as a scope.</summary>
public sealed class VariantGraphScoop : Scoop, IDisposable
{
    private readonly string _root;
    private readonly VariantGraphRequest? _request;
    private VariantGraphReading? _reading;

    /// <summary>Makes a scope over the variant graph of a directory.</summary>
    /// <param name="directory">Any directory inside the Workspace.</param>
    /// <param name="request">The window to draw, or nothing for the default one.</param>
    public VariantGraphScoop(string directory, VariantGraphRequest? request = null)
    {
        _root = Path.GetFullPath(directory);
        _request = request;
    }

    /// <summary>The reading, once it is made.</summary>
    public VariantGraphReading Reading
    {
        get
        {
            if (!Loaded || _reading is null)
            {
                throw new InvalidOperationException("call Read first");
            }

            return _reading;
        }
    }

    /// <inheritdoc/>
    protected override void IsValid()
    {
        if (!Directory.Exists(_root))
        {
            throw new RolaReadError(
                RolaReadErrorKind.InvalidArgument,
                _root,
                "the graph's directory is not there"
            );
        }
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        Dispose();
        cancellationToken.ThrowIfCancellationRequested();

        try
        {
            _reading =
                VariantGraphReading.Locate(_root, _request)
                ?? throw new RolaReadError(
                    RolaReadErrorKind.Unavailable,
                    _root,
                    "the Workspace has no variant graph to draw"
                );
        }
        catch (InvalidOperationException error)
        {
            throw new RolaReadError(
                RolaReadErrorKind.Unreadable,
                _root,
                error.Message
            );
        }
    }

    /// <summary>Gives the reading back.</summary>
    public void Dispose()
    {
        _reading = null;
        Loaded = false;
        GC.SuppressFinalize(this);
    }
}
