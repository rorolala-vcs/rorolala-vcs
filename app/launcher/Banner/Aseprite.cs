using System.Buffers.Binary;
using System.IO.Compression;

namespace RorolalaLauncher.Banner;

/// <summary>One frame of an <c>.aseprite</c> file, as it is drawn.</summary>
/// <param name="Duration">How long the frame is shown for.</param>
/// <param name="Pixels">The frame's pixels, four bytes to a pixel, in the order a bitmap wants them.</param>
internal sealed record AsepriteFrame(TimeSpan Duration, byte[] Pixels);

/// <summary>An <c>.aseprite</c> file's animation, ready to be played.</summary>
/// <param name="Width">The canvas's width.</param>
/// <param name="Height">The canvas's height.</param>
/// <param name="Frames">Every frame, in the order they are played.</param>
internal sealed record AsepriteAnimation(int Width, int Height, IReadOnlyList<AsepriteFrame> Frames);

/// <summary>
/// Reads an <c>.aseprite</c> file into the frames it holds.
/// </summary>
/// <remarks>
/// The file is a banner, and the banner is a picture of somebody's own: what is read is the whole document
/// — layers composited in order, cels placed where they are placed — so that what is drawn is the
/// animation the artist drew and not a flattening of it.
/// <para>
/// What is not read is named rather than drawn wrongly: an indexed-colour file, a tilemap cel, or a
/// compression with no reader here all say so, because a banner is worth an honest refusal and not a
/// picture that is quietly not the one in the file. An indexed file is the one a reader meets in practice,
/// and Aseprite writes it as RGBA with one menu choice: the refusal names that choice.
/// </para>
/// <para>
/// The pixels of a frame are Bgra8888, unpremultiplied, which is what the banner's bitmap is made from.
/// </para>
/// </remarks>
internal static class Aseprite
{
    /// <summary>The number that says a file is an aseprite one.</summary>
    private const ushort Magic = 0xA5E0;

    /// <summary>The number that says a block of a file is a frame.</summary>
    private const ushort FrameMagic = 0xF1FA;

    /// <summary>The header every file opens with.</summary>
    private const int HeaderLength = 128;

    /// <summary>The most of a file this reads, so that a wrong file is refused rather than swallowed.</summary>
    private const int Most = 64 * 1024 * 1024;

    /// <summary>How long a frame is shown when the file says nothing about it.</summary>
    private const int DefaultDuration = 100;

    /// <summary>
    /// Reads a file.
    /// </summary>
    /// <param name="path">The file.</param>
    /// <returns>The animation it holds.</returns>
    /// <exception cref="InvalidDataException">The file is not one this can read.</exception>
    /// <exception cref="IOException">The file could not be read.</exception>
    public static AsepriteAnimation Read(string path) => Parse(File.ReadAllBytes(path));

    /// <summary>
    /// Reads the bytes of a file.
    /// </summary>
    /// <param name="file">The file's bytes.</param>
    /// <returns>The animation it holds.</returns>
    /// <exception cref="InvalidDataException">The bytes are not one this can read.</exception>
    public static AsepriteAnimation Parse(byte[] file)
    {
        ArgumentNullException.ThrowIfNull(file);

        try
        {
            return ReadAll(file);
        }
        catch (Exception error)
            when (error is ArgumentOutOfRangeException or IndexOutOfRangeException or EndOfStreamException)
        {
            throw new InvalidDataException("the file ends in the middle of what it states", error);
        }
    }

