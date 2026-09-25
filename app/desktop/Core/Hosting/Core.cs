using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// The kernel: what is always present and cannot be disabled.
/// </summary>
/// <remarks>
/// The plugin manager and the log are kernel because the program must remain diagnosable and
/// recoverable even when a plugin is broken. Everything else that can be a plugin is one.
/// </remarks>
internal static class Core
{
    /// <summary>The identity the kernel registers its own docks and menus under.</summary>
    public static readonly PluginId Id = new("rorolala.core");

    /// <summary>The name the kernel's own log lines carry.</summary>
    public const string Source = "kernel";

    /// <summary>The kernel's place in the ordering: before every plugin.</summary>
    public const int Position = -1;
}
