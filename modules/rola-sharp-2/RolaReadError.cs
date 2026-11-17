using System.Diagnostics.CodeAnalysis;

namespace RolaSharp.V2;

/// <summary>Why a read did not produce what the caller asked for.</summary>
public enum RolaReadErrorKind
{
    /// <summary>The path or name the caller gave is not one a read accepts.</summary>
    InvalidArgument,

    /// <summary>No Workspace holds the directory.</summary>
    NoWorkspace,

    /// <summary>No Vault holds the directory.</summary>
    NoVault,

    /// <summary>The Workspace is working in no Layout.</summary>
    NoLayout,

    /// <summary>The copy of a Vault's Layout a name asked for was never fetched here.</summary>
    NotCached,

    /// <summary>The name is not one a place has a Layout under.</summary>
    Missing,

    /// <summary>The store, index or object the read names is not there.</summary>
    NotFound,

    /// <summary>The read could be made, but the thing it was about is not available.</summary>
    Unavailable,

    /// <summary>What is there could not be read back.</summary>
    Unreadable,

    /// <summary>The caller asked the read to stop.</summary>
    Cancelled,

    /// <summary>A string or document did not cross the boundary as it was written.</summary>
    Marshalling,

    /// <summary>The native side refused for a reason of its own.</summary>
    Native,
}

/// <summary>What a read failed with, as a value a caller can catch.</summary>
[SuppressMessage(
    "Naming",
    "CA1710:Identifiers should have correct suffix",
    Justification = "the issue names the type RolaReadError, and the C ABI error carries the same name"
)]
public sealed class RolaReadError : Exception
{
    /// <summary>Makes a read failure.</summary>
    /// <param name="kind">Why the read failed.</param>
    /// <param name="path">What the read was about, when there is a path to name.</param>
    /// <param name="reason">What the native side said, when it said anything.</param>
    public RolaReadError(RolaReadErrorKind kind, string? path = null, string? reason = null)
        : base(reason ?? kind.ToString())
    {
        Kind = kind;
        Path = path;
    }

    /// <summary>Why the read failed.</summary>
    public RolaReadErrorKind Kind { get; }

    /// <summary>What the read was about, when there is a path to name.</summary>
    public string? Path { get; }
}