    /// <summary>Reads a file whose header has already been found to be long enough.</summary>
    private static AsepriteAnimation ReadAll(byte[] file)
    {
        if (file.Length < HeaderLength)
        {
            throw new InvalidDataException("the file is shorter than an aseprite header");
        }

        var at = 0;
        var size = Integer(file, ref at, 4);

        if (size > Most)
        {
            throw new InvalidDataException("the file is larger than this reads");
        }

        if (Word(file, ref at) != Magic)
        {
            throw new InvalidDataException("the file is not an aseprite file");
        }

        var count = Word(file, ref at);
        var width = Word(file, ref at);
        var height = Word(file, ref at);
        var depth = Word(file, ref at);

        _ = Integer(file, ref at, 4);

        var speed = Word(file, ref at);

        // The eight bytes after the speed are a zero, a colour an image is not drawn with here, and three
        // more the format keeps for itself.
        at += 8;

        _ = Word(file, ref at);
        at += 2;

        // The grid is where the file was last looked at through a grid; nothing is drawn with it. What is
        // left of the header is reserved, and every file's header is the same length whatever it states.
        at = HeaderLength;

        if (width == 0 || height == 0)
        {
            throw new InvalidDataException("the canvas has no size");
        }

        if (depth is not (16 or 32))
        {
            throw new InvalidDataException(
                depth == 8
                    ? "an indexed-colour file is not read; save the banner as RGBA in Aseprite"
                    : $"a colour depth of {depth} bits is not read"
            );
        }

        var frames = new List<Frame>(count);
        var layers = new List<Layer>();

        for (var frame = 0; frame < count; frame++)
        {
            frames.Add(ReadFrame(file, ref at, depth, layers, speed));
        }

        var animation = new List<AsepriteFrame>(frames.Count);

        foreach (var held in frames)
        {
            animation.Add(new AsepriteFrame(TimeSpan.FromMilliseconds(held.Duration), Draw(frames, held, width, height, layers)));
        }

        return new AsepriteAnimation(width, height, animation);
    }

    /// <summary>Reads one frame's chunks.</summary>
    private static Frame ReadFrame(
        byte[] file,
        ref int at,
        int depth,
        List<Layer> layers,
        int speed
    )
    {
        var start = at;
        var length = Integer(file, ref at, 4);

        if (length < 16 || start + length > file.Length)
        {
            throw new InvalidDataException("a frame is not as long as it says it is");
        }

        if (Word(file, ref at) != FrameMagic)
        {
            throw new InvalidDataException("a frame does not begin with a frame");
        }

        var oldChunks = Word(file, ref at);
        var duration = Word(file, ref at);

        at += 2;

        var chunks = Integer(file, ref at, 4);
        var count = chunks == 0 ? oldChunks : (ushort)chunks;
        var end = start + (int)length;
        var cels = new Dictionary<int, Cel>();

        for (var chunk = 0; chunk < count; chunk++)
        {
            if (at + 6 > end)
            {
                throw new InvalidDataException("a frame ends before its chunks do");
            }

            var size = Integer(file, ref at, 4);
            var type = Word(file, ref at);

            if (size < 6 || at + size - 6 > end)
            {
                throw new InvalidDataException("a chunk is not as long as it says it is");
            }

            var next = at + (int)size - 6;

            switch (type)
            {
                // A layer and a cel are stated under two numbers, because the format renamed its chunk
                // types: a file written by an older Aseprite says 0x0005 and 0x0006, and one written by the
                // editor as it is now says 0x2004 and 0x2005 of the same two things.
                case 0x2004 or 0x0005:
                    layers.Add(ReadLayer(file, at));
                    break;

                case 0x2005 or 0x0006:
                    var (layer, cel) = ReadCel(file, at, depth, layers);
                    cels[layer] = cel;
                    break;

                default:
                    // Every other chunk says something about the document — its palette, its tags, its
                    // slices, the user data on a layer — and nothing about the picture this draws.
                    break;
            }

            at = next;
        }

        at = end;

        return new Frame(cels, duration > 0 ? duration : speed > 0 ? speed : DefaultDuration);
    }

