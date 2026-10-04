using Avalonia;
using Avalonia.Media;
using RorolalaDesktop.CoreDocks;
using RorolalaDesktop.Theming;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A colour as a hue and two fractions, and where a point on the plate stands.
/// </summary>
/// <remarks>
/// What is checked is the arithmetic and the geometry, which is the part of a plate that can be worked out
/// without a screen: the pointer has to read as the colour under it, and a colour has to be found again where
/// it was put — a plate that is off by a corner, or that reads the way down as darker the wrong way round,
/// picks a colour the user did not point at.
/// </remarks>
public sealed class ColourTests
{
    /// <summary>A colour taken apart and put back together is the colour it started as.</summary>
    /// <param name="colour">The colour to take apart.</param>
    [Theory]
    [InlineData("#000000")]
    [InlineData("#FFFFFF")]
    [InlineData("#808080")]
    [InlineData("#FF0000")]
    [InlineData("#00FF00")]
    [InlineData("#0000FF")]
    [InlineData("#FFFF00")]
    [InlineData("#BFFF00")]
    [InlineData("#FF7EA2")]
    [InlineData("#7EA2FF")]
    [InlineData("#42212A")]
    public void AColourIsTheSameTheWayRound(string colour)
    {
        var (hue, saturation, value) = Hsv.Of(Color.Parse(colour));

        Assert.Equal(Color.Parse(colour), Hsv.To(hue, saturation, value));
    }

    /// <summary>The three a grey is: no hue to speak of, and nothing of it.</summary>
    [Fact]
    public void AGreyHasNoHueAndNothingOfIt()
    {
        var (_, saturation, value) = Hsv.Of(Colors.Gray);

        Assert.Equal(0, saturation);
        Assert.Equal(Colors.Gray.R / 255.0, value, 3);
    }

    /// <summary>The corners of the space are the colours they are made of.</summary>
    [Fact]
    public void TheCornersOfTheSpaceAreThemselves()
    {
        Assert.Equal(Colors.Red, Hsv.To(0, 1, 1));
        Assert.Equal(Colors.White, Hsv.To(210, 0, 1));
        Assert.Equal(Colors.Black, Hsv.To(210, 1, 0));
    }

    /// <summary>A hue past the end of the wheel is the hue it comes round to.</summary>
    [Fact]
    public void AHuePastTheEndOfTheWheelComesRound()
    {
        Assert.Equal(Hsv.To(30, 1, 1), Hsv.To(390, 1, 1));
        Assert.Equal(Hsv.To(330, 1, 1), Hsv.To(-30, 1, 1));
    }

    /// <summary>
    /// Where a point stands on the plate: to the right is more of the hue, and to the top is lighter.
    /// </summary>
    /// <param name="x">How far across the point is.</param>
    /// <param name="y">How far down it is.</param>
    /// <param name="saturation">How much of the hue it stands for.</param>
    /// <param name="value">How light it stands for.</param>
    [Theory]
    [InlineData(0, 0, 0, 1)]
    [InlineData(220, 0, 1, 1)]
    [InlineData(0, 130, 0, 0)]
    [InlineData(220, 130, 1, 0)]
    [InlineData(110, 65, 0.5, 0.5)]
    public void APointOnThePlateIsHowMuchHueAndHowLight(double x, double y, double saturation, double value)
    {
        var (much, light) = ColourPlate.At(new Point(x, y), new Size(220, 130));

        Assert.Equal(saturation, much, 3);
        Assert.Equal(value, light, 3);
    }

    /// <summary>A point dragged off the plate stands at the edge it left by, not at a colour that is not there.</summary>
    [Fact]
    public void APointOffThePlateIsItsEdge()
    {
        var (much, light) = ColourPlate.At(new Point(-40, 400), new Size(220, 130));

        Assert.Equal(0, much);
        Assert.Equal(0, light);
    }
}
