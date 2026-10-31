using Avalonia.Controls;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using MenuItem = Avalonia.Controls.MenuItem;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The listing of a place: the folder it makes, the row that folder is named in, and what a row's menu offers.
/// </summary>
/// <remarks>
/// A place is read, and what this side arranges in it — a folder made, a row named, a row removed — is the place's
/// answer rather than a directory's. What is checked here is the hand-off between the listing that asked for the folder
/// and the listing that names it, which is the one thing a redraw in between must not lose.
/// </remarks>
public sealed class PlacesTests
{
    /// <summary>
    /// A folder made by a place is named by the listing built after it, from the entries that have the folder in them.
    /// </summary>
    /// <remarks>
    /// Making a folder reads the place again, and the read builds a new view and a new listing: the listing that asked
    /// has the entries from before the making, where the new row is not. Answering the hand-off there would answer it
    /// with a stale list, find no row and lose the name.
    /// </remarks>
    [Fact]
    public void ANewFolderIsNamedByTheListingBuiltAfterIt()
    {
        var (host, browser, provider, address) = Setup();
        var asked = new Places(host, browser, Place(provider, address));

        asked.NewFolder();

        Assert.NotEmpty(provider.Made);
        Assert.Null(asked.Pending());

        // The listing the place was read into again, which is the one whose rows hold the folder just made.
        var drawn = new Places(host, browser, Place(provider, address));
        var naming = drawn.Pending();

        Assert.NotNull(naming);
        Assert.Equal(provider.Made, naming.Name);
        Assert.Contains(naming, drawn.Items);

        // Answered once: a row is named once, and a second view of the same place does not open a box on it again.
        Assert.Null(drawn.Pending());
    }

    /// <summary>Removing a row is asked about first, and done when the user said so.</summary>
    /// <remarks>
    /// A dialog needs a window and a headless test has none, so what is checked is the asking: with nothing to show the
    /// question on, nothing is removed — which is what says the row was not removed on the key alone.
    /// </remarks>
    [Fact]
    public void RemovingARowAsksFirst()
    {
        var (host, browser, provider, address) = Setup();
        var data = provider.Suggest("Photos");
        var listing = new Places(host, browser, Place(provider, address));
        var row = Assert.Single(listing.Items);

        listing.Delete([row], ask: true);

        Assert.Empty(provider.Removed);

        listing.Delete([row], ask: false);

        Assert.Equal(data, Assert.Single(provider.Removed));
    }

    /// <summary>The menu on a row a place lets this side edit offers naming and removing it.</summary>
    /// <remarks>
    /// The keys are what the items are, because no translations are registered here: what is checked is that the two
    /// actions a directory's menu carries are carried by a place's menu too, rather than only being reachable by key.
    /// </remarks>
    [Fact]
    public void ARowAPlaceLetsBeEditedOffersNamingAndRemoving()
    {
        var (host, browser, provider, address) = Setup();

        _ = provider.Suggest("Photos");

        var listing = new Places(host, browser, Place(provider, address));
        var view = new ListBrowser<LogicalEntry>(host, listing);
        var menu = new ContextMenu();

        listing.Fill(menu, view, [Assert.Single(listing.Items)], force: false);

        var headers = menu.Items.OfType<MenuItem>().Select(item => item.Header).OfType<string>().ToArray();

        Assert.Contains("rorolala_file_system.rename", headers);
        Assert.Contains("rorolala_file_system.delete", headers);
    }

    /// <summary>The listing of a place, and the place answering it, over a scratch directory.</summary>
    /// <returns>The host, the location, the place's answer, and the address it is read at.</returns>
    private static (IPluginHost Host, Browser Browser, VirtualPlace Provider, LogicalAddress Address) Setup()
    {
        var directory = Scratch.New("places");
        var services = Host.Services();
        var host = services.For(new PluginId("it.places"), 1);
        var shared = new Shared(
            directory,
            new HideRegistry(host.Config),
            host.Config,
            host.Files
        );
        var provider = new VirtualPlace(host);
        var address = new LogicalAddress("logic", "somewhere");

        host.LogicalViews.Register(address.Scheme, provider);

        return (
            host,
            new Browser(host.Log, shared, address.ToString(), host.LogicalViews),
            provider,
            address
        );
    }

    /// <summary>One address as one answer: the place's title, its entries, and where going up goes.</summary>
    /// <param name="provider">What answers the place.</param>
    /// <param name="address">The address.</param>
    /// <returns>The place.</returns>
    private static LogicalPlace Place(VirtualPlace provider, LogicalAddress address) =>
        new(address, provider, "the place", provider.Entries(address), null);
}
