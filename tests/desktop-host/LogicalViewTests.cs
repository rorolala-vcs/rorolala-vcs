using FileSystemPlugin;
using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The places plugins answer for: which prefix reaches which one, and what an address is.
/// </summary>
/// <remarks>
/// What is checked is the asking rather than the drawing: an address is split at its first <c>@</c> and its
/// payload is handed over unread, a prefix reaches the plugin that claimed it, and one no plugin claimed reaches
/// nobody. What a provider then says is the provider's own business, and what a listing draws of it needs a
/// window.
/// </remarks>
public sealed class LogicalViewTests
{
    /// <summary>An address is a prefix and everything after the first `@`, unread.</summary>
    [Theory]
    [InlineData("logic@one", "logic", "one")]
    [InlineData("rola@/work/place@vault", "rola", "/work/place@vault")]
    [InlineData("logic@", "logic", "")]
    [InlineData("logic@a@b@c", "logic", "a@b@c")]
    public void AnAddressIsAPrefixAndEverythingAfterIt(string written, string scheme, string payload)
    {
        Assert.True(LogicalAddress.TryRead(written, out var address));
        Assert.NotNull(address);
        Assert.Equal(scheme, address.Scheme);
        Assert.Equal(payload, address.Payload);
        Assert.Equal(written, address.ToString());
    }

    /// <summary>What is not an address is not one, and nothing is refused for it.</summary>
    /// <param name="written">What was typed.</param>
    [Theory]
    [InlineData("")]
    [InlineData("no-at-sign")]
    [InlineData("@nothing-before-it")]
    [InlineData("/a/path/with@in-it")]
    [InlineData("Upper@case")]
    [InlineData("has space@here")]
    public void WhatIsNotAnAddressIsNotOne(string written)
    {
        Assert.False(LogicalAddress.TryRead(written, out var address));
        Assert.Null(address);
    }

    /// <summary>A prefix reaches the plugin that claimed it, and one nobody claimed reaches nobody.</summary>
    [Fact]
    public void APrefixReachesThePluginThatClaimedIt()
    {
        var services = Host.Services();
        var provider = new Stub();

        services.For(new PluginId("it.places"), 0).LogicalViews.Register("logic", provider);

        Assert.Same(provider, services.LogicalViews.Find("logic"));
        Assert.Null(services.LogicalViews.Find("nobody"));
        Assert.Equal(["logic"], services.LogicalViews.Schemes);
    }

    /// <summary>
    /// The first plugin to claim a prefix answers it, whatever is started after it.
    /// </summary>
    /// <remarks>
    /// A prefix is claimed rather than assigned, and the question of who owns one is a question about meaning:
    /// letting a later plugin take it would mean the meaning of an address changed with the load order.
    /// </remarks>
    [Fact]
    public void TheFirstClaimOfAPrefixStands()
    {
        var services = Host.Services();
        var first = new Stub();
        var later = new Stub();

        services.For(new PluginId("it.first"), 0).LogicalViews.Register("rola", first);
        services.For(new PluginId("it.later"), 5).LogicalViews.Register("rola", later);

        Assert.Same(first, services.LogicalViews.Find("rola"));
    }

    /// <summary>A prefix that is not of the shape an address may begin with is refused where it is claimed.</summary>
    /// <param name="scheme">The prefix to claim.</param>
    [Theory]
    [InlineData("")]
    [InlineData("Upper")]
    [InlineData("has space")]
    [InlineData("with@at")]
    [InlineData("with/slash")]
    [InlineData("9leading")]
    public void APrefixThatIsNotOneIsRefusedWhereItIsClaimed(string scheme)
    {
        var services = Host.Services();

        Assert.Throws<ArgumentException>(() =>
            services.For(new PluginId("it.places"), 0).LogicalViews.Register(scheme, new Stub())
        );
    }

    /// <summary>A place is read from an address, with its title and its entries, in one go.</summary>
    [Fact]
    public void APlaceIsReadFromAnAddress()
    {
        var services = Host.Services();
        var provider = new Stub { Title_ = "the place", Entries_ = [new LogicalEntry("one")] };

        services.For(new PluginId("it.places"), 0).LogicalViews.Register("logic", provider);

        // Read through a plugin's view of the registry, which is the interface: the class behind it is the host's.
        var views = services.For(new PluginId("it.reader"), 9).LogicalViews;

        var place = Logical.Find("logic@somewhere", views);

        Assert.NotNull(place);
        Assert.Equal("somewhere", place.Address.Payload);
        Assert.Same(provider, place.Provider);
        Assert.Equal("the place", place.Title);
        Assert.Equal(["one"], place.Entries.Select(entry => entry.Name));
    }

