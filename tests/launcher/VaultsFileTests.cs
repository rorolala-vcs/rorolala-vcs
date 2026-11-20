using RorolalaLauncher;

namespace RolalalaLauncher.IntegrationTests;

/// <summary>
/// The addresses this launcher hosts: the file they are kept in, and the rule that an address is listed
/// once.
/// </summary>
public sealed class VaultsFileTests
{
    /// <summary>An address survives being written and read.</summary>
    [Fact]
    public void TheHostedAddressesSurviveARoundTrip()
    {
        var directory = Scratch.New("vaults");
        var path = Path.Combine(directory, "vaults.toml");
        var file = new VaultsFile();

        file.Entries.Add(new VaultEntry("atlas", "rola://atlas.example.com:7420/staff", VaultSource.Hosted));
        file.Entries.Add(new VaultEntry("work", "rola://work.example.com:7420/archive", VaultSource.Hosted));
        file.Write(path);

        var read = VaultsFile.Read(path);

        Assert.Collection(
            read.Entries,
            entry =>
            {
                Assert.Equal("atlas", entry.Name);
                Assert.Equal("rola://atlas.example.com:7420/staff", entry.Address);
            },
            entry =>
            {
                Assert.Equal("work", entry.Name);
                Assert.Equal("rola://work.example.com:7420/archive", entry.Address);
            }
        );
    }

    /// <summary>A hand-written file with a comment and a blank line is read.</summary>
    [Fact]
    public void AHandWrittenFileIsRead()
    {
        var directory = Scratch.New("vaults-hand");
        var path = Path.Combine(directory, "vaults.toml");

        File.WriteAllText(
            path,
            "# The addresses this launcher hosts\n[[vault]]\nname = \"atlas\"\n"
                + "address = \"rola://atlas.example.com:7420/staff\" # the studio\n"
        );

        var read = VaultsFile.Read(path);

        Assert.Single(read.Entries);
        Assert.Equal("atlas", read.Entries[0].Name);
    }

    /// <summary>Two spellings of one address are one address, and the edited entry is left out.</summary>
    [Fact]
    public void AnAddressIsListedOnce()
    {
        var file = new VaultsFile();
        var entry = new VaultEntry("atlas", "rola://atlas.example.com:7420/staff", VaultSource.Hosted);

        file.Entries.Add(entry);

        Assert.True(file.Contains("ROLA://ATLAS.EXAMPLE.COM:7420/STAFF"));
        Assert.False(file.Contains("ROLA://ATLAS.EXAMPLE.COM:7420/STAFF", entry));
        Assert.False(file.Contains("rola://other.example.com:7420/staff"));
    }
}
