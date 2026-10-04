using RorolalaDesktop.Contract;

namespace FileSystemPlugin;

/// <summary>
/// One way of deciding that an entry is hidden.
/// </summary>
/// <remarks>
/// This is the extension point, reached by depending on this plugin: a manifest that names
/// <c>rorolala.file_system</c> in its dependencies is started after it, and its <c>Initialize</c> may
/// hand one of these to <see cref="HideRegistry.Register"/>. Hiding is not exclusive: an entry is
/// hidden as soon as one provider in force says so, and the providers that say so are what the browser
/// offers to switch off.
/// <para>
/// A provider answers about an entry rather than about a path, because whether something is a file or
/// a directory is what the listing already read and what a rule like "a dot-file" needs. What the
/// answer costs — an attribute read, a command — is the provider's to keep cheap: it is asked at
/// staging time, and a listing that is staged again asks it again.
/// </para>
/// </remarks>
public interface IEntryHideProvider
{
    /// <summary>
    /// The stable value this provider is switched by, unique among the providers offered.
    /// </summary>
    /// <remarks>
    /// It is written into <c>preference.json</c> as one value of the <c>Hides/providers</c> setting, so
    /// it must not change once shipped.
    /// </remarks>
    string Id { get; }

    /// <summary>An i18n key naming what the provider hides.</summary>
    string LabelKey { get; }

    /// <summary>
    /// Whether this provider hides an entry.
    /// </summary>
    /// <remarks>
    /// A provider that cannot answer — a command that is not installed, an item it may not inspect —
    /// says no rather than failing the listing: hiding something for not being answerable is the worse
    /// mistake, and the entry is shown.
    /// </remarks>
    /// <param name="entry">The entry to consider.</param>
    /// <returns>Whether the provider hides it.</returns>
    bool Hides(Entry entry);

    /// <summary>
    /// Whether this provider hides an entry, read against the place the view is rooted at.
    /// </summary>
    /// <remarks>
    /// A rule that is about a repository — Git's — has to be read from somewhere, and the entry's own
    /// directory is not always that somewhere: a tree is rooted at a base and reads the directories
    /// under it, so a rule read from each directory in turn would let a repository nested inside the
    /// base answer for itself. What is given is the place the view is rooted at, which is the base for
    /// a tree and the directory being listed for a listing.
    /// <para>
    /// A provider with no such notion answers as it would without it, which is what the default does:
    /// whether a name begins with a dot, or what the platform marks hidden, is not a question about a
    /// repository.
    /// </para>
    /// </remarks>
    /// <param name="entry">The entry to consider.</param>
    /// <param name="root">The directory the view reading the entry is rooted at.</param>
    /// <returns>Whether the provider hides it.</returns>
    bool Hides(Entry entry, string root) => Hides(entry);

    /// <summary>
    /// Says that what this provider answered may have changed.
    /// </summary>
    /// <remarks>
    /// A rule that is about a repository is asked of a program once per directory and kept, since asking it
    /// per entry would be a process per entry. The rules themselves are files, though — a <c>.gitignore</c>
    /// is edited, a repository is set up in a directory that had none — so a kept answer can outlive what it
    /// was read from. A provider that keeps anything lets go of it here; one that reads afresh each time has
    /// nothing to do, which is what the default does.
    /// </remarks>
    void Forget() { }
}

/// <summary>
/// The hide providers in force, and where a dependent plugin adds its own.
/// </summary>
/// <remarks>
/// One is made per plugin run, holds the host's own configuration so that a provider switched off is
/// asked nothing, and is handed to every location and dock: whether an entry is hidden is one answer
/// for the whole File System, or two docks listing one directory would list different things.
/// <para>
/// The providers are offered to the user as one multi-choice setting, <c>Hides/providers</c>, whose
/// options are the providers registered by the time the preference panel is drawn. A dependent plugin
/// adds its provider during its own <c>Initialize</c>, which the host runs after this plugin's — so the
/// setting's options list is the live catalogue rather than a copy, and the option a dependent plugin
/// adds is offered even though this plugin declared the setting first (the same shape
/// <see cref="FileOperationPresets"/> uses for its presets).
/// </para>
/// </remarks>
public sealed class HideRegistry
{
    /// <summary>The setting the chosen providers are kept under.</summary>
    public const string Setting = "Hides/providers";

