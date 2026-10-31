using Avalonia;
using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Media;
using Avalonia.Threading;
using Avalonia.VisualTree;
using FileSystemPlugin;
using RorolalaDesktop.Contract;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The table a directory is read in, as a table: where its columns stand.
/// </summary>
/// <remarks>
/// The columns are pixels written into the band of headings and into every row, rather than a share each sheet
/// works out for itself — a row that worked its own share out would put its cells where its own words asked for
/// them, and two rows holding different words would disagree about where a column begins. What is checked here is
/// that the band and every row are given one set of widths, and that the set fits the room the table has.
/// </remarks>
public sealed class ListColumnTests
{
    /// <summary>
    /// The columns the table is made of, for the widths to be read off: the picture, the name, one contributed
    /// column read with the name, the room, and one value after it.
    /// </summary>
    private const int Columns = 7;

    /// <summary>Every column begins at the same place in the band and in every row.</summary>
    [Fact]
    public void AColumnStandsWhereItIsInEveryPartOfTheTable()
    {
        var browser = Show(700, out var window);

        try
        {
            var sheets = Sheets(browser);

            Assert.True(sheets.Count >= 3, $"{sheets.Count} sheets, which is the band and fewer rows than were listed");

            var band = Widths(sheets[0]);

            foreach (var sheet in sheets.Skip(1))
            {
                Assert.Equal(band, Widths(sheet));
            }
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>A table squeezed by a narrow dock stays inside it, and its columns stay lined up.</summary>
    [Fact]
    public void ASqueezedTableFitsTheRoomItIsDrawnIn()
    {
        var browser = Show(700, out var window);

        try
        {
            // A width the columns cannot all fit in at the width they ask for, which is where a table that kept
            // them would put its last columns past the dock's edge.
            window.Width = 300;

            Dispatcher.UIThread.RunJobs();
            AvaloniaHeadlessPlatform.ForceRenderTimerTick();

            var sheets = Sheets(browser);
            var band = Widths(sheets[0]);

            foreach (var sheet in sheets)
            {
                var row = sheet.GetVisualParent<Control>();
                var widths = Widths(sheet);
                var room = row?.Bounds.Width ?? sheet.Bounds.Width;

                Assert.Equal(band, widths);
                Assert.True(widths.Sum() <= room + 1, $"{widths.Sum()} wide in {room}");
            }
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>The column names of one sheet, as the width each came to.</summary>
    /// <param name="sheet">The sheet to read.</param>
    /// <returns>The widths.</returns>
    private static double[] Widths(Grid sheet) =>
        [.. sheet.ColumnDefinitions.Select(column => Math.Round(column.ActualWidth, 1))];

    /// <summary>Every sheet of the table on screen: the band of headings and each row's cells.</summary>
    /// <param name="browser">The table.</param>
    /// <returns>The grids, the band first.</returns>
    private static List<Grid> Sheets(Control browser) =>
        [.. browser.GetVisualDescendants().OfType<Grid>().Where(grid => grid.ColumnDefinitions.Count == Columns)];

    /// <summary>Puts a table of a few entries on a headless window of a given width.</summary>
    /// <param name="width">How wide the window is.</param>
    /// <param name="window">The window, which the caller closes.</param>
    /// <returns>The table.</returns>
    private static ListBrowser<string> Show(double width, out Window window)
    {
        var services = Host.Services();
        var host = services.For(new PluginId("test"), 1);

        var browser = new ListBrowser<string>(host, new Few(host));

        window = new Window
        {
            Width = width,
            Height = 300,
            Content = browser,
        };

        window.Show();

        Dispatcher.UIThread.RunJobs();
        AvaloniaHeadlessPlatform.ForceRenderTimerTick();

        return browser;
    }

    /// <summary>
    /// A listing of entries whose names are of different lengths, so that two rows that measured themselves would
    /// not measure the same.
    /// </summary>
    private sealed class Few(IPluginHost host)
        : Listing<string>(
            host,
            ["a.psd", "a rather longer name than the one above it at all.psd", "b.psd"]
        )
    {
        /// <summary>What the columns are: two contributed ones, so that where they stand can be read.</summary>
        public override IReadOnlyList<Column<string>> Columns =>
            [
                new Column<string>("rorolala_file_system.column_owner", 90, entry => entry.Length.ToString(), Placement: ColumnPlacement.BesideName),
                new Column<string>("rorolala_file_system.column_size", 70, entry => $"{entry.Length} B", Placement: ColumnPlacement.BeforeValues),
            ];

        /// <inheritdoc />
        public override object Key(string item) => item;

        /// <inheritdoc />
        public override string Name(string item) => item;

        /// <inheritdoc />
        public override Control Icon(string item, int size) =>
            new Border { Width = size, Height = size, Background = Brushes.Gray };

        /// <inheritdoc />
        public override void Open(string item) { }

        /// <inheritdoc />
        public override void Fill(ContextMenu menu, Control from, IReadOnlyList<string> chosen, bool force) { }
    }
}
