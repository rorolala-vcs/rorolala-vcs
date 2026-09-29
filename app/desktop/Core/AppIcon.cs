using Avalonia.Controls;
using Avalonia.Platform;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Logging;

namespace RorolalaDesktop;

/// <summary>
/// The picture the program is known by, as the windows it makes are drawn with it.
/// </summary>
/// <remarks>
/// The host reads it once and carries it, so no window costs a decode: the host's own windows take it
/// from there, and a plugin's do too, through the contract. The file is embedded in this assembly by
/// the project file — the path below is the one that file gives it — rather than read from beside the
/// program, because what a program looks like is part of what it is: there is no copy of it to go
/// missing.
/// </remarks>
internal static class AppIcon
{
    /// <summary>Where the project file puts the icon inside this assembly.</summary>
    private const string Resource = "avares://RorolalaDesktop/res/icons/yizi.ico";

    /// <summary>
    /// Reads the embedded icon, or answers nothing when it cannot be read.
    /// </summary>
    /// <remarks>
    /// The file is put into this assembly while the program is built, so a failure here is a mistake
    /// in the build rather than something a run meets. It still does not end the run: an icon is what
    /// a window is drawn with and not what it does, so a window without one is better than no window,
    /// and the log says which it was. Out of memory is left to travel, as everywhere else here.
    /// </remarks>
    /// <param name="log">Where a failure is recorded.</param>
    /// <returns>The icon, or nothing when it could not be read.</returns>
    public static WindowIcon? Load(LogService log)
    {
        try
        {
            using var stream = AssetLoader.Open(new Uri(Resource, UriKind.Absolute));
            return new WindowIcon(stream);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            log.Record(LogLevel.Warn, Core.Source, $"the window icon could not be read: {error.Message}");
            return null;
        }
    }
}
