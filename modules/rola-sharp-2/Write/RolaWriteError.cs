using System.Diagnostics.CodeAnalysis;

namespace RolaSharp.V2.Write;

/// <summary>Why a write did not finish.</summary>
public enum RolaWriteErrorKind
{
    /// <summary>No Workspace holds the directory.</summary>
    NoWorkspace,

    /// <summary>The Workspace works in no Layout.</summary>
    NoLayout,

    /// <summary>No Workspace or Vault holds a store.</summary>
    NoStore,

    /// <summary>No index is there.</summary>
    NoIndex,

    /// <summary>No account is bound, or the account named is not here.</summary>
    NoAccount,

    /// <summary>The Layout tracks no Vault.</summary>
    NoTrack,

    /// <summary>The copy of a Vault's Layout was never fetched here.</summary>
    NotCached,

    /// <summary>The run is offline.</summary>
    Offline,

    /// <summary>Another run holds the lock.</summary>
    Locked,

    /// <summary>A path is not one the write accepts.</summary>
    InvalidPath,

    /// <summary>What the write names is not there.</summary>
    NotFound,

    /// <summary>A precondition does not hold.</summary>
    Precondition,

    /// <summary>Ownership refuses the write.</summary>
    Ownership,

    /// <summary>The caller asked the write to stop.</summary>
    Cancelled,

    /// <summary>An exchange with the Vault failed.</summary>
    Action,

    /// <summary>A Layout could not be read or written.</summary>
    Layout,

    /// <summary>A store could not be read or written.</summary>
    Storage,

    /// <summary>An index could not be read or written.</summary>
    Index,

    /// <summary>A file or a directory failed.</summary>
    Io,

    /// <summary>A JSON document did not read.</summary>
    Json,

    /// <summary>The write stopped for a reason this build does not name.</summary>
    Internal,
}

/// <summary>What a write failed with, as a value a caller can catch.</summary>
[SuppressMessage(
    "Naming",
    "CA1710:Identifiers should have correct suffix",
    Justification = "the issue names the type RolaWriteError, and the C ABI error carries the same name"
)]
public sealed class RolaWriteError : Exception
{
    /// <summary>Makes a write failure.</summary>
    /// <param name="kind">Why the write failed.</param>
    /// <param name="path">What the write was about, when there is a path to name.</param>
    /// <param name="reason">What the native side said, when it said anything.</param>
    public RolaWriteError(RolaWriteErrorKind kind, string? path = null, string? reason = null)
        : base(reason ?? kind.ToString())
    {
        Kind = kind;
        Path = path;
    }

    /// <summary>Why the write failed.</summary>
    public RolaWriteErrorKind Kind { get; }

    /// <summary>What the write was about, when there is a path to name.</summary>
    public string? Path { get; }
}
