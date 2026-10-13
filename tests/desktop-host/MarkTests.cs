using FileSystemPlugin;
using RorolalaDesktop.Contract;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The badge extension point: which providers a tile asks, and in what order their badges are numbered.
/// </summary>
/// <remarks>
/// Nothing here draws — that needs a renderer, and a badge is a control — so what is exercised is the
/// catalogue: a badge exists only where a provider speaks for the directory, and the order the providers
/// were offered in is what says which badge of the arc is whose.
/// </remarks>
public sealed class MarkTests
{
    /// <summary>Begins each test from no provider at all.</summary>
    public MarkTests() => EntryMarkProviders.Clear();

    /// <summary>Every provider that speaks for a directory is asked, in the order they were offered.</summary>
    /// <remarks>
    /// Every one rather than the first, which is where this differs from the Ownership column: a tile wears as
    /// many badges as there are answers about it, and the order is what makes a badge stand in the same place
    /// of the arc from one listing to the next.
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
    /// Two plugins claiming one name would otherwise be two badges of the arc whose order, and so whose
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
}
