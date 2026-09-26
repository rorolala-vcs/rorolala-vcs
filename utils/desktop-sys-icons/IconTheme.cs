using System.Diagnostics;
using System.Diagnostics.CodeAnalysis;
using System.Globalization;
using Avalonia.Media.Imaging;

namespace RorolalaDesktop.SysIcons;

/// <summary>
/// The icon a freedesktop desktop gives a file or a directory.
/// </summary>
/// <remarks>
/// There is no call that answers this: a desktop names an icon and its theme says where the picture
/// is, so what is read here is the theme the desktop says it is using, then whatever that theme says
/// it inherits from, then hicolor, which every theme has to fall back to.
/// <para>
/// What a theme keeps for a folder is usually an SVG — Papirus, Adwaita and Breeze all do — and
/// nothing here can draw one, so the system's own renderer is asked to. librsvg is what the desktop
/// draws these with itself, so what comes back is the picture the desktop would have shown rather
/// than an approximation of it. It is asked for once per icon, and the answer is kept.
/// </para>
/// </remarks>
internal static class IconTheme
{
    /// <summary>The names a directory may be drawn under, most specific first.</summary>
    private static readonly string[] Directories = ["inode-directory", "folder"];

    /// <summary>The names a plain file may be drawn under, most specific first.</summary>
    private static readonly string[] Files = ["text-plain", "text-x-generic", "application-x-generic"];

    /// <summary>The contexts an icon may be kept in, the likeliest first.</summary>
    private static readonly string[] Contexts = ["places", "mimetypes"];

    /// <summary>The sizes a theme may keep an icon at.</summary>
    private static readonly int[] Sizes = [16, 22, 24, 32, 48, 64, 128];

    /// <summary>Where themes are installed, most specific first, as the specification lays them out.</summary>
    private static readonly string[] Roots =
        [
            Path.Combine(Data(), "icons"),
            Path.Combine(Home(), ".icons"),
            .. DataDirs().Select(dir => Path.Combine(dir, "icons")),
            "/usr/share/pixmaps",
        ];

    /// <summary>The renderers to ask for an SVG, in the order it is worth asking them.</summary>
    private static readonly string[] Renderers =
        ["/usr/bin/rsvg-convert", "/usr/local/bin/rsvg-convert"];

    /// <summary>The icon for one kind of thing, at a size, or nothing when the desktop has none.</summary>
    /// <param name="kind">The kind of thing the icon is for.</param>
    /// <param name="size">How many pixels wide the icon is wanted.</param>
    public static Bitmap? Drawn(Kind kind, int size)
    {
        var names = kind == Kind.Directory ? Directories : Files;

        foreach (var theme in Themes())
        {
            foreach (var name in names)
            {
                if (Picture(theme, name, size) is { } picture)
                {
                    return picture;
                }
            }
        }

        return null;
    }

    /// <summary>The themes to look in: the one in use, what it inherits, and hicolor last.</summary>
    private static IReadOnlyList<string> Themes()
    {
        var chain = new List<string>();

        Adding(Named() ?? "hicolor");

        if (!chain.Contains("hicolor", StringComparer.Ordinal))
        {
            chain.Add("hicolor");
        }

        return chain;

        void Adding(string? name)
        {
            if (name is null || chain.Contains(name, StringComparer.Ordinal))
            {
                return;
            }

            chain.Add(name);

            foreach (var inherited in Inherits(name))
            {
                Adding(inherited);
            }
        }
    }

    /// <summary>
    /// The theme the desktop says it is using, or nothing when it does not say.
    /// </summary>
    /// <remarks>
    /// Read out of the two files desktops write it in — GTK's and KDE's — and then asked of GSettings,
    /// which is where a GNOME desktop keeps its answer and keeps it nowhere a file can be read.
    /// </remarks>
    private static string? Named()
    {
        var settings = Path.Combine(Home(), ".config/gtk-3.0/settings.ini");

        if (Setting(settings, "Settings", "gtk-icon-theme-name") is { } gtk)
        {
            return gtk;
        }

        if (Setting(Path.Combine(Home(), ".config/kdeglobals"), "Icons", "Theme") is { } kde)
        {
            return kde;
        }

        // What GSettings says is quoted, and says nothing at all when it is not there to ask.
        return Asked("gsettings", ["get", "org.gnome.desktop.interface", "icon-theme"])?.Trim('\'', '"');
    }

