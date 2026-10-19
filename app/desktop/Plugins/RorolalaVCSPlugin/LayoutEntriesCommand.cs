using System.Text.Json;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>One Layout, as <c>rola layout ls --json</c> lists it.</summary>
/// <param name="Name">The name it is known by.</param>
/// <param name="Current">Whether it is the one the Workspace is working in.</param>
/// <param name="Tracking">The Vault it tracks, empty when it tracks none.</param>
internal sealed record LayoutListing(string Name, bool Current, string Tracking);

/// <summary>One Vault the Workspace knows, as <c>rola vault ls --json</c> lists it.</summary>
/// <param name="Name">The name the Workspace bound it under.</param>
/// <param name="Default">
/// Whether it is the Workspace's default Vault, which is the one the work's own upstream is opened at when the Layout
/// being worked in tracks none.
/// </param>
internal sealed record VaultListing(string Name, bool Default);

/// <summary>One entry, as <c>rola layout entries --json</c> lists it on either side of the work.</summary>
/// <remarks>
/// The two sides answer the same shape because they are the same command: <c>rola entries</c> resolves which Layout
/// to read and hands the listing to <c>rola layout entries</c>, so a row of the Vault's copy and a row of this one
/// differ in what they say and not in how they are read.
/// <para>
/// A path is empty for an entry the side names nowhere. That is not a row to draw — the reading it comes from
/// says where the entry is, and one that is nowhere is not a place — but it is still an entry the side holds,
/// which is what the uuid is kept for.
/// </para>
/// </remarks>
/// <param name="Path">The path it is at, relative to the Workspace's root; empty when it is at none.</param>
/// <param name="Uuid">The uuid it is known by.</param>
/// <param name="Version">The version it holds, as the command line writes a hash.</param>
/// <param name="Owner">The account that holds it, empty when none does.</param>
/// <param name="Description">What it says about itself.</param>
/// <param name="Deprecated">Whether the Vault has deprecated it, by moving it under <c>@/removed/</c>.</param>
internal sealed record LayoutEntry(
    string Path,
    string Uuid,
    string Version,
    string Owner,
    string Description,
    bool Deprecated
)
{
    /// <summary>The last step of its path, which is what a tree draws it by.</summary>
    public string Name => Path[(Path.LastIndexOf('/') + 1)..];
}

/// <summary>What a listing of the Layouts came to.</summary>
/// <param name="Done">Whether the command ended well.</param>
/// <param name="Code">The status it ended with, which is negative when it could not be run.</param>
/// <param name="Said">What it said when it did not end well, or when its answer could not be read.</param>
/// <param name="Layouts">The Layouts it listed.</param>
internal sealed record LayoutsAnswer(
    bool Done,
    int Code,
    string Said,
    IReadOnlyList<LayoutListing> Layouts
);

/// <summary>What a listing of the entries came to.</summary>
/// <param name="Done">Whether the command ended well.</param>
/// <param name="Code">The status it ended with, which is negative when it could not be run.</param>
/// <param name="Said">What it said when it did not end well, or when its answer could not be read.</param>
/// <param name="Entries">The entries it listed.</param>
internal sealed record EntriesAnswer(
    bool Done,
    int Code,
    string Said,
    IReadOnlyList<LayoutEntry> Entries
);

/// <summary>The Vaults a Workspace knows, and what a listing of them came to.</summary>
/// <param name="Done">Whether the command ended well.</param>
/// <param name="Code">The status it ended with, which is negative when it could not be run.</param>
/// <param name="Said">What it said when it did not end well, or when its answer could not be read.</param>
/// <param name="Vaults">The Vaults it listed, in the order it named them.</param>
internal sealed record VaultsAnswer(bool Done, int Code, string Said, IReadOnlyList<VaultListing> Vaults);

