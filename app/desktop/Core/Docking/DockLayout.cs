using System.Text.Json;
using System.Text.Json.Serialization;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Logging;

namespace RorolalaDesktop.Docking;

/// <summary>
/// Which way a split divides its two children.
/// </summary>
/// <remarks>
/// The two children of a split are ordered, and the axis is what says in which of the two directions the
/// order runs: columns put the first on the left, rows put it on top. There is no third axis, because a
/// split of a rectangle is one of exactly these two.
/// </remarks>
internal enum DockAxis
{
    /// <summary>First on the left, second on the right.</summary>
    Columns,

    /// <summary>First on top, second below.</summary>
    Rows,
}

/// <summary>
/// One place in the layout tree: a group of tabs, or a split of two children.
/// </summary>
/// <remarks>
/// The tree is what replaced the five fixed regions, and it is recursive without a depth limit: a leaf is a
/// group of tabs, and any group may be split again. It is also the persisted shape — the layout file is this
/// tree written down with a ratio for every split — so the types here are the schema the file is read and
/// written through, with <c>kind</c> as the discriminator a hand-edited file states.
/// <para>
/// Geometry is ratios only. A split stores the share of its first child in <c>(0, 1)</c>; nothing stores
/// pixels, so a layout follows the window rather than keeping a band at one width.
/// </para>
/// </remarks>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "kind")]
[JsonDerivedType(typeof(LayoutGroup), "group")]
[JsonDerivedType(typeof(LayoutSplit), "split")]
internal abstract class LayoutNode;

/// <summary>One split of the area into two, either side by side or one above the other.</summary>
internal sealed class LayoutSplit : LayoutNode
{
    /// <summary>The direction the two children are laid out in.</summary>
    [JsonPropertyName("axis")]
    public required DockAxis Axis { get; init; }

    /// <summary>The share of the split's room the first child takes, in <c>(0, 1)</c>.</summary>
    /// <remarks>
    /// The second child takes what is left. It is the share rather than a pixel width because the two are
    /// not the same fact: a width is one window's, and a share is every window's.
    /// </remarks>
    [JsonPropertyName("ratio")]
    public required double Ratio { get; set; }

    /// <summary>The child on the left or on top.</summary>
    [JsonPropertyName("first")]
    public required LayoutNode First { get; set; }

    /// <summary>The child on the right or below.</summary>
    [JsonPropertyName("second")]
    public required LayoutNode Second { get; set; }
}

/// <summary>One group of tabs: the docks that share a place, in the order their tabs are drawn.</summary>
internal sealed class LayoutGroup : LayoutNode
{
    /// <summary>The group's stable id, of the form <c>g-&lt;number&gt;</c>.</summary>
    /// <remarks>
    /// What the rest of the host names a group by — the active group, and where a dock brought back from a
    /// window of its own goes. Assigned by the host and never reused within a run, because a dock in a
    /// window of its own remembers the group it came from.
    /// </remarks>
    [JsonPropertyName("id")]
    public required string Id { get; set; }

    /// <summary>Which of the group's docks is the tab being shown, as an index into <see cref="Docks"/>.</summary>
    [JsonPropertyName("active")]
    public int Active { get; set; }

    /// <summary>The group's docks, in the order their tabs are drawn.</summary>
    [JsonPropertyName("docks")]
    public List<LayoutDock> Docks { get; set; } = [];
}

/// <summary>One dock as the layout records it: which dock, and which instance of it.</summary>
internal sealed class LayoutDock
{
    /// <summary>The dock's stable name id.</summary>
    [JsonPropertyName("nameId")]
    public string DockNameId { get; set; } = "";

    /// <summary>Which instance of that name this is.</summary>
    [JsonPropertyName("ordinal")]
    public int Ordinal { get; set; }

    /// <summary>
    /// What the dock remembered about itself, by key.
    /// </summary>
    /// <remarks>
    /// Kept beside the dock rather than in the plugin's own preferences, because it is one dock's state and
    /// not the plugin's: two instances of one dock are two sets of it, and a dock that is closed takes its
    /// own away with it.
    /// </remarks>
    [JsonPropertyName("meta")]
    public Dictionary<string, string> Meta { get; set; } = [];
}

/// <summary>
/// Writes a split's share as a short fraction rather than as whatever a division left behind.
/// </summary>
/// <remarks>
/// A ratio is read from two measured pixels and is a division, so it carries the whole tail of one: the file
/// would churn on every drag of the same splitter, and a person reading it would meet a number that says
/// nothing. Six places is finer than a splitter can be placed in any window this program draws in. It is a
/// converter on the options rather than an attribute on the property so that the type is one this assembly
/// makes rather than one the serializer has to reach for by name.
/// </remarks>
internal sealed class RatioConverter : JsonConverter<double>
{
    /// <inheritdoc />
    public override double Read(
        ref Utf8JsonReader reader,
        Type typeToConvert,
        JsonSerializerOptions options
    ) => reader.GetDouble();

