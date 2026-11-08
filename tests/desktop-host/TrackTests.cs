using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Threading;
using Avalonia.VisualTree;
using RorolalaVCSPlugin;

using Vcs = global::RorolalaVCSPlugin.RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The track: what a run of it is told, and the field the message is written in.
/// </summary>
/// <remarks>
/// The recording itself is the command line's and is not run here. What is checked is the one thing
/// this plugin decides about a forced run, whether the command line is asked to record what a plain
/// track would refuse, and the room the field under the graph takes.
/// </remarks>
public sealed class TrackTests
{
    /// <summary>A forced track carries the force flag, and a plain one does not.</summary>
    [Fact]
    public void AForcedTrackAsksTheCommandLineToForceIt()
    {
        var plain = TrackCommand.Arguments(["/work/logo.psd"], "logo", force: false);

        Assert.Contains("track", plain);
        Assert.Contains("--no-editor", plain);
        Assert.DoesNotContain("--force", plain);

        var forced = TrackCommand.Arguments(["/work/logo.psd"], "logo", force: true);

        Assert.Contains("--force", forced);
        Assert.Contains("/work/logo.psd", forced);
    }

    /// <summary>
    /// The message field fills the room beside the button, reads as text across it, and keeps its row
    /// one height.
    /// </summary>
    /// <remarks>
    /// The field is the whole left side of the row rather than the line the message is drawn on. The
    /// base theme puts the text beam on that line alone, so a field which did not carry the beam
    /// itself would let a press land on the padding and the empty width while the pointer said the
    /// room was ground. The hint is a text block, so it would also take the body type rather than the
    /// field's and change the row's height when the first character replaced it.
    /// </remarks>
    [Fact]
    public void TheMessageFieldCoversItsRoomAndReadsAsTextAcrossIt()
    {
        var services = Host.Services();
        var plugin = new Vcs();

        plugin.Initialize(services.For(plugin.Manifest.Id, 1));

        var view = services
            .Docks.Registrations.Single(dock =>
                dock.DockNameId == RorolalaDocks.VersionGraphNameId
            )
            .Create()
            .View;
        var window = new Window { Content = view, Width = 1128, Height = 160 };

        window.Show();
        Pump();

        var field = window.GetVisualDescendants().OfType<TextBox>().Single();
        var row = Assert.IsType<Grid>(field.Parent);
        var bar = window.GetVisualDescendants().OfType<VersionGraphTrackBar>().Single();

        // The field is the star column and nothing else: the button takes the room after it.
        Assert.Equal(row.ColumnDefinitions[0].ActualWidth, field.Bounds.Width, 3);

        // The beam is the field's own, so the template's border inherits it over the whole room,
        // the padding and the empty width included, rather than only over the line of text.
        Assert.Equal("Ibeam", field.Cursor?.ToString());

        var border = field.GetVisualDescendants().OfType<Border>().First(one => one.Name == "border");

        Assert.Equal("Ibeam", border.Cursor?.ToString());

        // The hint and the text are the same line, so a row showing one is as tall as a row
        // showing the other: typing must not move the whole strip.
        var empty = bar.Bounds.Height;

        field.Text = "a message";
        Pump();

        Assert.Equal(empty, bar.Bounds.Height, 3);

        window.Close();
    }

    /// <summary>Lets the window run a few turns of its loop, which is what lays a control out.</summary>
    private static void Pump()
    {
        for (var turn = 0; turn < 20; turn++)
        {
            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        }

        Dispatcher.UIThread.RunJobs();
    }
}
