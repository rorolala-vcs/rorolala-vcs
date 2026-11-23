using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Controls.Templates;
using Avalonia.Headless;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Styling;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// A grid realizes only the lines its viewport reaches, and the extent it reports is the whole grid.
/// </summary>
/// <remarks>
/// The source here is a fixed tile so that what is checked is the panel alone: the extent, the columns,
/// and which containers exist at the top and at the bottom.
/// </remarks>
public sealed class JustifiedPanelTests
{
    /// <summary>The first screen realizes the top, and the last screen realizes the bottom.</summary>
    [Fact]
    public void OnlyTheLinesTheViewportHoldsAreRealized()
    {
        var panel = new JustifiedVirtualizingPanel { Gap = 12, Source = new FixedSource(100, 40) };
        var list = new ListBox
        {
            ItemsSource = Enumerable.Range(0, 500).Select(at => new Box(at)).ToArray(),
            ItemsPanel = new FuncTemplate<Panel?>(() => panel),
            ItemTemplate = new FuncDataTemplate<Box>(
                (_, _) => new Border { Width = 100, Height = 40 },
                supportsRecycling: true
            ),
        };

        list.Styles.Add(
            new Style(selector => selector.OfType<ListBoxItem>())
            {
                Setters =
                {
                    new Setter(TemplatedControl.PaddingProperty, new Thickness(0)),
                    new Setter(Layoutable.MarginProperty, new Thickness(0)),
                    new Setter(Layoutable.MinHeightProperty, 0.0),
                    new Setter(TemplatedControl.BorderThicknessProperty, new Thickness(0)),
                },
            }
        );

        // A wrapping grid needs a width to wrap against, and a horizontal scrollbar would measure it
        // at an unbounded width and lay every tile on one line.
        ScrollViewer.SetHorizontalScrollBarVisibility(list, ScrollBarVisibility.Disabled);

        var window = new Window { Width = 900, Height = 600, Content = list };

        window.Show();

        try
        {
            Pump();

            Assert.True(panel.Columns > 1, $"the grid has {panel.Columns} column");
            Assert.NotNull(panel.RealizedAt(0));
            Assert.Null(panel.RealizedAt(499));

            var first = panel.RealizedAt(0);

            Assert.Same(panel.RealizedAt(1), panel.Move(NavigationDirection.Right, first!, false));
            Assert.Same(
                panel.RealizedAt(panel.Columns),
                panel.Move(NavigationDirection.Down, first!, false)
            );
            Assert.NotNull(panel.ScrollTo(300));
            Assert.NotNull(panel.RealizedAt(300));

            var scroll = list.GetVisualDescendants().OfType<ScrollViewer>().First();

            Assert.True(scroll.Extent.Height > scroll.Viewport.Height, "the grid is not taller than its viewport");

            scroll.Offset = new Vector(0, scroll.Extent.Height - scroll.Viewport.Height);
            Pump();

            Assert.NotNull(panel.RealizedAt(499));
            Assert.Null(panel.RealizedAt(0));
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>Lets the layout and the drawing catch up.</summary>
    private static void Pump()
    {
        AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        Dispatcher.UIThread.RunJobs();
        Thread.Sleep(10);
    }

    /// <summary>One item, whose identity is all the test needs.</summary>
    /// <param name="Value">Its place in the list.</param>
    private sealed record Box(int Value);

    /// <summary>A source whose tiles are all one size.</summary>
    private sealed class FixedSource(double width, double height) : IGridTileSource
    {
        /// <inheritdoc />
        public double TileWidth => width;

        /// <inheritdoc />
        public double HeightOf(int index) => height;

        /// <inheritdoc />
        public void Calibrate(Control item, int index) { }
    }
}
