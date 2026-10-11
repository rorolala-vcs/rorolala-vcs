namespace RorolalaDesktop.Theming;

/// <summary>
/// How a window draws its own frame: which buttons it carries, and where they sit.
/// </summary>
/// <remarks>
/// The window is the program's own on every platform — it draws the row the menu sits in, and the row's
/// buttons with it (Section 10) — and the three ways of drawing those buttons are the three a user meets:
/// the marks at the right that Windows puts there, the three lights at the left that macOS puts there, and
/// none at all, which is what a window under a tiling window manager should draw, the window manager
/// outside being the frame already.
/// </remarks>
internal enum WindowStyle
{
    /// <summary>Three buttons at the right: minimize, maximize and close.</summary>
    Windows,

    /// <summary>Three coloured lights at the left, drawn the way macOS draws them.</summary>
    Mac,

    /// <summary>No buttons: the window manager outside is the frame.</summary>
    TillingWm,
}

/// <summary>
/// What a window style is named, and which one the machine this runs on is drawn in.
/// </summary>
/// <remarks>
/// The names are the ones the command line writes: <c>rola desktop --window-style</c> hands the chosen
/// style over as <c>-WindowStyle:</c>, and the machine's own answer stands where nothing was written. A
/// desktop that draws its own frame is asked for by the desktop the program is run on, and a tiling window
/// manager is recognized by what it leaves in the environment rather than by being named anywhere.
/// </remarks>
internal static class WindowStyles
{
    /// <summary>
    /// The style this machine's desktop is drawn in.
    /// </summary>
    /// <remarks>
    /// Windows and macOS answer for themselves: each has one way of drawing a frame, and the window draws
    /// that way. Everything else is a Unix whose window manager may or may not draw a frame of its own — a
    /// tiling one does not, and a window there draws none either; anything else gets the three marks, which
    /// is the shape this program has always been drawn with.
    /// </remarks>
    /// <returns>The style to draw in when nothing named one.</returns>
    public static WindowStyle Here() =>
        OperatingSystem.IsWindows() ? WindowStyle.Windows
        : OperatingSystem.IsMacOS() ? WindowStyle.Mac
        : Tiling(Environment.GetEnvironmentVariable) ? WindowStyle.TillingWm
        : WindowStyle.Windows;

    /// <summary>
    /// The style a written name is read as, or nothing when it names none of them.
    /// </summary>
    /// <remarks>
    /// Read without regard to case, because the two spellings one name has — the command line's
    /// <c>tillingwm</c> and this program's own <c>TillingWM</c> — are one name to whoever wrote it.
    /// </remarks>
    /// <param name="name">What was written.</param>
    /// <returns>The style, or nothing when the name is not one of the three.</returns>
    public static WindowStyle? Named(string name)
    {
        var written = name.Trim();

        if (string.Equals(written, "windows", StringComparison.OrdinalIgnoreCase))
        {
            return WindowStyle.Windows;
        }

        if (string.Equals(written, "mac", StringComparison.OrdinalIgnoreCase))
        {
            return WindowStyle.Mac;
        }

        if (string.Equals(written, "tillingwm", StringComparison.OrdinalIgnoreCase))
        {
            return WindowStyle.TillingWm;
        }

        return null;
    }

    /// <summary>
    /// Whether an environment is a tiling window manager's.
    /// </summary>
    /// <remarks>
    /// Recognized by what the four this program knows about leave behind rather than by asking the session,
    /// which is a conversation a program with no window yet has no way to have: three of them publish the
    /// socket they may be spoken to on as an environment variable of their own, and all four are named by
    /// the session when it names itself. It is a simple read on purpose — a window manager that is none of
    /// these is given the three marks, which is a frame drawn where one was already there rather than a
    /// window with nothing left to close it by.
    /// </remarks>
    /// <param name="value">Reads an environment variable, answering nothing for one that is not set.</param>
    /// <returns>Whether the environment is a tiling one.</returns>
    public static bool Tiling(Func<string, string?> value)
    {
        foreach (var socket in Sockets)
        {
            if (!string.IsNullOrEmpty(value(socket)))
            {
                return true;
            }
        }

        foreach (var session in Sessions)
        {
            var named = value(session);

            if (named is null)
            {
                continue;
            }

            foreach (var manager in Managers)
            {
                if (named.Contains(manager, StringComparison.OrdinalIgnoreCase))
                {
                    return true;
                }
            }
        }

        return false;
    }

    /// <summary>The variables the tiling window managers publish their own socket under.</summary>
    private static readonly string[] Sockets =
    [
        "SWAYSOCK",
        "I3SOCK",
        "HYPRLAND_INSTANCE_SIGNATURE",
        "NIRI_SOCKET",
    ];

    /// <summary>The variables a session names the desktop it runs in.</summary>
    private static readonly string[] Sessions =
    [
        "XDG_CURRENT_DESKTOP",
        "XDG_SESSION_DESKTOP",
        "DESKTOP_SESSION",
    ];

    /// <summary>What those window managers are called, as a session spells them.</summary>
    private static readonly string[] Managers = ["sway", "i3", "hyprland", "niri"];
}
