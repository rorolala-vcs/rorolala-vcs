using RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The accounts dock: what the command line says read back into the accounts and the keys it named.
/// </summary>
/// <remarks>
/// The runs themselves are the command line's and are not made here — a run reads the machine's own key stores
/// and may write a pair — so what is checked is everything this plugin decides without one: what a listing and
/// a key listing come to, which name a key file stands for, and which names an account may not have.
/// </remarks>
public sealed class AccountsTests
{
    /// <summary>What `rola account` printed is read into the accounts, with the one in use marked.</summary>
    [Fact]
    public void WhatTheCommandListedIsReadIntoTheAccounts()
    {
        var accounts = AccountActions.Listed("""{"names":["alice","bob"],"current":"bob"}""");

        Assert.Equal(["alice", "bob"], accounts.Select(account => account.Name));
        Assert.False(accounts[0].Current);
        Assert.True(accounts[1].Current);

        // No account in use is not the same as no accounts: nothing is marked, and both are still listed.
        var none = AccountActions.Listed("""{"names":["alice"],"current":null}""");

        Assert.Single(none);
        Assert.False(none[0].Current);

        // And an answer that cannot be read is answered as no accounts rather than as a throw, so what the
        // command said is what is shown.
        foreach (var unreadable in new[] { "", "not json at all", """{"current":"alice"}""", "[]" })
        {
            Assert.Empty(AccountActions.Listed(unreadable));
        }
    }

    /// <summary>What `rola key` printed is read into the paths it listed.</summary>
    [Fact]
    public void WhatTheKeyListingPrintedIsReadIntoThePaths()
    {
        var paths = AccountActions.PathsOf(
            """{"paths":["/keys/alice.pem","/keys/bob.pem"]}"""
        );

        Assert.Equal(["/keys/alice.pem", "/keys/bob.pem"], paths);

        foreach (var unreadable in new[] { "", "not json at all", """{"kinds":[]}""", "[]" })
        {
            Assert.Empty(AccountActions.PathsOf(unreadable));
        }
    }

    /// <summary>A key file's account is its stem, so the private and public halves name one pair.</summary>
    [Fact]
    public void AKeyFileIsNamedByItsStem()
    {
        Assert.Equal("alice", AccountActions.Named("/keys/alice.pem"));
        Assert.Equal("alice", AccountActions.Named("C:\\keys\\alice.pub"));
        Assert.Equal("alice", AccountActions.Named("alice.pem"));
        Assert.Equal("alice", AccountActions.Named("alice"));

        // A path that names no file stem is no account, rather than one with an empty name.
        Assert.Null(AccountActions.Named("/keys/"));
        Assert.Null(AccountActions.Named(string.Empty));
    }

    /// <summary>
    /// The two names `rola account` reads as its own words cannot be an account.
    /// </summary>
    /// <remarks>
    /// A name is given to `rola account` as a word, and a word it knows as a subcommand is that subcommand
    /// rather than an account: an account called `ls` would list the accounts instead of being chosen, and one
    /// called `current` would print the account in use.
    /// </remarks>
    [Fact]
    public void TheNamesTheCommandReadsAsItsOwnAreRefused()
    {
        Assert.Equal(["current", "ls"], AccountActions.Forbidden);
        Assert.Contains("current", AccountActions.Forbidden);
        Assert.Contains("ls", AccountActions.Forbidden);

        // A name that merely contains one is a name like any other: what is refused is the word itself, so the
        // reading is exact rather than a search.
        Assert.DoesNotContain("Current", AccountActions.Forbidden);
        Assert.DoesNotContain("lsa", AccountActions.Forbidden);
    }
}