    /// <summary>Reads a layer's own state, which is whether it is drawn and how opaque it is.</summary>
    private static Layer ReadLayer(byte[] file, int at)
    {
        var flags = Word(file, at);
        var width = Word(file, at + 6);
        var height = Word(file, at + 8);
        var blend = Word(file, at + 10);
        var opacity = file[at + 12];

        // The layer's name follows its own sixteen bytes, and nothing here is drawn with it.
        at += 16;

        _ = Text(file, ref at);

        // The background bit belongs to the bottom layer of a file whose canvas is filled, and a group is
        // a layer that holds layers rather than pixels: neither is a picture to draw.
        var visible = (flags & 0x0001) != 0 && (flags & 0x0020) == 0;

        return new Layer(visible, opacity, blend, width, height);
    }

    /// <summary>
    /// Reads a cel, which is one layer's contribution to one frame.
    /// </summary>
    /// <remarks>
    /// A cel is one of three things: pixels of its own, a link to another frame's cel of the same layer, or
    /// pixels written compressed. What is kept is whichever of them the file states, so that a link can be
    /// followed once every frame has been read.
    /// </remarks>
    private static (int Layer, Cel Cel) ReadCel(byte[] file, int at, int depth, List<Layer> layers)
    {
        var layer = Word(file, at);
        var x = Short(file, at + 2);
        var y = Short(file, at + 4);
        var opacity = file[at + 6];
        var type = Word(file, at + 7);

        at += 16;

        switch (type)
        {
            case 0:
            {
                // A raw cel names no size of its own: it is as big as the layer it belongs to.
                if (layer >= layers.Count)
                {
                    throw new InvalidDataException("a cel belongs to a layer the file has not stated");
                }

                var size = layers[layer];

                return (
                    layer,
                    new Cel(
                        x,
                        y,
                        opacity,
                        size.Width,
                        size.Height,
                        Pixels(file, at, size.Width, size.Height, depth),
                        -1
                    )
                );
            }

            case 1:
                return (layer, new Cel(x, y, opacity, 0, 0, [], Word(file, at)));

            case 2:
            {
                var width = Word(file, at);
                var height = Word(file, at + 2);

                return (layer, new Cel(x, y, opacity, width, height, Inflate(file, at + 4, width, height, depth), -1));
            }

            default:
                throw new InvalidDataException("the file holds a cel this does not read");
        }
    }

    /// <summary>Reads pixels as the file writes them.</summary>
    private static byte[] Pixels(byte[] file, int at, int width, int height, int depth)
    {
        var drawn = new byte[width * height * 4];
        var size = depth / 8;

        for (var pixel = 0; pixel < width * height; pixel++)
        {
            Write(drawn, pixel, file, at + (pixel * size), depth);
        }

        return drawn;
    }

    /// <summary>
    /// Reads a cel's compressed pixels.
    /// </summary>
    /// <remarks>
    /// The modern spelling puts the compression in the first byte and the pixels after it; the older one is
    /// a zlib stream from the first byte. Both are recognized by looking at the stream rather than by a
    /// version number, since the number is the file's and a reader's job is to read what is there.
    /// </remarks>
    private static byte[] Inflate(byte[] file, int at, int width, int height, int depth)
    {
        if (at >= file.Length)
        {
            throw new InvalidDataException("a compressed cel has no data");
        }

        // A zlib stream begins with a deflate header, whose low nibble is 8.
        if (file.Length - at >= 2 && (file[at] & 0x0F) == 8)
        {
            return Unzip(file, at, file.Length - at, width, height, depth);
        }

        return file[at] switch
        {
            0 => Pixels(file, at + 1, width, height, depth),
            1 => Unzip(file, at + 1, file.Length - at - 1, width, height, depth),
            _ => throw new InvalidDataException("the file holds a compression this does not read"),
        };
    }