    /// <summary>An address nobody answers is a place with nobody to ask, and not a failure to read.</summary>
    [Fact]
    public void AnAddressNobodyAnswersIsAPlaceWithNobodyToAsk()
    {
        var services = Host.Services();
        var views = services.For(new PluginId("it.reader"), 9).LogicalViews;

        var place = Logical.Find("nobody@somewhere", views);

        Assert.NotNull(place);
        Assert.Null(place.Provider);
        Assert.Empty(place.Entries);
        Assert.Equal("nobody@somewhere", place.Title);
    }

    /// <summary>
    /// What is not an address is a path, and a path is not read as a place.
    /// </summary>
    /// <remarks>
    /// A directory whose name holds an at-sign is a directory, and a path that is not of the shape an address is
    /// has to keep opening as what it is.
    /// </remarks>
    /// <param name="written">What was typed.</param>
    [Theory]
    [InlineData("/tmp/plain")]
    [InlineData("relative/path")]
    [InlineData("@leading")]
    [InlineData("has space@here")]
    public void WhatIsNotAnAddressIsAPath(string written)
    {
        var services = Host.Services();

        Assert.Null(Logical.Find(written, services.For(new PluginId("it.reader"), 9).LogicalViews));
    }

    /// <summary>A drag is offered to what answers the place, and to nobody when nobody does.</summary>
    [Fact]
    public void ADragIsOfferedToWhatAnswersThePlace()
    {
        var services = Host.Services();
        var provider = new Stub { Takes_ = true };

        services.For(new PluginId("it.places"), 0).LogicalViews.Register("logic", provider);

        var views = services.For(new PluginId("it.reader"), 9).LogicalViews;
        var place = Logical.Find("logic@here", views);

        Assert.NotNull(place);
        Assert.True(Logical.Accepts(place, "what is dragged"));
        Assert.False(Logical.Accepts(place, 41));

        Logical.Drop(place, "what is dragged");

        Assert.Equal(["what is dragged"], provider.Dropped);

        var nobody = Logical.Find("nobody@here", views);

        Assert.NotNull(nobody);
        Assert.False(Logical.Accepts(nobody, "anything"));
        Logical.Drop(nobody, "anything");
    }

    /// <summary>
    /// A location moves between a directory and a place with one history, and says which it is looking at.
    /// </summary>
    /// <remarks>
    /// The move and the history are what a dock draws and what back and forward walk, and neither needs a
    /// window: what a toolbar shows follows from where the location says it is.
    /// </remarks>
    [Fact]
    public void ALocationLooksAtADirectoryOrAtAPlace()
    {
        var directory = Scratch.New("logical-location");
        File.WriteAllText(Path.Combine(directory, "file.txt"), "x");

        var host = Host.Services().For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(directory, new HideRegistry(host.Config), host.Config);
        var browser = new Browser(host.Log, shared, directory, host.LogicalViews);
        var provider = new Stub { Title_ = "the place" };

        host.LogicalViews.Register("logic", provider);

        Assert.False(browser.IsLogical);
        Assert.Equal(directory, browser.Current);

        Assert.True(browser.Go("logic@somewhere"));

        Assert.True(browser.IsLogical);
        Assert.Equal("logic@somewhere", browser.Current);
        Assert.Same(provider, browser.Place?.Provider);
        Assert.Equal("the place", browser.Place?.Title);

        // A place is not a path: there is nothing over it to go up to, and a tree cannot be rooted at it.
        Assert.Null(browser.Parent);
        Assert.False(browser.CanGoUp);
        Assert.False(browser.SetBase("logic@somewhere"));

        // A path is a directory again, read as it was — including the directory the browser was in before, which
        // has to be gone to rather than taken for where it already is.
        Assert.True(browser.Go(directory));

        Assert.False(browser.IsLogical);
        Assert.Equal(directory, browser.Current);
        Assert.Equal(["file.txt"], browser.Entries.Select(entry => Path.GetFileName(entry.Path)));

        // And back into the place, through the one history.
        browser.Back();

        Assert.True(browser.IsLogical);
        Assert.Equal("logic@somewhere", browser.Current);

        browser.Forward();

        Assert.False(browser.IsLogical);
        Assert.Equal(directory, browser.Current);
    }

