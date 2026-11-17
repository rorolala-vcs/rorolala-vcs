using System.Threading;

namespace RolaSharp.V2;

/// <summary>
/// A scope a caller loads and then reads: what it is, how it loads, and what it then answers.
/// </summary>
/// <remarks>
/// A <see cref="Scoop"/> owns the identity of the thing it names, and nothing about it is read until
/// <see cref="Read()"/> or <see cref="ReadAsync"/>. Reading validates the identity immediately before
/// the load, so the window between "is this there" and "read it" is as short as the call; the
/// validation does not try to close it, and <see cref="NoCheck"/> is how a caller that has just
/// checked says not to check again.
/// </remarks>
public abstract class Scoop
{
    /// <summary>Whether a read skips the validation before it.</summary>
    /// <remarks>
    /// It skips only the cheap local check this side makes. The native side still validates what it
    /// reads, because that is where a path that moved between the check and the read is caught.
    /// </remarks>
    public bool NoCheck { get; set; }

    /// <summary>Whether a read has completed.</summary>
    public bool Loaded { get; protected set; }

    /// <summary>Loads what this scope names.</summary>
    /// <exception cref="RolaReadError">The scope is not there, or what is there did not read.</exception>
    public void Read() => Read(CancellationToken.None);

    /// <summary>Loads what this scope names, stopping when the caller asks it to.</summary>
    /// <param name="cancellationToken">The caller's token.</param>
    /// <exception cref="RolaReadError">The scope is not there, what is there did not read, or the caller stopped it.</exception>
    public void Read(CancellationToken cancellationToken)
    {
        try
        {
            if (!NoCheck)
            {
                IsValid();
            }

            cancellationToken.ThrowIfCancellationRequested();
            Load(cancellationToken);
            cancellationToken.ThrowIfCancellationRequested();
            Loaded = true;
        }
        catch (OperationCanceledException)
        {
            throw new RolaReadError(RolaReadErrorKind.Cancelled);
        }
    }

    /// <summary>Loads what this scope names on the thread pool.</summary>
    /// <param name="cancellationToken">The caller's token.</param>
    /// <returns>The read, as a task.</returns>
    public Task ReadAsync(CancellationToken cancellationToken = default) =>
        Task.Run(() => Read(cancellationToken), CancellationToken.None);

    /// <summary>Validates the identity of this scope without reading it.</summary>
    /// <exception cref="RolaReadError">The scope is not there.</exception>
    protected abstract void IsValid();

    /// <summary>Reads what this scope names into itself.</summary>
    /// <param name="cancellationToken">The caller's token.</param>
    protected abstract void Load(CancellationToken cancellationToken);
}
