using System.Numerics;
using Avalonia.Media.Imaging;

namespace SightPlugin;

/// <summary>What one preview came to.</summary>
public abstract record SightDocument;

/// <summary>A file read as lines of text.</summary>
/// <param name="Lines">The lines, as they are written.</param>
/// <param name="Truncated">Whether the file was longer than what was read.</param>
public sealed record SightTextDocument(IReadOnlyList<string> Lines, bool Truncated)
    : SightDocument;

/// <summary>A file decoded as a picture.</summary>
/// <param name="Picture">The picture, made on the UI thread.</param>
/// <param name="Format">What the extension calls the format, in capitals.</param>
public sealed record SightImageDocument(Bitmap Picture, string Format) : SightDocument;

/// <summary>What one line of a diff does.</summary>
public enum SightDiffKind
{
    /// <summary>The line is in both files.</summary>
    Context,

    /// <summary>The line is only in the second file.</summary>
    Added,

    /// <summary>The line is only in the first file.</summary>
    Removed,

    /// <summary>The line names the range that follows.</summary>
    Hunk,
}

/// <summary>One line of a unified diff.</summary>
/// <param name="Kind">What the line does.</param>
/// <param name="Text">The line itself, or the range for a hunk header.</param>
public sealed record SightDiffLine(SightDiffKind Kind, string Text);

/// <summary>Two text files compared line by line.</summary>
/// <param name="Lines">The diff, hunk headers included.</param>
/// <param name="Added">How many lines the second file adds.</param>
/// <param name="Removed">How many lines the first file has and the second does not.</param>
public sealed record SightDiffDocument(IReadOnlyList<SightDiffLine> Lines, int Added, int Removed)
    : SightDocument;

/// <summary>One vertex of a model.</summary>
/// <param name="X">Its x.</param>
/// <param name="Y">Its y.</param>
/// <param name="Z">Its z.</param>
public readonly record struct SightVertex(float X, float Y, float Z);

/// <summary>A model as triangles over a vertex list.</summary>
/// <param name="Vertices">Every vertex, in file order.</param>
/// <param name="Indices">Three indexes a triangle, in file order.</param>
public sealed record SightMesh(IReadOnlyList<SightVertex> Vertices, IReadOnlyList<int> Indices)
{
    /// <summary>How many triangles the mesh holds.</summary>
    public int TriangleCount => Indices.Count / 3;
}

/// <summary>A model file read as a mesh.</summary>
/// <param name="Mesh">The triangles.</param>
/// <param name="Format">What the extension calls the format, in capitals.</param>
public sealed record SightModelDocument(SightMesh Mesh, string Format) : SightDocument;

/// <summary>A PSD file read from its header, and its merged composite when it is plain.</summary>
/// <param name="Width">How many pixels wide it is.</param>
/// <param name="Height">How many pixels tall it is.</param>
/// <param name="Channels">How many channels it holds.</param>
/// <param name="Depth">How many bits a channel holds.</param>
/// <param name="Mode">Colour mode, as the header names it.</param>
/// <param name="Picture">The merged composite, when it was decoded, or nothing.</param>
public sealed record SightPsdDocument(
    int Width,
    int Height,
    int Channels,
    int Depth,
    string Mode,
    Bitmap? Picture
) : SightDocument;

/// <summary>A file whose size passes the limit a previewer reads under.</summary>
/// <param name="Limit">The limit, in bytes.</param>
public sealed record SightTooLargeDocument(long Limit) : SightDocument;

/// <summary>A model vertex, kept as the vector type the renderer works in.</summary>
internal static class SightVectors
{
    /// <summary>The vertex as a vector.</summary>
    /// <param name="vertex">The vertex.</param>
    /// <returns>The vector.</returns>
    public static Vector3 Of(SightVertex vertex) => new(vertex.X, vertex.Y, vertex.Z);
}