    /// <summary>An address nobody answers is a place a location can be at, and not a path that was refused.</summary>
    [Fact]
    public void AnAddressNobodyAnswersIsAPlaceToBeAt()
    {
        var directory = Scratch.New("logical-nobody");
        var host = Host.Services().For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(directory, new HideRegistry(host.Config), host.Config);
        var browser = new Browser(host.Log, shared, directory, host.LogicalViews);

        Assert.True(browser.Go("nobody@somewhere"));

        Assert.True(browser.IsLogical);
        Assert.Equal("nobody@somewhere", browser.Current);
        Assert.NotNull(browser.Place);
        Assert.Null(browser.Place.Provider);
        Assert.Empty(browser.Place.Entries);
    }

    /// <summary>
    /// A directory whose name holds an at-sign still opens as the directory it is.
    /// </summary>
    /// <remarks>
    /// An address begins with a prefix of a shape a path step cannot be — no separator — so a path that holds an
    /// at-sign later on is not one, which is what keeps a directory named with one reachable.
    /// </remarks>
    [Fact]
    public void APathHoldingAnAtSignStaysAPath()
    {
        var directory = Scratch.New("logical-path");
        var odd = Path.Combine(directory, "a@b");
        Directory.CreateDirectory(odd);

        var host = Host.Services().For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(directory, new HideRegistry(host.Config), host.Config);
        var browser = new Browser(host.Log, shared, directory, host.LogicalViews);

        Assert.True(browser.Go(odd));

        Assert.False(browser.IsLogical);
        Assert.Equal(odd, browser.Current);
    }

    /// <summary>
    /// A place says where going up goes, and the way up is what the location follows.
    /// </summary>
    /// <remarks>
    /// A payload has no grammar in the contract, so what is over an address is the provider's answer and nobody
    /// else's — the location only carries it to the button, and a place with nothing over it leaves the button with
    /// nowhere to go.
    /// </remarks>
    [Fact]
    public void APlaceSaysWhereGoingUpGoes()
    {
        var directory = Scratch.New("logical-up");
        var host = Host.Services().For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(directory, new HideRegistry(host.Config), host.Config);
        var browser = new Browser(host.Log, shared, directory, host.LogicalViews);
        var provider = new Stub { Parent_ = new LogicalAddress("logic", "a") };

        host.LogicalViews.Register("logic", provider);

        Assert.True(browser.Go("logic@a/b"));

        Assert.NotNull(browser.Place);
        Assert.Equal("logic@a", browser.Place.Parent?.ToString());
        Assert.True(browser.CanGoUp);

        browser.Up();

        Assert.True(browser.IsLogical);
        Assert.Equal("logic@a", browser.Current);

        // A place that names nothing over it is a place the way up cannot leave.
        host.LogicalViews.Register("plain", new Stub());

        Assert.True(browser.Go("plain@a/b"));
        Assert.False(browser.CanGoUp);

        browser.Up();

        Assert.Equal("plain@a/b", browser.Current);
    }

    /// <summary>A provider that says nothing about anything, for the checks that are about the asking.</summary>
    private sealed class Stub : ILogicalViewProvider
    {
        /// <summary>What the place is called, for a check that reads it.</summary>
        public string Title_ { get; init; } = string.Empty;

        /// <summary>What is in it, for a check that reads it.</summary>
        public IReadOnlyList<LogicalEntry> Entries_ { get; init; } = [];

        /// <summary>What is over it, for a check that goes up.</summary>
        public LogicalAddress? Parent_ { get; init; }

        /// <summary>Whether it takes a drag, for a check that offers one.</summary>
        public bool Takes_ { get; init; }

        /// <summary>What was dropped on it, in the order it was dropped.</summary>
        public List<string> Dropped { get; } = [];

        /// <inheritdoc />
        public string Title(LogicalAddress address) => Title_;

        /// <inheritdoc />
        public IReadOnlyList<LogicalEntry> Entries(LogicalAddress address) => Entries_;

        /// <inheritdoc />
        public LogicalAddress? Parent(LogicalAddress address) => Parent_;

        /// <inheritdoc />
        public bool Accepts(LogicalAddress address, LogicalDrag drag) =>
            Takes_ && drag.Data is string;

        /// <inheritdoc />
        public void Drop(LogicalAddress address, LogicalDrag drag)
        {
            if (drag.Data is string said)
            {
                Dropped.Add(said);
            }
        }
    }
}
