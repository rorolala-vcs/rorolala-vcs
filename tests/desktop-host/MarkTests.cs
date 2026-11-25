using FileSystemPlugin;
using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The badge extension point: which providers a tile asks, and in what order their badges are numbered.
/// </summary>
/// <remarks>
/// Nothing here draws (that needs a renderer, and a badge is a control) so what is exercised is the
/// catalogue: a badge exists only where a provider speaks for the directory, and the order the providers
/// were offered in is what says which badge of the row is whose.
/// </remarks>
public sealed class MarkTests
{
    /// <summary>Begins each test from no provider at all.</summary>
    public MarkTests() => EntryMarkProviders.Clear();

    /// <summary>Every provider that speaks for a directory is asked, in the order they were offered.</summary>
    /// <remarks>
    /// Every one rather than the first, which is where this differs from the Ownership column: a tile wears as
    /// many badges as there are answers about it, and the order is what makes a badge stand in the same place
    /// of the row from one listing to the next.
    /// </remarks>
    [Fact]
    public void EveryProviderThatSpeaksIsAskedInTheOrderItWasOffered()
    {
        var first = new FakeMarkProvider("it.mark.first", "/work");
        var second = new FakeMarkProvider("it.mark.second", "/work");
        var elsewhere = new FakeMarkProvider("it.mark.elsewhere", "/other");

        EntryMarkProviders.Register(first);
        EntryMarkProviders.Register(second);
        EntryMarkProviders.Register(elsewhere);

        Assert.Equal([first, second, elsewhere], EntryMarkProviders.All);
        Assert.Equal([first, second], EntryMarkProviders.For("/work"));
        Assert.Equal([elsewhere], EntryMarkProviders.For("/other"));
        Assert.Empty(EntryMarkProviders.For("/nowhere"));
    }

    /// <summary>A provider of an identity already offered is ignored rather than replacing it.</summary>
    /// <remarks>
    /// Two plugins claiming one name would otherwise be two badges of the row whose order, and so whose
    /// positions, depended on which of them happened to be loaded second.
    /// </remarks>
    [Fact]
    public void AProviderOfAnIdentityAlreadyOfferedIsIgnored()
    {
        var first = new FakeMarkProvider("it.mark.same", "/work");
        var second = new FakeMarkProvider("it.mark.same", "/other");

        EntryMarkProviders.Register(first);
        EntryMarkProviders.Register(second);

        Assert.Equal([first], EntryMarkProviders.All);
        Assert.Equal([first], EntryMarkProviders.For("/work"));
        Assert.Empty(EntryMarkProviders.For("/other"));
    }

    /// <summary>What every provider answered is let go of when the files may have changed.</summary>
    /// <remarks>
    /// Said to all of them rather than to the ones a directory is answered by, because what a provider holds
    /// is its own business and one with nothing to let go of answers nothing to it.
    /// </remarks>
    [Fact]
    public void WhatTheProvidersAnsweredIsLetGoOf()
    {
        var first = new FakeMarkProvider("it.mark.one", "/work");
        var second = new FakeMarkProvider("it.mark.two", "/other");

        EntryMarkProviders.Register(first);
        EntryMarkProviders.Register(second);
        EntryMarkProviders.Forget();

        Assert.Equal(1, first.Forgotten);
        Assert.Equal(1, second.Forgotten);
    }

    /// <summary>A provider is asked about the directory once, however many rows are drawn from it.</summary>
    /// <remarks>
    /// The contract says a provider answers for a directory rather than for an entry, and a listing is one
    /// directory: what a row asks is what the provider already answered for the directory it sits in.
    /// </remarks>
    [Fact]
    public void AProviderIsAskedOnceForADirectoryHoweverManyRowsAreDrawn()
    {
        var root = Scratch.New("mark-once");

        for (var at = 0; at < 20; at++)
        {
            File.WriteAllText(Path.Combine(root, $"file-{at}.txt"), "x");
        }

        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared(root, new HideRegistry(host.Config), host.Config, host.Files);
        var browser = new Browser(host.Log, shared, root, host.LogicalViews);
        var clip = new Clip(host.Log);
        var files = new Files(host, browser, new BrowserActions(host, browser, clip), clip);
        var counting = new CountingMarkProvider(root);

        EntryMarkProviders.Register(counting);

        foreach (var entry in files.Items)
        {
            files.Badges(entry);
        }

        Assert.Equal(1, counting.AppliesCount);
        Assert.Equal(20, counting.MarkCount);
    }

    /// <summary>A provider that speaks for one directory and says one thing about an entry.</summary>
    /// <param name="id">The identity it is offered under.</param>
    /// <param name="directory">The directory it answers for.</param>
    private sealed class FakeMarkProvider(string id, string directory) : IEntryMarkProvider
    {
        /// <inheritdoc />
        public string Id => id;

        /// <summary>How many times it was told what it answered may have changed.</summary>
        public int Forgotten { get; private set; }

        /// <inheritdoc />
        public bool Applies(string at) => string.Equals(at, directory, StringComparison.Ordinal);

        /// <inheritdoc />
        public EntryMark Mark(Entry entry) => new(IconKey: "it.mark.picture", Ink: MarkInk.Accent);

        /// <inheritdoc />
        public void Forget() => Forgotten++;
    }

    /// <summary>A provider that counts how often it was asked, and says nothing about any entry.</summary>
    private sealed class CountingMarkProvider(string directory) : IEntryMarkProvider
    {
        /// <inheritdoc />
        public string Id => "it.mark.counting";

        /// <summary>How many times the provider was asked whether it speaks for a directory.</summary>
        public int AppliesCount { get; private set; }

        /// <summary>How many entries it was asked about.</summary>
        public int MarkCount { get; private set; }

        /// <inheritdoc />
        public bool Applies(string at)
        {
            AppliesCount++;

            return string.Equals(at, directory, StringComparison.Ordinal);
        }

        /// <inheritdoc />
        public EntryMark Mark(Entry entry)
        {
            MarkCount++;

            return new EntryMark();
        }

        /// <inheritdoc />
        public void Forget() { }
    }
}