    /// <summary>What a theme says it inherits from, in the order it says it.</summary>
    private static IEnumerable<string> Inherits(string theme)
    {
        foreach (var root in Roots)
        {
            var said = Setting(Path.Combine(root, theme, "index.theme"), "Icon Theme", "Inherits");

            foreach (var name in said?.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries) ?? [])
            {
                yield return name;
            }
        }
    }

    /// <summary>One theme's picture for a name, or nothing when it has none.</summary>
    /// <remarks>
    /// What is read here is what the method answers with, and the caller is what keeps it: disposing it here
    /// would hand back a disposed picture. The rule cannot see that the value leaves by the return, so it is
    /// excused beside the method rather than for the assembly.
    /// </remarks>
    [SuppressMessage(
        "Reliability",
        "CA2000:Dispose objects before losing scope",
        Justification = "the picture read here is the method's answer, and the caller keeps it"
    )]
    private static Bitmap? Picture(string theme, string name, int size)
    {
        foreach (var root in Roots)
        {
            var dir = Path.Combine(root, theme);

            if (!System.IO.Directory.Exists(dir))
            {
                continue;
            }

            foreach (var picture in Pictures(dir, name, size))
            {
                if (Read(picture, size) is { } kept)
                {
                    return kept;
                }
            }

            foreach (var svg in Svgs(dir, name))
            {
                if (Read(svg, size) is { } rendered)
                {
                    return rendered;
                }
            }
        }

        return null;
    }

    /// <summary>
    /// The pictures a theme may keep for a name: PNGs, which are drawn as they are, and SVGs, which
    /// are not.
    /// </summary>
    /// <remarks>
    /// Both are looked for in the size directories, because that is where themes keep them — Papirus
    /// has no scalable directory at all and keeps an SVG inside each of its size directories — and the
    /// size wanted is taken before the sizes further off, since a picture of the size wanted need not
    /// be scaled at all.
    /// </remarks>
    private static IEnumerable<string> Pictures(string theme, string name, int wanted)
    {
        foreach (var context in Contexts)
        {
            foreach (var size in Sizes.OrderBy(at => Math.Abs(at - wanted)).ThenBy(at => at))
            {
                yield return Path.Combine(theme, $"{size}x{size}", context, $"{name}.svg");
                yield return Path.Combine(theme, $"{size}x{size}", context, $"{name}.png");
                yield return Path.Combine(theme, $"{size}x{size}@2x", context, $"{name}.png");
            }
        }
    }

    /// <summary>The SVGs a theme may keep for a name outside its size directories.</summary>
    /// <remarks>
    /// Drawn at the size wanted, since that is the size the picture is going to be shown at.
    /// </remarks>
    private static IEnumerable<string> Svgs(string theme, string name)
    {
        foreach (var context in Contexts)
        {
            yield return Path.Combine(theme, "scalable", context, $"{name}.svg");
        }

        yield return Path.Combine(theme, "scalable", $"{name}.svg");
    }

    /// <summary>
    /// Reads a picture, drawing an SVG by asking the system to where one has to be drawn.
    /// </summary>
    /// <remarks>
    /// A path that is not there is not a picture: every candidate is asked for by name, most of which
    /// a theme does not have, and a renderer asked about one that is not there answers with an error
    /// and nothing else.
    /// </remarks>
    private static Bitmap? Read(string file, int size)
    {
        if (!File.Exists(file))
        {
            return null;
        }

        return file.EndsWith(".svg", StringComparison.OrdinalIgnoreCase)
            ? Rendered(file, size)
            : Loaded(file);
    }

    /// <summary>
    /// Draws an SVG by asking the system to, since nothing here can.
    /// </summary>
    /// <remarks>
    /// The render is written among the system's temporary files and taken away again. What is handed
    /// to the renderer is a path from an installed theme rather than anything a user typed, and it is
    /// given as an argument rather than through a shell.
    /// </remarks>
    private static Bitmap? Rendered(string svg, int size)
    {
        var renderer = Array.Find(Renderers, File.Exists);

        if (renderer is null)
        {
            return null;
        }

        var png = Path.Combine(Path.GetTempPath(), $"rorolala-icon-{Guid.NewGuid():N}.png");

        try
        {
            using var process = Process.Start(
                new ProcessStartInfo(renderer)
                {
                    ArgumentList =
                    {
                        "--width",
                        size.ToString(CultureInfo.InvariantCulture),
                        "--height",
                        size.ToString(CultureInfo.InvariantCulture),
                        "--output",
                        png,
                        svg,
                    },
                }
            );

            process?.WaitForExit(5000);

            return Loaded(png);
        }
        catch (Exception error) when (error is IOException or InvalidOperationException or System.ComponentModel.Win32Exception)
        {
            return null;
        }
        finally
        {
            try
            {
                File.Delete(png);
            }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException)
            {
                // A render left behind is a file in a temporary directory, which is where things that
                // could not be taken away are meant to be left.
            }
        }
    }

    /// <summary>Reads a picture off the disk, or nothing when it will not read.</summary>
    private static Bitmap? Loaded(string file)
    {
        if (!File.Exists(file))
        {
            return null;
        }

        try
        {
            using var stream = File.OpenRead(file);

            return new Bitmap(stream);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            // A theme file that will not decode is a theme with one icon fewer, which is not a reason
            // to stop drawing a listing.
            return null;
        }
    }

    /// <summary>
    /// The value of one key under one section of a file shaped like an INI, or nothing.
    /// </summary>
    /// <remarks>
    /// Read line by line rather than parsed into a dictionary: three files are read once each and one
    /// key is wanted from each, so there is nothing to keep.
    /// </remarks>
    private static string? Setting(string file, string section, string key)
    {
        if (!File.Exists(file))
        {
            return null;
        }

        var wanted = $"[{section}]";
        var within = false;

        try
        {
            foreach (var raw in File.ReadLines(file))
            {
                var line = raw.Trim();

                if (line.StartsWith('['))
                {
                    within = line.Equals(wanted, StringComparison.OrdinalIgnoreCase);

                    continue;
                }

                var equals = line.IndexOf('=', StringComparison.Ordinal);

                if (!within || equals < 0)
                {
                    continue;
                }

                if (line.AsSpan(0, equals).Trim().Equals(key, StringComparison.OrdinalIgnoreCase))
                {
                    return line[(equals + 1)..].Trim().Trim('"', '\'');
                }
            }
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return null;
        }

        return null;
    }

    /// <summary>
    /// What a program says when asked, or nothing when it is not there or says nothing.
    /// </summary>
    /// <remarks>
    /// A program is asked rather than a file read where the answer is kept somewhere no file holds it
    /// — GSettings, which is not a text file — and where the program that computes it is the desktop's
    /// own. It is asked once, since what it answers with is cached.
    /// </remarks>
    private static string? Asked(string program, string[] arguments)
    {
        try
        {
            var start = new ProcessStartInfo(program) { RedirectStandardOutput = true };

            foreach (var argument in arguments)
            {
                start.ArgumentList.Add(argument);
            }

            using var process = Process.Start(start);

            if (process is null)
            {
                return null;
            }

            var said = process.StandardOutput.ReadToEnd().Trim();
            process.WaitForExit(5000);

            return said.Length == 0 ? null : said;
        }
        catch (Exception error) when (error is IOException or InvalidOperationException or System.ComponentModel.Win32Exception)
        {
            return null;
        }
    }

    /// <summary>The user's own directory, which is where a desktop writes what it has been told.</summary>
    private static string Home() => Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);

    /// <summary>The directory user data is kept in, as the specification says where it is.</summary>
    private static string Data() =>
        Environment.GetEnvironmentVariable("XDG_DATA_HOME") is { Length: > 0 } set
            ? set
            : Path.Combine(Home(), ".local/share");

    /// <summary>The directories system data is kept in, most specific first.</summary>
    private static IEnumerable<string> DataDirs() =>
        Environment.GetEnvironmentVariable("XDG_DATA_DIRS") is { Length: > 0 } set
            ? set.Split(':', StringSplitOptions.RemoveEmptyEntries)
            : new[] { "/usr/local/share", "/usr/share" };
}