    /// <summary>
    /// Every provider registered this run, in the order they are offered.
    /// </summary>
    /// <remarks>
    /// One list for the whole run rather than one per registry, because a registry is made wherever a
    /// setting is read — by the plugin, by a dock, by a dependent plugin adding its provider — and every one
    /// of them has to see the same catalogue: a dependent plugin's provider registered through one registry
    /// would otherwise be invisible to a listing staged through another. The options the setting is declared
    /// with are the same catalogue in the shape a setting takes, and are added to together.
    /// </remarks>
    private static readonly List<IEntryHideProvider> Catalogue = [];

    /// <summary>The catalogue as the setting offers it, which is what the preference panel reads.</summary>
    private static readonly List<SettingOption> Offer = [];

    /// <summary>The plugin's own settings, which say which providers are in force.</summary>
    private readonly IPluginConfig _config;

    /// <summary>How the i18n keys of this plugin's own providers are namespaced.</summary>
    private const string Key = "rorolala_file_system.hides.";

    /// <summary>Makes a registry over the plugin's own settings.</summary>
    /// <param name="config">The settings the chosen providers are read from.</param>
    public HideRegistry(IPluginConfig config) => _config = config;

    /// <summary>
    /// Every provider, in the order they are offered.
    /// </summary>
    /// <remarks>
    /// Static because the catalogue is: the providers are the run's, and an instance of this class is only a
    /// reading of the settings that choose among them.
    /// </remarks>
    public static IReadOnlyList<IEntryHideProvider> All => Catalogue;

    /// <summary>The options the setting offers, as a reader outside the plugin sees them.</summary>
    /// <remarks>
    /// The same list object the setting was declared with, so that a provider a dependent plugin registered
    /// after that declaration is in it — which is the whole point of the catalogue being one list.
    /// </remarks>
    public static IReadOnlyList<SettingOption> Providers => Offer;

    /// <summary>
    /// Adds a provider, after the ones already added.
    /// </summary>
    /// <remarks>
    /// A provider of an identity already offered is ignored rather than replacing it, so that two
    /// plugins claiming one name cannot have a listing hide by one of them and the panel show the
    /// other. Static because the catalogue is shared: a dependent plugin registers through this class rather
    /// than through a registry of its own, so there is nothing an instance would add.
    /// </remarks>
    /// <param name="provider">The provider to add.</param>
    public static void Register(IEntryHideProvider provider)
    {
        ArgumentNullException.ThrowIfNull(provider);

        if (Catalogue.Any(other => string.Equals(other.Id, provider.Id, StringComparison.Ordinal)))
        {
            return;
        }

        Catalogue.Add(provider);
        Offer.Add(new SettingOption(provider.Id, provider.LabelKey));
    }

    /// <summary>
    /// Removes every provider, which the suite that drives this plugin needs.
    /// </summary>
    /// <remarks>
    /// The catalogue is the run's rather than a registry's, so nothing else can put it back to the state
    /// before any plugin started: a check of what one plugin adds to another's extension point would
    /// otherwise be a check of what every check before it happened to add, in whatever order they ran in.
    /// </remarks>
    public static void Clear()
    {
        Catalogue.Clear();
        Offer.Clear();
    }

