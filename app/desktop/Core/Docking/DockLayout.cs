using System.Text.Json;
using System.Text.Json.Serialization;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Logging;

namespace RorolalaDesktop.Docking;

/// <summary>One dock as the layout records it: which dock, which instance, and where.</summary>
internal sealed class LayoutDock
{
    /// <summary>The dock's stable name id.</summary>
    [JsonPropertyName("nameId")]
    public string DockNameId { get; set; } = "";

    /// <summary>Which instance of that name this is.</summary>
    [JsonPropertyName("ordinal")]
    public int Ordinal { get; set; }

    /// <summary>Where the dock sits.</summary>
    [JsonPropertyName("placement")]
    [JsonConverter(typeof(JsonStringEnumConverter))]
    public DockPlacement Placement { get; set; }

    /// <summary>
    /// What the dock remembered about itself, by key.
    /// </summary>
    /// <remarks>
    /// Kept beside the placement rather than in the plugin's own preferences, because it is one dock's
    /// state and not the plugin's: two instances of one dock are two sets of it, and a dock that is
    /// closed takes its own away with it.
    /// </remarks>
    [JsonPropertyName("meta")]
    public Dictionary<string, string> Meta { get; set; } = [];
}

/// <summary>
/// The dock layout, as it survives a restart.
/// </summary>
/// <remarks>
/// The record is the stable name id plus an instance ordinal rather than any runtime handle, so it
/// keeps its meaning across versions of the program and versions of the plugins. The region sizes
/// are the sizes to restore, and are updated as the user drags the splitters; what a dock kept about
/// itself is written down with it, so that a dock comes back as it was and not only where it was.
/// </remarks>
internal sealed class DockLayout
{
    /// <summary>The schema version this program reads and writes.</summary>
    public const int SchemaVersion = 1;

    /// <summary>The width the left region starts at, in pixels.</summary>
    /// <remarks>
    /// Whole pixels, because these are where a person put the splitters rather than what a layout engine worked
    /// out: a region asked for at 258.4 is a region the engine rounds again, and a file read by hand should not
    /// disagree with the file written beside it.
    /// </remarks>
    public const double DefaultLeftWidth = 258;

    /// <summary>The width the right region starts at, in pixels.</summary>
    public const double DefaultRightWidth = 219;

    /// <summary>The height the bottom region starts at, in pixels.</summary>
    public const double DefaultBottomHeight = 253;

    /// <summary>The height the top region starts at, in pixels.</summary>
    public const double DefaultTopHeight = 72;

    /// <summary>The schema version written.</summary>
    [JsonPropertyName("_version")]
    public int Version { get; set; } = SchemaVersion;

    /// <summary>The width of the left region, in pixels.</summary>
    [JsonPropertyName("leftWidth")]
    public double LeftWidth { get; set; } = DefaultLeftWidth;

    /// <summary>The width of the right region, in pixels.</summary>
    [JsonPropertyName("rightWidth")]
    public double RightWidth { get; set; } = DefaultRightWidth;

    /// <summary>The height of the bottom region, in pixels.</summary>
    [JsonPropertyName("bottomHeight")]
    public double BottomHeight { get; set; } = DefaultBottomHeight;

    /// <summary>The height of the top region, in pixels.</summary>
    [JsonPropertyName("topHeight")]
    public double TopHeight { get; set; } = DefaultTopHeight;

    /// <summary>The docks that were open, in the order they are restored in.</summary>
    [JsonPropertyName("docks")]
    public List<LayoutDock> Docks { get; set; } = [];

    /// <summary>
    /// The arrangement a user who has never moved a splitter, opened a dock, or closed one is given.
    /// </summary>
    /// <remarks>
    /// What is being worked on in the middle, its tree down the right, and the program's own two docks beside
    /// them. It is a starting point rather than a decision: every one of them can be closed, moved, or opened
    /// again, and the layout is written with what there is from then on.
    /// <para>
    /// A layout of its own rather than what an empty one happens to hold, because a layout is built for what is
    /// open as well as for what is started — one built empty and filled from the docks that are open would
    /// otherwise carry this arrangement on top of them, and a dock that was closed would be written down again.
    /// The instances are numbered from nothing, since a dock has one instance until somebody opens a second.
    /// </para>
    /// </remarks>
    /// <returns>The layout to start from, with the region sizes at their defaults.</returns>
    public static DockLayout Starting() =>
        new()
        {
            Docks =
            [
                new() { DockNameId = "rorolala.core.plugin_manager", Placement = DockPlacement.Center },
                new() { DockNameId = "rorolala.core.preference", Placement = DockPlacement.Center },
                new() { DockNameId = "rorolala.file_system.browser", Placement = DockPlacement.Center },
                new() { DockNameId = "rorolala.file_system.tree", Placement = DockPlacement.Right },
            ],
        };
}

/// <summary>
/// Reads and writes the dock layout.
/// </summary>
/// <remarks>
/// The layout is a convenience rather than a contract: a file that is not there, or will not read,
/// is started over and the user is told, but nothing stops. Losing a layout is not losing work, and
/// a stop over it would be worse than a fresh arrangement.
/// </remarks>
internal static class LayoutStore
{
    /// <summary>
    /// How the layout is written.
    /// </summary>
    /// <remarks>
    /// Indented, because a person reads this file while working something out about the docks, and one
    /// instance rather than one per write, because the serializer caches what it works out about a type
    /// inside the options it was handed: a fresh one each time throws that work away every time.
    /// </remarks>
    private static readonly JsonSerializerOptions Writing = new() { WriteIndented = true };

    /// <summary>Reads the layout, answering a fresh one when there is none to read.</summary>
    /// <param name="log">Where a file that would not read is reported.</param>
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
            return JsonSerializer.Deserialize<DockLayout>(File.ReadAllText(path))
                ?? DockLayout.Starting();
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
}
