using System.Globalization;

namespace SightPlugin;

/// <summary>The previewer that reads an ASCII OBJ file as a triangle mesh.</summary>
/// <remarks>
/// It reads what a wireframe needs and no more: `v` and `f` lines, with quads split into triangles
/// and the first index of a `v/vt/vn` triple taken. Normals, texture coordinates, materials and
/// groups are not read; the renderer shades each triangle from its own normal.
/// </remarks>
internal sealed class SightModel : ISightPreviewer
{
    /// <inheritdoc />
    public string Id => "sight.model";

    /// <inheritdoc />
    public string NameKey => "rorolala_sight.previewer.model";

    /// <inheritdoc />
    public string? IconKey => Icons.Cube;

    /// <inheritdoc />
    public int Rank => 55;

    /// <inheritdoc />
    public bool Cares(SightFile primary, SightFile? second) =>
        second is null && string.Equals(primary.Extension, "obj", StringComparison.OrdinalIgnoreCase);

    /// <inheritdoc />
    public async ValueTask<SightDocument?> OpenAsync(
        SightOpenRequest request,
        CancellationToken cancel
    )
    {
        ArgumentNullException.ThrowIfNull(request);

        var limit = SightSettings.ImageLimitBytes;

        if (request.Primary.Length > limit)
        {
            return new SightTooLargeDocument(limit);
        }

        var lines = await File.ReadAllLinesAsync(request.Primary.Path, cancel)
            .ConfigureAwait(false);
        var mesh = Read(lines);

        cancel.ThrowIfCancellationRequested();

        return mesh is null ? null : new SightModelDocument(mesh, "OBJ");
    }

    /// <summary>Reads the vertices and triangles out of OBJ lines.</summary>
    /// <param name="lines">The file, line by line.</param>
    /// <returns>The mesh, or nothing when the file holds no triangle.</returns>
    internal static SightMesh? Read(IReadOnlyList<string> lines)
    {
        ArgumentNullException.ThrowIfNull(lines);

        var vertices = new List<SightVertex>();
        var indices = new List<int>();

        foreach (var line in lines)
        {
            var text = line.Trim();

            if (text.StartsWith("v ", StringComparison.Ordinal))
            {
                var parts = Words(text);

                if (
                    parts.Length >= 4
                    && float.TryParse(parts[1], NumberStyles.Float, CultureInfo.InvariantCulture, out var x)
                    && float.TryParse(parts[2], NumberStyles.Float, CultureInfo.InvariantCulture, out var y)
                    && float.TryParse(parts[3], NumberStyles.Float, CultureInfo.InvariantCulture, out var z)
                )
                {
                    vertices.Add(new SightVertex(x, y, z));
                }

                continue;
            }

            if (!text.StartsWith("f ", StringComparison.Ordinal))
            {
                continue;
            }

            var face = Words(text);
            var polygon = new List<int>(face.Length - 1);

            for (var at = 1; at < face.Length; at++)
            {
                var head = face[at].Split('/')[0];

                if (!int.TryParse(head, NumberStyles.Integer, CultureInfo.InvariantCulture, out var index))
                {
                    polygon.Clear();

                    break;
                }

                index = index < 0 ? vertices.Count + index : index - 1;

                if (index < 0 || index >= vertices.Count)
                {
                    polygon.Clear();

                    break;
                }

                polygon.Add(index);
            }

            for (var at = 2; at < polygon.Count; at++)
            {
                indices.Add(polygon[0]);
                indices.Add(polygon[at - 1]);
                indices.Add(polygon[at]);
            }
        }

        return indices.Count == 0 ? null : new SightMesh(vertices, indices);
    }

    /// <summary>The words of one line, empty ones left out.</summary>
    /// <param name="line">The line.</param>
    /// <returns>The words.</returns>
    private static string[] Words(string line) =>
        line.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries);
}
