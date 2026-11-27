namespace RolaSharp;

using RolaSharp.Write;

/// <summary>The operations a Workspace recorded, newest first.</summary>
/// <remarks>
/// The list is the stack <see cref="HistoryWriter.Undo"/> pops: one file per run under the
/// Workspace's cache, read once and released before the reading answers, so the scope holds no
/// native memory between reads. A file that does not read is skipped and counted rather than failing
/// the list.
/// </remarks>
public sealed class OperationLogScope : Scope, IDisposable
{
    private readonly string _root;
    private IReadOnlyList<RecordedOperation>? _operations;

    /// <summary>Makes a scope over the operations the Workspace holding a directory recorded.</summary>
    /// <param name="directory">Any directory inside the Workspace.</param>
    public OperationLogScope(string directory) => _root = Path.GetFullPath(directory);

    /// <summary>The operations, newest first, once they are read.</summary>
    public IReadOnlyList<RecordedOperation> Operations =>
        Loaded && _operations is not null
            ? _operations
            : throw new InvalidOperationException("call Read first");

    /// <summary>How many log files could not be read and were skipped.</summary>
    public int UnreadableCount { get; private set; }

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

        (_operations, var unreadable) = Histories.Read(_root);
        UnreadableCount = unreadable;
    }

    /// <summary>Gives the reading back.</summary>
    public void Dispose()
    {
        _operations = null;
        UnreadableCount = 0;
        Loaded = false;
        GC.SuppressFinalize(this);
    }
}