    /// <summary>
    /// Declares the setting the providers are chosen with, and the providers this plugin brings itself.
    /// </summary>
    /// <remarks>
    /// Every provider is in force until the user says otherwise, which is what is declared by stating no
    /// default: a multi-choice setting that has never been chosen is worth every option it offers — see
    /// <c>SettingRegistry.Value</c> — so a provider a dependent plugin has just added is in force without
    /// the user having to tick it.
    /// </remarks>
    /// <param name="host">The host, for declaring the setting.</param>
    public static void Declare(IPluginHost host)
    {
        ArgumentNullException.ThrowIfNull(host);

        Register(new DotFileHide());
        Register(new DotDirHide());
        Register(new WindowsHiddenFileHide());

        // The options are the live list, mutable by `Register` until the window is shown, which is when a
        // dependent plugin adds its provider (Section 6.1).
        host.Config.Add(new PluginSetting(Setting, SettingKind.MultiChoice, Key + "providers", Options: Offer));

        // Whether what is hidden is shown is declared beside the choice of what hides it, because it is the
        // same subject and the same reader: whoever stages a listing reads both, and the panel offers both.
        // It is on by default as well, since it is the switch that shows what a rule did rather than one
        // that hides anything itself.
        host.Config.Add(
            new PluginSetting(
                Shared.ShowSetting,
                SettingKind.Bool,
                "rorolala_file_system.hidden",
                "true",
                10
            )
        );
    }

    /// <summary>Whether any provider in force hides an entry.</summary>
    /// <remarks>
    /// Asked of the list rather than of a provider, because the answer is "at least one" and the list is
    /// what says which — an empty list is the answer "nothing hides it", which is not the same as no answer.
    /// <para>
    /// Read against the directory holding the entry, which is where a rule about one entry is stated: a
    /// reader that is looking at a directory asks about what it holds. A view rooted elsewhere — a tree —
    /// asks the other overload.
    /// </para>
    /// </remarks>
    /// <param name="entry">The entry to consider.</param>
    /// <returns>Whether it is hidden.</returns>
    public bool Hides(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return Hides(entry, Holder(entry));
    }

    /// <summary>Whether any provider in force hides an entry, read against a view's root.</summary>
    /// <param name="entry">The entry to consider.</param>
    /// <param name="root">The directory the view reading the entry is rooted at.</param>
    /// <returns>Whether it is hidden.</returns>
    public bool Hides(Entry entry, string root) => Provider(entry, root).Count > 0;

    /// <summary>
    /// The providers in force that hide an entry, in the order they are offered.
    /// </summary>
    /// <param name="entry">The entry to consider.</param>
    /// <returns>The providers that hide it.</returns>
    public IReadOnlyList<IEntryHideProvider> Provider(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return Provider(entry, Holder(entry));
    }

    /// <summary>
    /// The providers in force that hide an entry, read against a view's root.
    /// </summary>
    /// <param name="entry">The entry to consider.</param>
    /// <param name="root">The directory the view reading the entry is rooted at.</param>
    /// <returns>The providers that hide it.</returns>
    public IReadOnlyList<IEntryHideProvider> Provider(Entry entry, string root)
    {
        ArgumentNullException.ThrowIfNull(root);

        var chosen = Selection();

        return Catalogue
            .Where(provider => chosen.Contains(provider.Id) && provider.Hides(entry, root))
            .ToArray();
    }

    /// <summary>The directory a rule about an entry is read from when the caller names none.</summary>
    /// <param name="entry">The entry to consider.</param>
    /// <returns>The directory holding it, or its own path where it holds nothing under one.</returns>
    private static string Holder(Entry entry) =>
        Path.GetDirectoryName(entry.Path) is { Length: > 0 } parent ? parent : entry.Path;

    /// <summary>
    /// Says to every provider that what it answered may have changed.
    /// </summary>
    /// <remarks>
    /// Said to every provider and not only to the ones in force: whether one is in force is the user's to
    /// change at any moment, and a provider put back into force would otherwise answer from what it read
    /// before it was taken out. What a provider is holding is its own business, and one with nothing to let
    /// go of answers nothing to this.
    /// <para>
    /// Static because the catalogue is, and because the answer is the same whatever the settings say: it is
    /// said to the class rather than to a reading of them. The callers inside this class reach it through the
    /// class name, since the registry they hold also has a say in what is asked.
    /// </para>
    /// </remarks>
    public static void Forget()
    {
        foreach (var provider in Catalogue)
        {
            provider.Forget();
        }
    }

