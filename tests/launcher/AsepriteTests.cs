using RorolalaLauncher.Banner;

namespace RolalalaLauncher.IntegrationTests;

/// <summary>
/// What an <c>.aseprite</c> file is read into.
/// </summary>
/// <remarks>
/// What is checked is the banner the program ships with, because that is the file a person meets: a reader
/// that cannot read its own picture is a banner that is not there, and the format is one an artist writes
/// with an editor rather than one this project owns.
/// </remarks>
public sealed class AsepriteTests
{
    /// <summary>The banner the program ships with is read as the animation it is.</summary>
    [Fact]
    public void TheShippedBannerPlays()
    {
        var animation = Aseprite.Read(Path.Combine(AppContext.BaseDirectory, "red-flash.aseprite"));

        // The picture is 128 by 32, which is the four-to-one a banner is, and the frames are the ten it was
        // drawn with: a reader that found fewer would be a reader that stopped early.
        Assert.Equal(128, animation.Width);
        Assert.Equal(32, animation.Height);
        Assert.Equal(10, animation.Frames.Count);

        foreach (var frame in animation.Frames)
        {
            Assert.Equal(128 * 32 * 4, frame.Pixels.Length);
            Assert.Equal(TimeSpan.FromMilliseconds(100), frame.Duration);
        }
    }

    /// <summary>The frames are not all the same, which is what makes it an animation.</summary>
    [Fact]
    public void TheFramesAreNotAllTheSame()
    {
        var animation = Aseprite.Read(Path.Combine(AppContext.BaseDirectory, "red-flash.aseprite"));
        var first = animation.Frames[0].Pixels;

        Assert.Contains(animation.Frames, frame => !frame.Pixels.SequenceEqual(first));
    }

    /// <summary>What is drawn is drawn over the whole canvas, at least somewhere.</summary>
    [Fact]
    public void SomethingIsDrawn()
    {
        var animation = Aseprite.Read(Path.Combine(AppContext.BaseDirectory, "red-flash.aseprite"));

        Assert.Contains(
            animation.Frames,
            frame => frame.Pixels.Where((_, at) => at % 4 == 3).Any(alpha => alpha != 0)
        );
    }

    /// <summary>
    /// Every layer of the shipped banner is drawn in every frame, a linked cel included.
    /// </summary>
    /// <remarks>
    /// The picture's bottom layer fills the canvas and its frames after the first reach that layer through a
    /// linked cel, which states no size of its own. A reader that drew the link's size rather than the size of
    /// the cel it names leaves those frames as the layers above the link and nothing else, which is a layer
    /// missing from every frame but the first 鈥?what this checks is that the canvas is covered throughout.
    /// </remarks>
    [Fact]
    public void AFrameDrawnThroughALinkedCelCoversTheCanvas()
    {
        var animation = Aseprite.Read(Path.Combine(AppContext.BaseDirectory, "rola-command.aseprite"));

        foreach (var frame in animation.Frames)
        {
            // The corner belongs to the bottom layer alone, so what is there is what that layer drew.
            Assert.Equal(255, frame.Pixels[3]);
        }
    }

    /// <summary>A file that is not one is refused rather than half-drawn.</summary>
    [Fact]
    public void AnotherFileIsRefused()
    {
        var path = Scratch.Write("not-aseprite", "file.aseprite", [0x00, 0x01, 0x02, 0x03]);

        Assert.Throws<InvalidDataException>(() => Aseprite.Read(path));
    }

    /// <summary>A file that stops in the middle of what it states is refused.</summary>
    [Fact]
    public void ATruncatedFileIsRefused()
    {
        var whole = File.ReadAllBytes(Path.Combine(AppContext.BaseDirectory, "red-flash.aseprite"));
        var path = Scratch.Write("truncated", "file.aseprite", whole[..(whole.Length / 2)]);

        Assert.Throws<InvalidDataException>(() => Aseprite.Read(path));
    }
}