/// <summary>
/// Asking the command line what a Layout holds: the Layouts a run works in, and the entries of one.
/// </summary>
/// <remarks>
/// A Layout is the command line's to read — it is a set of shards whose names mean something only to the library —
/// so what this owns is the question, the flags that make the answer one a program reads, and the shape of the
/// answer.
/// <para>
/// The Vault's copy is read through <c>--layout truth@VAULT</c> rather than through <c>--remote</c>: a place names
/// which Vault it is the upstream of, and the one the Layout being worked in tracks is only one of them.
/// </para>
/// <para>
/// The status a run ended with is answered beside its words because one of the ways it fails is a state rather
/// than a fault: a copy that was never fetched is what the place offers a fetch for, and telling it from a copy
/// that cannot be read means reading the <c>EC_ERR_LAYOUT_NOT_CACHED</c> status rather than the message, which
/// is in the reader's language.
/// </para>
/// </remarks>
internal static class LayoutEntriesCommand
{
    /// <summary>
    /// The status <c>rola layout entries</c> ends with when the Vault's copy was never fetched.
    /// </summary>
    /// <remarks>
    /// It is the command line's own <c>EC_ERR_LAYOUT_NOT_CACHED</c>, stated here because the two programs share
    /// no header: the number is the contract across the boundary, so it is written where it is used.
    /// </remarks>
    public const int NotCached = 192;

    /// <summary>The arguments the Layouts are listed with, after the program's name.</summary>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> LayoutsArguments() =>
        ["--theme-choice=" + Sync.Theme, "--json", "layout", "ls"];

    /// <summary>The arguments the Vaults a Workspace knows are listed with.</summary>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> VaultsArguments() =>
        ["--theme-choice=" + Sync.Theme, "--json", "vault", "ls"];

    /// <summary>The arguments the entries of the Layout being worked in are listed with.</summary>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> LocalArguments() =>
        ["--theme-choice=" + Sync.Theme, "--json", "entries"];

    /// <summary>The arguments the entries of one Vault's own Layout are listed with.</summary>
    /// <param name="vault">The Vault whose Layout is read.</param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> UpstreamArguments(string vault)
    {
        ArgumentNullException.ThrowIfNull(vault);

        return
        [
            "--theme-choice=" + Sync.Theme,
            "--json",
            "layout",
            "entries",
            "--layout",
            LayoutPathMoveCommand.Named(vault),
        ];
    }

    /// <summary>The arguments one Layout of the Workspace's own is listed with.</summary>
    /// <remarks>
    /// The Layout is named rather than left to be resolved, since what a place shows is the one the address names and not
    /// the one being worked in.
    /// </remarks>
    /// <param name="layout">The Layout's name.</param>
    /// <returns>The arguments.</returns>
    public static IReadOnlyList<string> LayoutArguments(string layout)
    {
        ArgumentNullException.ThrowIfNull(layout);

        return ["--theme-choice=" + Sync.Theme, "--json", "layout", "entries", "--layout", layout];
    }

    /// <summary>Asks which Layouts the run works in, and which of them tracks what.</summary>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <returns>What the run came to, and the Layouts it listed.</returns>
    public static LayoutsAnswer Layouts(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        if (Program() is not { } program)
        {
            return new LayoutsAnswer(false, Sync.FailedToRun, NoProgram(), []);
        }

        var (code, said, output) = Sync.Coded(program, LayoutsArguments(), directory);

        if (code != 0)
        {
            return new LayoutsAnswer(false, code, said, []);
        }

        try
        {
            return new LayoutsAnswer(true, code, string.Empty, ParseLayouts(output));
        }
        catch (JsonException error)
        {
            return new LayoutsAnswer(false, code, error.Message, []);
        }
    }

    /// <summary>Asks what the Layout being worked in holds.</summary>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <returns>What the run came to, and the entries it listed.</returns>
    public static EntriesAnswer Local(string directory) => Read(directory, LocalArguments());

