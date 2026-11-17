namespace RolaSharp.V2;

/// <summary>One entry of a Layout: the path it is at and what it records.</summary>
/// <param name="Path">The path it is at, <c>.</c> when it is at none.</param>
/// <param name="Uuid">The Uuid it is known by.</param>
/// <param name="Version">The version it is at, as hex.</param>
/// <param name="Owner">The account that holds it, empty when none does.</param>
/// <param name="Description">What it says about itself.</param>
/// <param name="Deprecated">Whether the Vault has moved it out of the work.</param>
public sealed record LayoutEntry(
    string Path,
    string Uuid,
    string Version,
    string Owner,
    string Description,
    bool Deprecated
);