    /// <summary>
    /// The providers in force, as the setting says.
    /// </summary>
    /// <remarks>
    /// Read each time rather than kept, because the choice is the user's and is changed while the
    /// program runs: a reader that kept it would hide by what was chosen at startup.
    /// <para>
    /// The value is the list of chosen identities, which is what the setting's own kind stores. A setting
    /// that states nothing means every registered provider is in force, which is the default the catalogue
    /// is offered with: a fresh run hides by each rule it knows, and the switch that shows what is hidden
    /// is what lets them be looked at. A setting that states an empty list means none is, and writes an
    /// empty list rather than nothing, so that unticking the last rule is not the default coming back.
    /// </para>
    /// </remarks>
    /// <returns>The identities of the providers in force.</returns>
    public IReadOnlySet<string> Selection() =>
        (_config.ReadKeyAs<string[]>(Setting) ?? [.. Catalogue.Select(provider => provider.Id)])
            .ToHashSet(StringComparer.Ordinal);
}

/// <summary>Hides a file whose name begins with a dot, which is the Unix convention.</summary>
public sealed class DotFileHide : IEntryHideProvider
{
    /// <inheritdoc />
    public string Id => "dot_file";

    /// <inheritdoc />
    public string LabelKey => "rorolala_file_system.hides.dot_file";

    /// <inheritdoc />
    public bool Hides(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return entry.Kind == EntryKind.File && DottedNames.Dotted(entry.Path);
    }
}

/// <summary>
/// Hides a directory whose name begins with a dot.
/// </summary>
/// <remarks>
/// A directory is a provider of its own rather than the same rule with a different kind, because the
/// two are what a user switches apart: stepping into a dot-directory is rare enough to be worth
/// switching off on its own, and a repository's own metadata is exactly that.
/// </remarks>
public sealed class DotDirHide : IEntryHideProvider
{
    /// <inheritdoc />
    public string Id => "dot_dir";

    /// <inheritdoc />
    public string LabelKey => "rorolala_file_system.hides.dot_dir";

    /// <inheritdoc />
    public bool Hides(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        return entry.Kind == EntryKind.Directory && DottedNames.Dotted(entry.Path);
    }
}

/// <summary>
/// Hides an item Windows marks as hidden.
/// </summary>
/// <remarks>
/// Windows is the only system that marks one this way, so on another system this provider answers no
/// for everything and its switch does nothing. It is offered there all the same: a preference file is
/// the user's and travels, and a list of options that changed with the platform would make one file
/// mean two things.
/// </remarks>
public sealed class WindowsHiddenFileHide : IEntryHideProvider
{
    /// <inheritdoc />
    public string Id => "windows_hidden";

    /// <inheritdoc />
    public string LabelKey => "rorolala_file_system.hides.windows_hidden";

    /// <inheritdoc />
    public bool Hides(Entry entry)
    {
        ArgumentNullException.ThrowIfNull(entry);

        if (!OperatingSystem.IsWindows())
        {
            return false;
        }

        try
        {
            return (File.GetAttributes(entry.Path) & FileAttributes.Hidden) != 0;
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            // An item this process may not inspect is shown rather than hidden: hiding something for not
            // being answerable is the worse mistake.
            return false;
        }
    }
}

/// <summary>
/// What the two dot providers both ask: whether a path's own name begins with a dot.
/// </summary>
/// <remarks>
/// A name that is nothing but dots is not one: <c>.</c> and <c>..</c> are the listing's own steps rather
/// than items it holds, and a name no filesystem allows is not worth treating as a name. What is left —
/// <c>.gitignore</c>, <c>.git</c> — is what a dot-name is.
/// </remarks>
internal static class DottedNames
{
    /// <summary>Whether a path's own name begins with a dot.</summary>
    /// <param name="path">The path to name.</param>
    /// <returns>Whether the name is a dot-name.</returns>
    public static bool Dotted(string path)
    {
        if (Path.GetFileName(path) is not { Length: > 0 } name || name[0] != '.')
        {
            return false;
        }

        return name.Trim('.').Length > 0;
    }
}