    /// <summary>Asks what one Vault's own Layout holds, as the copy the Workspace fetched has it.</summary>
    /// <remarks>
    /// The copy is read and nothing is fetched for it: a place that quietly reached for the Vault would be a view of the
    /// work that talks to it whenever it is opened.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="vault">The Vault whose Layout is read.</param>
    /// <returns>What the run came to, and the entries it listed.</returns>
    public static EntriesAnswer Upstream(string directory, string vault) =>
        Read(directory, UpstreamArguments(vault));

    /// <summary>Asks what one Layout of the Workspace's own holds.</summary>
    /// <remarks>
    /// Read from the Layout itself rather than from a copy of the Vault's: a Layout of the Workspace's is the work's own
    /// record, and there is nothing to fetch for it.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="layout">The Layout's name.</param>
    /// <returns>What the run came to, and the entries it listed.</returns>
    public static EntriesAnswer Layout(string directory, string layout) =>
        Read(directory, LayoutArguments(layout));

    /// <summary>Asks which Vaults the Workspace knows.</summary>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <returns>What the run came to, and the Vaults it named.</returns>
    public static VaultsAnswer Vaults(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        if (Program() is not { } program)
        {
            return new VaultsAnswer(false, Sync.FailedToRun, NoProgram(), []);
        }

        var (code, said, output) = Sync.Coded(program, VaultsArguments(), directory);

        if (code != 0)
        {
            return new VaultsAnswer(false, code, said, []);
        }

        try
        {
            return new VaultsAnswer(true, code, string.Empty, ParseVaults(output));
        }
        catch (JsonException error)
        {
            return new VaultsAnswer(false, code, error.Message, []);
        }
    }

    /// <summary>
    /// Reads the Vaults the command answered with.
    /// </summary>
    /// <remarks>
    /// A Vault is a name and an address, which the command writes as an object: the name is what a place is keyed by, and
    /// the address is where it answers — which this side has no use for, since a run inside the Workspace reaches it by
    /// the name the Workspace bound it under. Whether a Vault is the Workspace's default is read as well, because that is
    /// the one the work's own upstream is opened at when the Layout being worked in tracks none.
    /// </remarks>
    /// <param name="output">What it wrote, which is one JSON object when it was asked for one.</param>
    /// <returns>The Vaults, in the order the command listed them.</returns>
    /// <exception cref="JsonException">The output is not JSON.</exception>
    internal static IReadOnlyList<VaultListing> ParseVaults(string output)
    {
        ArgumentNullException.ThrowIfNull(output);

        using var document = JsonDocument.Parse(output);

        if (Items(document.RootElement, "vaults") is not { } items)
        {
            return [];
        }

        var found = new List<VaultListing>(items.GetArrayLength());

        foreach (var item in items.EnumerateArray())
        {
            if (Text(item, "name") is not { Length: > 0 } name)
            {
                continue;
            }

            found.Add(
                new VaultListing(
                    name,
                    item.TryGetProperty("is_default", out var isDefault)
                        && isDefault.ValueKind is JsonValueKind.True
                )
            );
        }

        return found;
    }

    /// <summary>Reads the listing the command answered with.</summary>
    /// <param name="output">What it wrote, which is one JSON object when it was asked for one.</param>
    /// <returns>The Layouts it named.</returns>
    /// <exception cref="JsonException">The output is not JSON.</exception>
    internal static IReadOnlyList<LayoutListing> ParseLayouts(string output)
    {
        ArgumentNullException.ThrowIfNull(output);

        using var document = JsonDocument.Parse(output);

        if (Items(document.RootElement, "layouts") is not { } items)
        {
            return [];
        }

        var found = new List<LayoutListing>(items.GetArrayLength());

        foreach (var item in items.EnumerateArray())
        {
            if (Text(item, "name") is not { Length: > 0 } name)
            {
                continue;
            }

            found.Add(
                new LayoutListing(
                    name,
                    item.TryGetProperty("is_current", out var current)
                        && current.ValueKind is JsonValueKind.True,
                    Text(item, "tracking") ?? string.Empty
                )
            );
        }

        return found;
    }