    /// <summary>Reads a zlib image a compressed cel states.</summary>
    private static byte[] Unzip(byte[] file, int at, int length, int width, int height, int depth)
    {
        var expected = width * height * depth / 8;

        if (expected <= 0 || at + length > file.Length)
        {
            throw new InvalidDataException("a cel's pixels are not what it says they are");
        }

        using var stream = new ZLibStream(new MemoryStream(file, at, length), CompressionMode.Decompress);
        var pixels = new byte[expected];
        var read = 0;

        while (read < expected)
        {
            var got = stream.Read(pixels, read, expected - read);

            if (got == 0)
            {
                throw new InvalidDataException("a cel's pixels are cut short");
            }

            read += got;
        }

        var drawn = new byte[width * height * 4];
        var size = depth / 8;

        for (var pixel = 0; pixel < width * height; pixel++)
        {
            Write(drawn, pixel, pixels, pixel * size, depth);
        }

        return drawn;
    }

    /// <summary>Writes one pixel of a given depth as Bgra8888.</summary>
    private static void Write(byte[] to, int pixel, byte[] from, int at, int depth)
    {
        switch (depth)
        {
            case 32:
                to[(pixel * 4) + 0] = from[at + 2];
                to[(pixel * 4) + 1] = from[at + 1];
                to[(pixel * 4) + 2] = from[at + 0];
                to[(pixel * 4) + 3] = from[at + 3];
                break;

            case 16:
                // Grayscale and alpha, both one byte: the grey is written on all three channels.
                to[(pixel * 4) + 0] = from[at];
                to[(pixel * 4) + 1] = from[at];
                to[(pixel * 4) + 2] = from[at];
                to[(pixel * 4) + 3] = from[at + 1];
                break;

            default:
                throw new InvalidDataException($"a colour depth of {depth} bits is not read");
        }
    }

    /// <summary>
    /// Draws one frame: every cel of it, composited in the order the layers are in.
    /// </summary>
    /// <remarks>
    /// Every layer is drawn as normal, whatever blend mode it carries. A blend mode is a way of mixing one
    /// layer with what is under it, and a banner is a picture that is meant to be seen rather than a
    /// document that is meant to be edited: losing a layer to a mode this cannot mix would be losing part
    /// of the picture, where drawing it plainly is the layer the artist drew.
    /// </remarks>
    private static byte[] Draw(
        List<Frame> frames,
        Frame frame,
        int width,
        int height,
        List<Layer> layers
    )
    {
        var canvas = new byte[width * height * 4];

        for (var index = 0; index < layers.Count; index++)
        {
            if (!layers[index].Visible || !frame.Cels.TryGetValue(index, out var cel))
            {
                continue;
            }

            var pixels = PixelsOf(frames, frame, cel, index);

            if (pixels is null)
            {
                continue;
            }

            var opacity = cel.Opacity * layers[index].Opacity / 255;

            for (var y = 0; y < cel.Height; y++)
            {
                var to = cel.Y + y;

                if (to < 0 || to >= height)
                {
                    continue;
                }

                for (var x = 0; x < cel.Width; x++)
                {
                    var from = cel.X + x;

                    if (from < 0 || from >= width)
                    {
                        continue;
                    }

                    var source = ((y * cel.Width) + x) * 4;
                    var target = ((to * width) + from) * 4;
                    var alpha = pixels[source + 3] * opacity / 255;

                    Over(canvas, target, pixels, source, alpha);
                }
            }
        }

        return canvas;
    }

    /// <summary>The pixels a cel draws, following a link to the frame it names.</summary>
    private static byte[]? PixelsOf(
        List<Frame> frames,
        Frame frame,
        Cel cel,
        int layer
    )
    {
        var held = cel;
        var seen = new HashSet<int>();

        while (held.Pixels.Length == 0 && held.Link >= 0)
        {
            if (held.Link >= frames.Count || !seen.Add(held.Link))
            {
                return null;
            }

            if (!frames[held.Link].Cels.TryGetValue(layer, out var linked))
            {
                return null;
            }

            held = linked;
        }

        return held.Pixels.Length == 0 ? null : held.Pixels;
    }

