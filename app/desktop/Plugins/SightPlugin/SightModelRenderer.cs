using System.Numerics;

namespace SightPlugin;

/// <summary>
/// Draws a mesh into top-down BGRA bytes: flat-shaded triangles, their edges, and the grid under
/// them.
/// </summary>
/// <remarks>
/// It is a pure function of the mesh, the camera, the size and the grid choice, so a test can read
/// the pixels without a window and the view can run it away from the UI thread. There is no raster
/// backend and no dependency: a triangle is filled with a z-buffer and an edge is a line.
/// </remarks>
internal static class SightModelRenderer
{
    /// <summary>Where the light comes from, in the camera's own space.</summary>
    private static readonly Vector3 Light = Vector3.Normalize(new Vector3(0.35f, 0.55f, 0.75f));

    /// <summary>How bright a triangle facing away from the light still is.</summary>
    private const float Ambient = 0.32f;

    /// <summary>How much of the light's answer a triangle keeps.</summary>
    private const float Direct = 0.68f;

    /// <summary>Draws one view of the mesh.</summary>
    /// <param name="mesh">The mesh.</param>
    /// <param name="yaw">How far it is turned about the vertical axis, in radians.</param>
    /// <param name="pitch">How far it is tilted, in radians.</param>
    /// <param name="distance">How far the camera stands from its centre.</param>
    /// <param name="width">How many pixels wide the picture is.</param>
    /// <param name="height">How many pixels tall the picture is.</param>
    /// <param name="grid">Whether the grid is drawn under it.</param>
    /// <returns>Top-down BGRA bytes, transparent where nothing is drawn.</returns>
    public static byte[] Render(
        SightMesh mesh,
        float yaw,
        float pitch,
        float distance,
        int width,
        int height,
        bool grid
    )
    {
        ArgumentNullException.ThrowIfNull(mesh);

        if (width <= 0 || height <= 0)
        {
            return [];
        }

        var pixels = new byte[checked(width * height * 4)];

        if (mesh.Vertices.Count == 0 || mesh.Indices.Count < 3)
        {
            return pixels;
        }

        var points = Normalize(mesh);
        var camera = new Vector3[points.Length];
        var screen = new Projected[points.Length];

        for (var at = 0; at < points.Length; at++)
        {
            camera[at] = InCamera(points[at], yaw, pitch, distance);
            screen[at] = Project(camera[at], width, height);
        }

        if (grid)
        {
            DrawGrid(pixels, width, height, yaw, pitch, distance);
        }

        var depth = new float[width * height];
        Array.Fill(depth, float.PositiveInfinity);

        var triangles = mesh.TriangleCount;

        for (var at = 0; at < triangles; at++)
        {
            var a = mesh.Indices[at * 3];
            var b = mesh.Indices[(at * 3) + 1];
            var c = mesh.Indices[(at * 3) + 2];

            if (a >= points.Length || b >= points.Length || c >= points.Length)
            {
                continue;
            }

            Fill(pixels, depth, width, height, screen[a], screen[b], screen[c], camera[a], camera[b], camera[c]);
        }

        for (var at = 0; at < triangles; at++)
        {
            var a = mesh.Indices[at * 3];
            var b = mesh.Indices[(at * 3) + 1];
            var c = mesh.Indices[(at * 3) + 2];

            if (a >= points.Length || b >= points.Length || c >= points.Length)
            {
                continue;
            }

            Line(pixels, width, height, screen[a], screen[b]);
            Line(pixels, width, height, screen[b], screen[c]);
            Line(pixels, width, height, screen[c], screen[a]);
        }

        return pixels;
    }

    /// <summary>The mesh's points, centred and scaled so the largest axis is one.</summary>
    /// <param name="mesh">The mesh.</param>
    /// <returns>The points.</returns>
    private static Vector3[] Normalize(SightMesh mesh)
    {
        var min = new Vector3(float.PositiveInfinity);
        var max = new Vector3(float.NegativeInfinity);

        foreach (var vertex in mesh.Vertices)
        {
            var point = SightVectors.Of(vertex);
            min = Vector3.Min(min, point);
            max = Vector3.Max(max, point);
        }

        var centre = (min + max) * 0.5f;
        var extent = MathF.Max(max.X - min.X, MathF.Max(max.Y - min.Y, max.Z - min.Z)) * 0.5f;
        var scale = extent > 0.0001f ? extent : 1f;
        var points = new Vector3[mesh.Vertices.Count];

        for (var at = 0; at < points.Length; at++)
        {
            points[at] = (SightVectors.Of(mesh.Vertices[at]) - centre) / scale;
        }

        return points;
    }