    /// <inheritdoc />
    public override void Write(
        Utf8JsonWriter writer,
        double value,
        JsonSerializerOptions options
    ) => writer.WriteNumberValue(Math.Round(value, 6));
}

/// <summary>
/// The dock layout of the main window, as it survives a restart.
/// </summary>
/// <remarks>
/// A tree of groups and splits, and the group that was last worked in. The record is the stable name id plus
/// an instance ordinal rather than any runtime handle, so it keeps its meaning across versions of the program
/// and versions of the plugins. What a dock kept about itself is written down with it, so that a dock comes
/// back as it was and not only where it was.
/// <para>
/// A dock in a window of its own is not written down: the layout states what the main window's tree is, and
/// which docks are in windows of their own is this run's arrangement rather than what a run starts with.
/// </para>
/// </remarks>
internal sealed class DockLayout
{
    /// <summary>The schema version this program reads and writes.</summary>
    public const int SchemaVersion = 2;

    /// <summary>
    /// The share of the area the starting layout gives to the group that holds the browser.
    /// </summary>
    /// <remarks>
    /// The tree the five fixed regions used to be, as one ratio: the program opens at 1100 pixels wide, and
    /// the band down the right the tree was given was 219 of them, so the rest is this share. It is a
    /// starting point rather than a decision — every splitter may be dragged from there — and a ratio is
    /// written rather than a width because a width is one window's.
    /// </remarks>
    public const double StartingRatio = 0.8009;

    /// <summary>The schema version written.</summary>
    [JsonPropertyName("_version")]
    public int Version { get; set; } = SchemaVersion;

    /// <summary>The group last worked in, or nothing when the main window holds no docks.</summary>
    /// <remarks>
    /// What a dock opened or dropped without saying where goes into, and what a dock brought back without a
    /// group of its own lands in. It is remembered because a layout is arranged over more than one run and
    /// the place a person was working is part of the arrangement.
    /// </remarks>
    [JsonPropertyName("activeGroup")]
    public string? ActiveGroup { get; set; }

    /// <summary>The whole arrangement, or nothing when the main window holds no docks.</summary>
    [JsonPropertyName("root")]
    public LayoutNode? Root { get; set; }

    /// <summary>
    /// One past the largest group number this run has given out.
    /// </summary>
    /// <remarks>
    /// Not written down: what a loaded tree says is the most a file can say, and this is the rest. It is
    /// what keeps a group id from being handed out twice within one run: the largest id may belong to a
    /// group that was just pruned, and a dock in a window of its own may still remember that id as the group
    /// it came from — a new group given the same id would collect that dock on its return.
    /// </remarks>
    [JsonIgnore]
    public int GroupIdHighWater { get; set; }

    /// <summary>
    /// The arrangement a user who has never opened a dock or dragged a splitter is given.
    /// </summary>
    /// <remarks>
    /// What is being worked on in the middle, and its tree down the right: the arrangement the five fixed
    /// regions used to give, expressed as a split of two groups. The tab order is the order the old layout
    /// restored these docks in, so a first run looks as it did rather than merely holding the same docks. It
    /// is a starting point rather than a decision — every one of them can be closed, moved, or opened again,
    /// and the layout is written with what there is from then on.
    /// </remarks>
    /// <returns>The layout to start from.</returns>
    public static DockLayout Starting() =>
        new()
        {
            ActiveGroup = "g-1",
            Root = new LayoutSplit
            {
                Axis = DockAxis.Columns,
                Ratio = StartingRatio,
                First = StartingGroup(
                    "g-1",
                    "rorolala.core.plugin_manager",
                    "rorolala.core.preference",
                    "rorolala.file_system.browser"
                ),
                Second = StartingGroup("g-2", "rorolala.file_system.tree"),
            },
        };

    /// <summary>A group of the starting layout, holding docks that exist on a first run.</summary>
    /// <param name="id">The group's id.</param>
    /// <param name="dockNameIds">The docks it holds, in tab order.</param>
    /// <returns>The group, showing its first dock.</returns>
    private static LayoutGroup StartingGroup(string id, params string[] dockNameIds)
    {
        var group = new LayoutGroup { Id = id };

        foreach (var nameId in dockNameIds)
        {
            group.Docks.Add(new LayoutDock { DockNameId = nameId });
        }

        return group;
    }
}