    /// <summary>Puts one pixel over what is already there, the usual way.</summary>
    private static void Over(byte[] canvas, int target, byte[] pixels, int source, int alpha)
    {
        if (alpha == 0)
        {
            return;
        }

        if (alpha == 255)
        {
            canvas[target + 0] = pixels[source + 0];
            canvas[target + 1] = pixels[source + 1];
            canvas[target + 2] = pixels[source + 2];
            canvas[target + 3] = 255;

            return;
        }

        var under = canvas[target + 3];
        var total = alpha + ((under * (255 - alpha)) / 255);

        if (total == 0)
        {
            return;
        }

        for (var channel = 0; channel < 3; channel++)
        {
            canvas[target + channel] = (byte)(
                (
                    (pixels[source + channel] * alpha)
                    + (canvas[target + channel] * under * (255 - alpha) / 255)
                ) / total
            );
        }

        canvas[target + 3] = (byte)total;
    }

    /// <summary>Reads a length-prefixed, zero-terminated string.</summary>
    private static string Text(byte[] file, ref int at)
    {
        var length = Word(file, ref at);
        var text = System.Text.Encoding.UTF8.GetString(file, at, length);

        at += length;

        return text;
    }

    /// <summary>Reads an unsigned word, moving past it.</summary>
    private static ushort Word(byte[] file, ref int at)
    {
        var value = BinaryPrimitives.ReadUInt16LittleEndian(file.AsSpan(at, 2));

        at += 2;

        return value;
    }

    /// <summary>Reads an unsigned word at a place, without moving past it.</summary>
    private static ushort Word(byte[] file, int at) =>
        BinaryPrimitives.ReadUInt16LittleEndian(file.AsSpan(at, 2));

    /// <summary>Reads a signed word at a place, without moving past it.</summary>
    private static short Short(byte[] file, int at) =>
        BinaryPrimitives.ReadInt16LittleEndian(file.AsSpan(at, 2));

    /// <summary>Reads an unsigned integer of a stated length.</summary>
    private static uint Integer(byte[] file, ref int at, int length)
    {
        var value = length switch
        {
            1 => file[at],
            2 => BinaryPrimitives.ReadUInt16LittleEndian(file.AsSpan(at, 2)),
            _ => BinaryPrimitives.ReadUInt32LittleEndian(file.AsSpan(at, 4)),
        };

        at += length;

        return value;
    }

    /// <summary>One layer of the document, as far as drawing it is concerned.</summary>
    /// <param name="Visible">Whether it is drawn.</param>
    /// <param name="Opacity">How opaque it is.</param>
    /// <param name="Blend">The mode it is mixed with what is under it.</param>
    /// <param name="Width">The size a cel of it is when the cel names none.</param>
    /// <param name="Height">The size a cel of it is when the cel names none.</param>
    private sealed record Layer(bool Visible, byte Opacity, ushort Blend, int Width, int Height);

    /// <summary>One layer's contribution to one frame.</summary>
    /// <param name="X">Where it is placed on the canvas.</param>
    /// <param name="Y">Where it is placed on the canvas.</param>
    /// <param name="Opacity">How opaque its pixels are.</param>
    /// <param name="Width">How wide it is.</param>
    /// <param name="Height">How tall it is.</param>
    /// <param name="Pixels">Its pixels, or nothing when it is a link.</param>
    /// <param name="Link">The frame it links to, or -1.</param>
    private sealed record Cel(
        short X,
        short Y,
        byte Opacity,
        int Width,
        int Height,
        byte[] Pixels,
        int Link
    );

    /// <summary>One frame: its cels by layer, and how long it is shown for.</summary>
    /// <param name="Cels">Each layer's contribution, by the layer's position.</param>
    /// <param name="Duration">How long it is shown, in milliseconds.</param>
    private sealed record Frame(Dictionary<int, Cel> Cels, int Duration);
}