    /// <summary>The point as the camera sees it.</summary>
    /// <param name="point">The point, centred and scaled.</param>
    /// <param name="yaw">The turn about the vertical axis.</param>
    /// <param name="pitch">The tilt.</param>
    /// <param name="distance">How far the camera stands.</param>
    /// <returns>The point in camera space.</returns>
    private static Vector3 InCamera(Vector3 point, float yaw, float pitch, float distance)
    {
        var cosYaw = MathF.Cos(yaw);
        var sinYaw = MathF.Sin(yaw);
        var turned = new Vector3(
            (point.X * cosYaw) - (point.Z * sinYaw),
            point.Y,
            (point.X * sinYaw) + (point.Z * cosYaw)
        );

        var cosPitch = MathF.Cos(pitch);
        var sinPitch = MathF.Sin(pitch);
        var tilted = new Vector3(
            turned.X,
            (turned.Y * cosPitch) - (turned.Z * sinPitch),
            (turned.Y * sinPitch) + (turned.Z * cosPitch)
        );

        return new Vector3(tilted.X, tilted.Y, tilted.Z + distance);
    }

    /// <summary>Projects one camera-space point onto the picture.</summary>
    /// <param name="point">The point in camera space.</param>
    /// <param name="width">How wide the picture is.</param>
    /// <param name="height">How tall it is.</param>
    /// <returns>The point on the picture, and its depth.</returns>
    private static Projected Project(Vector3 point, int width, int height)
    {
        var depth = MathF.Max(point.Z, 0.05f);
        var factor = 1.6f / depth;
        var scale = height * 0.62f;

        return new Projected(
            (width * 0.5f) + (point.X * factor * scale),
            (height * 0.5f) - (point.Y * factor * scale),
            depth
        );
    }

    /// <summary>Fills one triangle, nearest pixels winning.</summary>
    private static void Fill(
        byte[] pixels,
        float[] depth,
        int width,
        int height,
        Projected a,
        Projected b,
        Projected c,
        Vector3 pa,
        Vector3 pb,
        Vector3 pc
    )
    {
        var area = Edge(a, b, c);

        if (MathF.Abs(area) < 0.001f)
        {
            return;
        }

        var left = Math.Clamp((int)MathF.Floor(MathF.Min(a.X, MathF.Min(b.X, c.X))), 0, width - 1);
        var right = Math.Clamp((int)MathF.Ceiling(MathF.Max(a.X, MathF.Max(b.X, c.X))), 0, width - 1);
        var top = Math.Clamp((int)MathF.Floor(MathF.Min(a.Y, MathF.Min(b.Y, c.Y))), 0, height - 1);
        var bottom = Math.Clamp((int)MathF.Ceiling(MathF.Max(a.Y, MathF.Max(b.Y, c.Y))), 0, height - 1);
        var normal = Vector3.Normalize(Vector3.Cross(pb - pa, pc - pa));
        var light = Ambient + (Direct * MathF.Abs(Vector3.Dot(normal, Light)));

        for (var y = top; y <= bottom; y++)
        {
            for (var x = left; x <= right; x++)
            {
                var point = new Vector2(x + 0.5f, y + 0.5f);
                var w0 = Edge(b, c, point);
                var w1 = Edge(c, a, point);
                var w2 = Edge(a, b, point);

                if (!(w0 >= 0 && w1 >= 0 && w2 >= 0) && !(w0 <= 0 && w1 <= 0 && w2 <= 0))
                {
                    continue;
                }

                var sum = w0 + w1 + w2;

                if (MathF.Abs(sum) < 0.0001f)
                {
                    continue;
                }

                var near = ((w0 * a.Depth) + (w1 * b.Depth) + (w2 * c.Depth)) / sum;
                var index = (y * width) + x;

                if (near >= depth[index])
                {
                    continue;
                }

                depth[index] = near;

                var colourAt = index * 4;
                pixels[colourAt] = (byte)(120 * light);
                pixels[colourAt + 1] = (byte)(168 * light);
                pixels[colourAt + 2] = (byte)(205 * light);
                pixels[colourAt + 3] = 255;
            }
        }
    }