    /// <summary>Reads the entries the command answered with.</summary>
    /// <remarks>
    /// A row no uuid is given is not an entry, so it is left out rather than drawn as one. A row at no path is
    /// kept, because what it is is an entry this side holds and cannot name: the caller that draws a tree
    /// leaves it out, and the caller that asks whether something is already held needs it.
    /// </remarks>
    /// <param name="output">What it wrote, which is one JSON object when it was asked for one.</param>
    /// <returns>The entries it named.</returns>
    /// <exception cref="JsonException">The output is not JSON.</exception>
    internal static IReadOnlyList<LayoutEntry> ParseEntries(string output)
    {
        ArgumentNullException.ThrowIfNull(output);

        using var document = JsonDocument.Parse(output);

        if (Items(document.RootElement, "entries") is not { } items)
        {
            return [];
        }

        var found = new List<LayoutEntry>(items.GetArrayLength());

        foreach (var item in items.EnumerateArray())
        {
            if (Text(item, "uuid") is not { Length: > 0 } uuid)
            {
                continue;
            }

            found.Add(
                new LayoutEntry(
                    Text(item, "path") ?? string.Empty,
                    uuid,
                    Text(item, "version") ?? string.Empty,
                    Text(item, "owner") ?? string.Empty,
                    Text(item, "description") ?? string.Empty,
                    item.TryGetProperty("deprecated", out var deprecated)
                        && deprecated.ValueKind is JsonValueKind.True
                )
            );
        }

        return found;
    }

    /// <summary>Runs one listing of the entries and reads what it answered.</summary>
    /// <param name="directory">Where the run is made.</param>
    /// <param name="arguments">What it is run with.</param>
    /// <returns>What the run came to, and the entries it listed.</returns>
    private static EntriesAnswer Read(string directory, IReadOnlyList<string> arguments)
    {
        ArgumentNullException.ThrowIfNull(directory);

        if (Program() is not { } program)
        {
            return new EntriesAnswer(false, Sync.FailedToRun, NoProgram(), []);
        }

        var (code, said, output) = Sync.Coded(program, arguments, directory);

        if (code != 0)
        {
            return new EntriesAnswer(false, code, said, []);
        }

        try
        {
            return new EntriesAnswer(true, code, string.Empty, ParseEntries(output));
        }
        catch (JsonException error)
        {
            return new EntriesAnswer(false, code, error.Message, []);
        }
    }

    /// <summary>The array under a member of the answer, or nothing when the answer holds none.</summary>
    /// <param name="root">The answer.</param>
    /// <param name="name">The member's name.</param>
    /// <returns>The array, or nothing.</returns>
    private static JsonElement? Items(JsonElement root, string name) =>
        root.ValueKind is JsonValueKind.Object
        && root.TryGetProperty(name, out var items)
        && items.ValueKind is JsonValueKind.Array
            ? items
            : null;

    /// <summary>The string a member of an object holds, or nothing when it holds none.</summary>
    /// <param name="item">The object.</param>
    /// <param name="name">The member's name.</param>
    /// <returns>The text, or nothing.</returns>
    private static string? Text(JsonElement item, string name) =>
        item.ValueKind is JsonValueKind.Object
        && item.TryGetProperty(name, out var value)
        && value.ValueKind is JsonValueKind.String
            ? value.GetString()
            : null;

    /// <summary>
    /// The program the window was opened by, or nothing when this window was opened on its own.
    /// </summary>
    /// <remarks>
    /// It is the environment <c>rola desktop</c> hands over rather than a <c>rola</c> found on the <c>PATH</c>:
    /// the two are exported together, and a program found elsewhere could be another version against another
    /// Vault.
    /// </remarks>
    /// <returns>The program, or nothing.</returns>
    private static string? Program() =>
        Environment.GetEnvironmentVariable(Sync.Program) is { Length: > 0 } program ? program : null;

    /// <summary>What is said when there is no program to ask, which is a window opened on its own.</summary>
    /// <returns>The message.</returns>
    private static string NoProgram() => RolaI18N.Get("rorolala_vcs.sync.no_program");
}