/// <summary>
/// Reads and writes the dock layout.
/// </summary>
/// <remarks>
/// The layout is a convenience rather than a contract: a file that is not there, or will not read, is started
/// over and the user is told, but nothing stops. Losing a layout is not losing work, and a stop over it would
/// be worse than a fresh arrangement.
/// </remarks>
internal static class LayoutStore
{
    /// <summary>
    /// How the layout is written.
    /// </summary>
    /// <remarks>
    /// Indented, because a person reads this file while working something out about the docks, and one
    /// instance rather than one per write, because the serializer caches what it works out about a type inside
    /// the options it was handed: a fresh one each time throws that work away every time.
    /// </remarks>
    private static readonly JsonSerializerOptions Writing = new()
    {
        WriteIndented = true,
        Converters =
        {
            new JsonStringEnumConverter(JsonNamingPolicy.CamelCase),
            new RatioConverter(),
        },
    };

    /// <summary>How the layout is read, which is the same shape it is written in.</summary>
    private static readonly JsonSerializerOptions Reading = new()
    {
        Converters =
        {
            new JsonStringEnumConverter(JsonNamingPolicy.CamelCase),
            new RatioConverter(),
        },
    };

    /// <summary>Reads the layout, answering a fresh one when there is none to read.</summary>
    /// <remarks>
    /// The version is read before the layout is: a file written by an earlier build states one, and what it
    /// states decides whether the rest of it is worth reading at all. This program reads version two and
    /// nothing else — a version one file records a placement and four pixel widths, which cannot be turned
    /// into a tree without inventing a window size — so an old file is left where it is, reported once, and
    /// replaced by the next save.
    /// </remarks>
    /// <param name="log">Where a file that would not read, or is of another version, is reported.</param>
    /// <returns>What was read, or a fresh layout.</returns>
    public static DockLayout Load(LogService log)
    {
        var path = ConfigPaths.Layout;

        if (!File.Exists(path))
        {
            return DockLayout.Starting();
        }

        try
        {
            var text = File.ReadAllText(path);
            var version = VersionOf(text);

            if (version != DockLayout.SchemaVersion)
            {
                log.Record(
                    LogLevel.Warn,
                    Core.Source,
                    version is { } stated
                        ? $"the dock layout is version {stated}, which this program does not read, so the starting arrangement is used"
                        : "the dock layout states no version, so the starting arrangement is used"
                );

                return DockLayout.Starting();
            }

            var layout = JsonSerializer.Deserialize<DockLayout>(text, Reading);

            if (layout is null)
            {
                return DockLayout.Starting();
            }

            // A hand-edited file is normalized rather than trusted: it may hold an empty group, a dock
            // twice, an id nothing else agrees with, or a ratio of exactly one. What can be repaired is;
            // what cannot threw above and started over.
            DockTree.Normalize(layout);

            return layout;
        }
        catch (Exception error)
            when (error is IOException or JsonException or UnauthorizedAccessException)
        {
            log.Record(
                LogLevel.Warn,
                "kernel",
                $"the dock layout could not be read and was started over: {error.Message}"
            );

            return DockLayout.Starting();
        }
    }

    /// <summary>Writes the layout.</summary>
    /// <param name="log">Where a file that could not be written is reported.</param>
    /// <param name="layout">What to write.</param>
    public static void Save(LogService log, DockLayout layout)
    {
        try
        {
            System.IO.Directory.CreateDirectory(ConfigPaths.Root);
            File.WriteAllText(
                ConfigPaths.Layout,
                JsonSerializer.Serialize(layout, Writing)
            );
        }
        catch (Exception error)
            when (error is IOException or UnauthorizedAccessException)
        {
            log.Record(
                LogLevel.Warn,
                "kernel",
                $"the dock layout could not be written: {error.Message}"
            );
        }
    }

    /// <summary>
    /// The version a file states, or nothing when it states none or will not be read as JSON.
    /// </summary>
    /// <remarks>
    /// Read from the document rather than the deserialized layout, because the whole question is whether that
    /// layout is the shape this program knows: deserializing an older one into the current types would fail
    /// rather than say which version it was, and the warning is worth more than the failure.
    /// </remarks>
    /// <param name="text">What the file holds.</param>
    /// <returns>The version, or nothing.</returns>
    private static int? VersionOf(string text)
    {
        using var document = JsonDocument.Parse(text);

        return document.RootElement.ValueKind == JsonValueKind.Object
            && document.RootElement.TryGetProperty("_version", out var version)
            && version.TryGetInt32(out var stated)
                ? stated
                : null;
    }
}
