using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Input;
using Avalonia.Media;
using Avalonia.Threading;
using Avalonia.VisualTree;

using RorolalaDesktop;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Docking;
using RorolalaDesktop.Hosting;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The grip where two dock seams cross: one drag on it resizes every panel around the crossing.
/// </summary>
/// <remarks>
/// The grip is drawn only where a column seam and a row seam overlap, which means the layout has to hold at
/// least four groups. The docks are fixtures with a control of their own, and what is checked happens in a
/// real window on the headless platform because the grip is a control over the dock area and nothing smaller
/// than a window has one.
/// </remarks>
public sealed class CrossTests
{
    /// <summary>Two seams crossing draw one grip, and dragging it moves both of them.</summary>
    [Fact]
    public void ACrossingGripResizesTheFourPanels()
    {
        var services = Host.Services();
        var first = Activate(services, "it.cross.first");
        var second = Activate(services, "it.cross.second");
        var third = Activate(services, "it.cross.third");
        var fourth = Activate(services, "it.cross.fourth");

        // A two-by-two grid: a column seam with two rows either side of it. The columns are made first so the
        // row splits land inside them, the row seam in the middle of each half.
        Beside(services, third, first, DockSide.Right);
        Beside(services, second, first, DockSide.Bottom);
        Beside(services, fourth, third, DockSide.Bottom);

        services.Windows.Make = (key, dock) => new MainWindow(services, key, dock);
        services.Windows.OpenMain();

        var main = (MainWindow)services.Shell.Window!;

        Pump();

        var area = main.GetVisualDescendants().OfType<DockArea>().First();
        var grips = area.GetVisualDescendants()
            .OfType<Panel>()
            .Where(panel => panel.Classes.Contains(DockArea.CrossClass))
            .ToList();

        Assert.Single(grips);

        // A layout may hold several crossings; the one this checks is the middle of the work.
        var grip = grips
            .OrderBy(panel =>
            {
                var at = panel.TranslatePoint(default, area);

                return at is { } point
                    ? Math.Abs(point.X - (area.Bounds.Width / 2))
                        + Math.Abs(point.Y - (area.Bounds.Height / 2))
                    : double.MaxValue;
            })
            .First();

        Assert.True(
            grip.Bounds.Width > 0 && grip.Bounds.Height > 0,
            "the crossing grip has not been laid out"
        );
        Assert.Equal(
            2,
            grip.GetVisualDescendants().OfType<Avalonia.Controls.Shapes.Rectangle>().Count()
        );

        var root = Assert.IsType<LayoutSplit>(services.Docks.Layout.Root);
        var left = Assert.IsType<LayoutSplit>(root.First);
        var right = Assert.IsType<LayoutSplit>(root.Second);

        Assert.Equal(DockAxis.Columns, root.Axis);
        Assert.Equal(DockAxis.Rows, left.Axis);
        Assert.Equal(DockAxis.Rows, right.Axis);

        var columns = root.Ratio;
        var leftRows = left.Ratio;
        var rightRows = right.Ratio;

        var at = grip.TranslatePoint(new Point(grip.Bounds.Width / 2, grip.Bounds.Height / 2), main);

        Assert.NotNull(at);

        main.MouseDown(at.Value, MouseButton.Left);

        var pressed = area.TranslatePoint(default, main);

        Assert.NotNull(pressed);

        // One step at a time, and the crossing stays under the hand at every step: a grip that follows the
        // pointer while the seams run away is what a drifting drag is.
        for (var step = 1; step <= 4; step++)
        {
            main.MouseMove(at.Value + new Vector(step * 10, step * 8));
            Pump();

            var under = area.GetVisualDescendants()
                .OfType<Panel>()
                .Single(panel => panel.Classes.Contains(DockArea.CrossClass));
            var centre = under.TranslatePoint(
                new Point(under.Bounds.Width / 2, under.Bounds.Height / 2),
                area
            );

            Assert.NotNull(centre);
            Assert.InRange(
                centre.Value.X - (at.Value.X - pressed.Value.X),
                (step * 10) - 2,
                (step * 10) + 2
            );
            Assert.InRange(
                centre.Value.Y - (at.Value.Y - pressed.Value.Y),
                (step * 8) - 2,
                (step * 8) + 2
            );
        }

        main.MouseMove(at.Value + new Vector(48, 36));
        Pump();
        main.MouseUp(at.Value + new Vector(48, 36), MouseButton.Left);
        Pump();

        Assert.True(root.Ratio > columns, "the column seam was not moved");
        Assert.True(left.Ratio > leftRows, "the left row seam was not moved");
        Assert.True(right.Ratio > rightRows, "the right row seam was not moved");

        // The crossing the pointer left must stand where the pointer left it: the grip and the seams are one
        // place, so a seam that moved less than the pointer is a drag that did not stay under the hand.
        var movedGrip = area.GetVisualDescendants()
            .OfType<Panel>()
            .Single(panel => panel.Classes.Contains(DockArea.CrossClass));
        var moved = movedGrip.TranslatePoint(
            new Point(movedGrip.Bounds.Width / 2, movedGrip.Bounds.Height / 2),
            area
        );

        Assert.NotNull(moved);
        Assert.InRange(moved.Value.X - (at.Value.X - pressed.Value.X), 46, 50);
        Assert.InRange(moved.Value.Y - (at.Value.Y - pressed.Value.Y), 34, 38);
    }

    /// <summary>Registers a dock and opens it, which is what a plugin asking for one does.</summary>
    /// <param name="services">Everything the host holds.</param>
    /// <param name="nameId">The dock's stable name id.</param>
    /// <returns>The dock.</returns>
    private static DockInstance Activate(HostServices services, string nameId)
    {
        var registration = new DockRegistration(
            Core.Id,
            nameId,
            nameId,
            DockOpenMode.New,
            DockSide.Center,
            () => new FakeDockView { Control = new Border { Background = Brushes.Gray } }
        );

        services.Docks.Register(registration);

        return services.Docks.Activate(registration)
            ?? throw new InvalidOperationException("the dock was not opened");
    }

    /// <summary>Moves a dock to the side of another, which is what a split is.</summary>
    /// <param name="services">Everything the host holds.</param>
    /// <param name="instance">The dock to move.</param>
    /// <param name="beside">The dock whose group it is put beside.</param>
    /// <param name="side">Which side.</param>
    private static void Beside(
        HostServices services,
        DockInstance instance,
        DockInstance beside,
        DockSide side
    )
    {
        var group =
            DockTree.GroupOf(services.Docks.Layout, beside.DockNameId, beside.Ordinal)
            ?? throw new InvalidOperationException("the dock is not in the main window's tree");

        services.Docks.Move(instance, new DockTarget(group.Id, side, 0));
    }

    /// <summary>Lets the toolkit catch up with the changes and one frame of it draw.</summary>
    private static void Pump()
    {
        for (var turn = 0; turn < 30; turn++)
        {
            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        }

        Dispatcher.UIThread.RunJobs();
    }
}
