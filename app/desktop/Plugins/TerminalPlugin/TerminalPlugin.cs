using RorolalaDesktop.Contract;
using FileSystem = global::FileSystemPlugin.FileSystemPlugin;

namespace TerminalPlugin;

/// <summary>
/// The Terminal plugin: a dock that runs a shell, as many of them as a user asks for.
/// </summary>
/// <remarks>
/// It depends on the File System plugin for the one thing it starts a shell from: where the tree is rooted.
/// A terminal is not a view of the file manager, though, so it is a plugin of its own rather than a dock of
/// that one — a shell is the shell's, and a user who wants no file manager may still want one.
/// <para>
/// The exchange with the shell is a pseudo-terminal, which is Rust's — see <c>rola-desktop-pty</c> — and what
/// the dock adds is the drawing and the keys. That half is deliberately this plugin's: what a terminal draws
/// is a grid of cells and a parser, and both are the window's answer rather than the exchange's.
/// </para>
/// </remarks>
public sealed class TerminalPlugin : IRolaPlugin
{
    /// <summary>This plugin's identity.</summary>
    public const string Identity = "rorolala.terminal";

    /// <summary>
    /// The stable name of the terminal dock.
    /// </summary>
    /// <remarks>Written into dock layout, so it must not change once shipped.</remarks>
    public const string TerminalDockNameId = "rorolala.terminal";

    /// <inheritdoc />
    public PluginManifest Manifest { get; } =
        new(
            new PluginId(Identity),
            "rorolala_terminal.name",
            typeof(IRolaPlugin).Assembly.GetName().Version ?? new Version(0, 0),
            [new PluginId(FileSystem.Identity)]
        );

    /// <inheritdoc />
    public void Initialize(IPluginHost host)
    {
        ArgumentNullException.ThrowIfNull(host);

        host.I18n.RegisterDirectory(Translations());

        // The program, its arguments and the face it is drawn in are the user's answers rather than this
        // plugin's: a machine may name a shell this program would not guess, and a user may want one started
        // with a flag. They are declared here, so that the preference panel shows them and keeps what is
        // chosen, and read when a dock is made rather than once, so that the next terminal is the next answer.
        TerminalSettings.Declare(host.Config);

        // A factory rather than a toggle: every activation is a shell of its own, which is what a terminal
        // is for. It stands at the bottom because that is where a prompt is looked at, and the side is a
        // preference the user may drag it out of.
        host.Docks.Register(
            new DockRegistration(
                Manifest.Id,
                TerminalDockNameId,
                "rorolala_terminal.name",
                DockOpenMode.New,
                DockSide.Bottom,
                () => new TerminalDock(host.Log)
            )
        );
    }

    /// <summary>The directory this plugin's translations sit in, beside its assembly.</summary>
    /// <returns>The directory.</returns>
    private static string Translations() =>
        Path.Combine(
            Path.GetDirectoryName(typeof(TerminalPlugin).Assembly.Location)!,
            "i18n",
            "rorolala_terminal"
        );
}
