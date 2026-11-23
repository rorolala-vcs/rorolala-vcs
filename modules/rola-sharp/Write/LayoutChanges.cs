namespace RolaSharp.Write;

/// <summary>Which change a <see cref="LayoutChange"/> names.</summary>
public enum LayoutChangeKind
{
    /// <summary>Makes an entry.</summary>
    CreateEntry,

    /// <summary>Changes what an entry holds.</summary>
    UpdateEntry,

    /// <summary>Removes an entry.</summary>
    RemoveEntry,

    /// <summary>Binds a path.</summary>
    CreatePath,

    /// <summary>Moves a path.</summary>
    MovePath,

    /// <summary>Removes a path.</summary>
    RemovePath,

    /// <summary>Changes who holds an entry.</summary>
    ChangeOwnership,

    /// <summary>Changes an entry's Uuid.</summary>
    ChangeUuid,
}

/// <summary>One change a Layout batch applies.</summary>
/// <remarks>
/// The eight changes share one shape because they travel as one tagged record each; which fields a
/// change reads is what <see cref="Kind"/> says, and the others stay empty.
/// </remarks>
public sealed record LayoutChange
{
    /// <summary>Which change this is.</summary>
    public required LayoutChangeKind Kind { get; init; }

    /// <summary>The entry's Uuid, for the changes that name one.</summary>
    public string Uuid { get; init; } = string.Empty;

    /// <summary>The path, for the changes that name one.</summary>
    public string Path { get; init; } = string.Empty;

    /// <summary>The version the entry holds, for the changes that name one.</summary>
    public string Version { get; init; } = string.Empty;

    /// <summary>The account that holds the entry, or an empty string for none.</summary>
    public string Owner { get; init; } = string.Empty;

    /// <summary>What the entry says about itself, or an empty string for nothing.</summary>
    public string Description { get; init; } = string.Empty;

    /// <summary>`tree` to make a tree entry, or an empty string for a file.</summary>
    public string EntryKind { get; init; } = string.Empty;

    /// <summary>The path a move starts at, or the Uuid one starts from.</summary>
    public string From { get; init; } = string.Empty;

    /// <summary>The path a move ends at, or the Uuid one ends at.</summary>
    public string To { get; init; } = string.Empty;
}
