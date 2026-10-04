using FileSystemPlugin;
using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The lock extension point: which provider a listing asks, what one contributes to it, and when what it
/// answered is let go of.
/// </summary>
/// <remarks>
/// Nothing here draws — that needs a running Avalonia, and the column and the mark are built out of
/// controls — so what is exercised is the catalogue: a column exists only where a provider speaks for
/// the directory, and which provider that is is what the listing is built from.
/// </remarks>
public sealed class LockTests
{
    /// <summary>Begins each test from no provider at all.</summary>
    public LockTests() => EntryLockProviders.Clear();

    /// <summary>A provider is the one a listing asks only for the directory it speaks for.</summary>
    [Fact]
    public void AProviderIsAskedOnlyForTheDirectoryItSpeaksFor()
    {
        var provider = new FakeLockProvider("it.lock.one", "/work");

        EntryLockProviders.Register(provider);

        Assert.Same(provider, EntryLockProviders.For("/work"));
        Assert.Null(EntryLockProviders.For("/elsewhere"));
    }

    /// <summary>The first provider registered for a directory is the one shown, and the others are not.</summary>
    [Fact]
    public void TheFirstProviderThatSpeaksIsTheOneShown()
    {
        var first = new FakeLockProvider("it.lock.first", "/work");
        var second = new FakeLockProvider("it.lock.second", "/work");

        EntryLockProviders.Register(first);
        EntryLockProviders.Register(second);

        // Both are offered, since a second plugin may speak for another directory.
        Assert.Equal([first, second], EntryLockProviders.All);

        // But one column is one answer, so the first registered answers it.
        Assert.Same(first, EntryLockProviders.For("/work"));
    }

    /// <summary>A provider of an identity already offered is ignored rather than replacing it.</summary>
    [Fact]
    public void AProviderOfAnIdentityAlreadyOfferedIsIgnored()
    {
        var first = new FakeLockProvider("it.lock.same", "/work");
        var second = new FakeLockProvider("it.lock.same", "/other");

        EntryLockProviders.Register(first);
        EntryLockProviders.Register(second);

        Assert.Single(EntryLockProviders.All);
        Assert.Same(first, EntryLockProviders.For("/work"));

        // The one that was ignored is not the one shown for the directory it spoke for either.
        Assert.Null(EntryLockProviders.For("/other"));
    }

    /// <summary>A listing asks for no provider at all until one is registered.</summary>
    [Fact]
    public void NothingSpeaksForADirectoryUntilSomethingIsRegistered()
    {
        Assert.Empty(EntryLockProviders.All);
        Assert.Null(EntryLockProviders.For("/work"));
    }

    /// <summary>What the providers answered is let go of when the files may have changed.</summary>
    /// <remarks>
    /// What a provider answers is read from a tree a file operation changes — a Layout names each path by
    /// a `Uuid`, and a move renames one — and a listing is read again for the same change. A listing read
    /// from an answer that was not let go of is drawn from a tree that is gone, which is what this says
    /// cannot happen again.
    /// </remarks>
    [Fact]
    public void WhatTheProvidersAnsweredIsLetGoOfWhenTheFilesMayHaveChanged()
    {
        var provider = new FakeLockProvider("it.lock.forget", "/work");
        EntryLockProviders.Register(provider);

        var services = Host.Services();
        var host = services.For(new PluginId(FileSystem.Identity), 1);
        var shared = new Shared("/work", new HideRegistry(host.Config), host.Config);

        var readAgain = 0;
        shared.Touched += () => readAgain++;

        shared.FilesChanged();

        Assert.Equal(1, provider.Forgotten);
        Assert.Equal(1, readAgain);
    }

    /// <summary>A provider that speaks for one directory and says one thing about an entry.</summary>
    /// <param name="id">The identity it is offered under.</param>
    /// <param name="directory">The directory it answers for.</param>
    private sealed class FakeLockProvider(string id, string directory) : IEntryLockProvider
    {
        /// <inheritdoc />
        public string Id => id;

        /// <inheritdoc />
        public string ColumnKey => "it.lock.column";

        /// <summary>How many times it was told what it answered may have changed.</summary>
        public int Forgotten { get; private set; }

        /// <inheritdoc />
        public bool Applies(string at) => string.Equals(at, directory, StringComparison.Ordinal);

        /// <inheritdoc />
        public EntryLockMark Mark(Entry entry) => new(TextKey: "it.lock.mine", Ink: LockInk.Accent);

        /// <inheritdoc />
        public void Forget() => Forgotten++;
    }
}
