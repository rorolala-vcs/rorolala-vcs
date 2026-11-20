namespace RorolalaLauncher;

/// <summary>
/// The addresses this launcher hosts, kept in <c>vaults.toml</c>.
/// </summary>
/// <remarks>
/// The address book is the daemon's registry and is never written here. This file holds only what the
/// launcher added itself, so that removing an entry never touches the registry and adding one never
/// pretends to be a daemon registration.
/// <para>
/// The shape is two keys under one table, one table a Vault. It is read a line at a time, like the
/// other files this program keeps; a line the file does not state is ignored rather than refused, so a
/// hand-edited file that gained a comment still opens.
/// </para>
/// </remarks>
internal sealed class VaultsFile
{
    /// <summary>The table one hosted address is written under.</summary>
    private const string Table = "[[vault]]";

    /// <summary>The key an entry's name is written under.</summary>
    private const string NameKey = "name";

    /// <summary>The key an entry's address is written under.</summary>
    private const string AddressKey = "address";

    /// <summary>Every hosted address, in the order the file names them.</summary>
    public List<VaultEntry> Entries { get; } = [];

    /// <summary>Reads the file, or answers an empty list when there is none.</summary>
    /// <param name="path">The file.</param>
    /// <returns>What it names.</returns>
    /// <exception cref="IOException">The file is there and could not be read.</exception>
    public static VaultsFile Read(string path)
    {
        var file = new VaultsFile();

        if (!File.Exists(path))
        {
            return file;
        }

        file.Entries.AddRange(ReadEntries(File.ReadAllText(path)));

        return file;
    }

    /// <summary>
    /// Whether an address is already in the file.
    /// </summary>
    /// <remarks>
    /// Addresses are compared without regard to case: the host of an address is not case sensitive, and
    /// two entries that reach the same daemon are one entry whatever they are spelled.
    /// </remarks>
    /// <param name="address">The address.</param>
    /// <param name="except">An entry to leave out of the comparison, when editing.</param>
    /// <returns>Whether it is already there.</returns>
    public bool Contains(string address, VaultEntry? except = null) =>
        Entries.Exists(held =>
            !ReferenceEquals(held, except)
            && string.Equals(held.Address, address, StringComparison.OrdinalIgnoreCase)
        );

    /// <summary>Removes an entry, answering whether it was there.</summary>
    /// <param name="entry">The entry.</param>
    /// <returns>Whether it was on the list.</returns>
    public bool Remove(VaultEntry entry) => Entries.Remove(entry);

    /// <summary>Writes the file, making the directory it sits in.</summary>
    /// <param name="path">The file.</param>
    /// <exception cref="IOException">The file could not be written.</exception>
    public void Write(string path)
    {
        var directory = Path.GetDirectoryName(path);

        if (!string.IsNullOrEmpty(directory))
        {
            Directory.CreateDirectory(directory);
        }

        var text = new System.Text.StringBuilder();

        foreach (var entry in Entries)
        {
            text.Append(Table).Append('\n');
            text.Append(NameKey).Append(" = ").Append(TomlLine.Quote(entry.Name)).Append('\n');
            text.Append(AddressKey).Append(" = ").Append(TomlLine.Quote(entry.Address)).Append('\n');
            text.Append('\n');
        }

        File.WriteAllText(path, text.ToString());
    }

    /// <summary>
    /// The entries a file states.
    /// </summary>
    /// <remarks>
    /// A table is taken when its bracket line is reached, and its two keys are read until the next one:
    /// a table missing either key is skipped rather than kept half read, since an address with no name
    /// is not something the dock can draw.
    /// </remarks>
    /// <param name="text">The file's text.</param>
    /// <returns>Every entry it states.</returns>
    private static List<VaultEntry> ReadEntries(string text)
    {
        var entries = new List<VaultEntry>();
        string? name = null;
        string? address = null;

        void Flush()
        {
            if (!string.IsNullOrWhiteSpace(name) && !string.IsNullOrWhiteSpace(address))
            {
                entries.Add(new VaultEntry(name, address, VaultSource.Hosted));
            }

            name = null;
            address = null;
        }

        foreach (var line in text.Split('\n'))
        {
            var trimmed = line.Trim();

            if (string.Equals(trimmed, Table, StringComparison.Ordinal))
            {
                Flush();

                continue;
            }

            var (key, value) = TomlLine.Split(trimmed);

            if (key is null || value is null)
            {
                continue;
            }

            if (string.Equals(key, NameKey, StringComparison.Ordinal))
            {
                name = TomlLine.String(value);
            }
            else if (string.Equals(key, AddressKey, StringComparison.Ordinal))
            {
                address = TomlLine.String(value);
            }
        }

        Flush();

        return entries;
    }
}