    /// <summary>The signed area of a triangle on the picture.</summary>
    private static float Edge(Projected a, Projected b, Projected c) =>
        ((b.X - a.X) * (c.Y - a.Y)) - ((b.Y - a.Y) * (c.X - a.X));

    /// <summary>The signed area of a triangle on the picture, at one point.</summary>
    private static float Edge(Projected a, Projected b, Vector2 point) =>
        ((b.X - a.X) * (point.Y - a.Y)) - ((b.Y - a.Y) * (point.X - a.X));

    /// <summary>Draws one mesh edge.</summary>
    private static void Line(byte[] pixels, int width, int height, Projected a, Projected b)
    {
        var x0 = (int)MathF.Round(a.X);
        var y0 = (int)MathF.Round(a.Y);
        var x1 = (int)MathF.Round(b.X);
        var y1 = (int)MathF.Round(b.Y);
        var dx = Math.Abs(x1 - x0);
        var sx = x0 < x1 ? 1 : -1;
        var dy = -Math.Abs(y1 - y0);
        var sy = y0 < y1 ? 1 : -1;
        var error = dx + dy;

        while (true)
        {
            if (x0 >= 0 && x0 < width && y0 >= 0 && y0 < height)
            {
                var at = ((y0 * width) + x0) * 4;
                pixels[at] = 225;
                pixels[at + 1] = 208;
                pixels[at + 2] = 77;
                pixels[at + 3] = 210;
            }

            if (x0 == x1 && y0 == y1)
            {
                break;
            }

            var twice = 2 * error;

            if (twice >= dy)
            {
                error += dy;
                x0 += sx;
            }

            if (twice <= dx)
            {
                error += dx;
                y0 += sy;
            }
        }
    }

    /// <summary>Draws the grid the model stands on.</summary>
    private static void DrawGrid(
        byte[] pixels,
        int width,
        int height,
        float yaw,
        float pitch,
        float distance
    )
    {
        const float floor = -0.62f;
        const float reach = 1.6f;

        for (var at = -2; at <= 2; at++)
        {
            var line = at * 0.55f;
            var a = Project(
                InCamera(new Vector3(line, floor, -reach), yaw, pitch, distance),
                width,
                height
            );
            var b = Project(
                InCamera(new Vector3(line, floor, reach), yaw, pitch, distance),
                width,
                height
            );
            var c = Project(
                InCamera(new Vector3(-reach, floor, line), yaw, pitch, distance),
                width,
                height
            );
            var d = Project(
                InCamera(new Vector3(reach, floor, line), yaw, pitch, distance),
                width,
                height
            );

            GridLine(pixels, width, height, a, b);
            GridLine(pixels, width, height, c, d);
        }
    }

    /// <summary>Draws one grid line, dimmer than a mesh edge.</summary>
    private static void GridLine(byte[] pixels, int width, int height, Projected a, Projected b)
    {
        var x0 = (int)MathF.Round(a.X);
        var y0 = (int)MathF.Round(a.Y);
        var x1 = (int)MathF.Round(b.X);
        var y1 = (int)MathF.Round(b.Y);
        var dx = Math.Abs(x1 - x0);
        var sx = x0 < x1 ? 1 : -1;
        var dy = -Math.Abs(y1 - y0);
        var sy = y0 < y1 ? 1 : -1;
        var error = dx + dy;

        while (true)
        {
            if (x0 >= 0 && x0 < width && y0 >= 0 && y0 < height)
            {
                var at = ((y0 * width) + x0) * 4;

                if (pixels[at + 3] == 0)
                {
                    pixels[at] = 76;
                    pixels[at + 1] = 66;
                    pixels[at + 2] = 60;
                    pixels[at + 3] = 255;
                }
            }

            if (x0 == x1 && y0 == y1)
            {
                break;
            }

            var twice = 2 * error;

            if (twice >= dy)
            {
                error += dy;
                x0 += sx;
            }

            if (twice <= dx)
            {
                error += dx;
                y0 += sy;
            }
        }
    }

    /// <summary>One point on the picture, with the depth it was projected from.</summary>
    /// <param name="X">The x.</param>
    /// <param name="Y">The y.</param>
    /// <param name="Depth">The camera-space depth.</param>
    private readonly record struct Projected(float X, float Y, float Depth);
}
