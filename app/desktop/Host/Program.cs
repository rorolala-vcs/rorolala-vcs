using System.Collections.Concurrent;
using System.Diagnostics;
using System.Globalization;
using System.Text;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input.Platform;
using Avalonia.Layout;
using Avalonia.Media;

namespace RorolalaDesktopHost;

/// <summary>
/// Starts the Desktop program and is what a crash is reported to.
/// </summary>
/// <remarks>
/// The Desktop program is a window, and a window has no terminal to print to: this program owns the
/// pipe between it and whoever asked for the window. It forwards what the Desktop writes where results
/// go, so a caller waiting for the window's readiness line still receives it, and keeps what it writes
/// where errors go. A run that ends with anything but zero prints that report, writes it to a file, and
/// opens a window for it, because a launcher has no terminal and a terminal is not there after a
/// launcher closes.
/// </remarks>
internal static class Program
{
    /// <summary>The name the Desktop program is published under, before a platform's suffix.</summary>
    private const string DesktopName = "RorolalaDesktop";

    /// <summary>Where a crash report is kept when the program cannot be told where to put it.</summary>
    private const string ReportDirectory = "rorolala";

    /// <summary>
    /// The code a start that found the Desktop already running ends with.
    /// </summary>
    /// <remarks>
    /// That ending is not a failure: it says the window was already there and this start asked it to
    /// come forward. The number is the Desktop's own, and the command line reads it to tell a call
    /// from a window, so a report and a crash window here would be reporting a perfectly ordinary
    /// answer as a fault.
    /// </remarks>
    private const int Called = 174;

    /// <summary>Starts the Desktop, waits for it, and reports a run that did not end cleanly.</summary>
    /// <param name="args">What was handed to this program, passed on unchanged.</param>
    /// <returns>The Desktop program's own exit code.</returns>
    [STAThread]
    public static int Main(string[] args)
    {
        var desktop = DesktopPath();

        if (!File.Exists(desktop))
        {
            WriteError($"Rorolala Desktop is not beside its host: {desktop}");

            return 1;
        }

        var start = Start(desktop, args);

        Process? running;

        try
        {
            running = Process.Start(start);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            WriteError($"Rorolala Desktop could not be started: {error.Message}");

            return 1;
        }

        if (running is null)
        {
            WriteError("Rorolala Desktop could not be started.");

            return 1;
        }

        using (running)
        {
            var errors = new ConcurrentQueue<string>();

            running.ErrorDataReceived += (_, said) =>
            {
                if (said.Data is { } line)
                {
                    errors.Enqueue(line);
                }
            };
            running.BeginErrorReadLine();

            Forward(running);

            running.WaitForExit();
            running.CancelErrorRead();

            var code = running.ExitCode;

            if (code == Called)
            {
                return code;
            }

            var report = Report(code, string.Join(Environment.NewLine, errors));

            if (code != 0)
            {
                var log = WriteLog(report);

                // The window comes before the stream: a launcher has already let go of this
                // program's error stream by the time a crash happens, and a write to a stream
                // nobody holds must not be what keeps the report from being seen.
                Show(report, log);
                WriteError(report);

                return code;
            }

            // A clean run should have said nothing on this stream. If it did, it is still the only
            // place that says it, so it goes on rather than being dropped by the program that reads it.
            if (!errors.IsEmpty)
            {
                WriteError(string.Join(Environment.NewLine, errors));
            }

            return code;
        }
    }

    /// <summary>Where the Desktop program sits, which is beside this program.</summary>
    /// <returns>The path.</returns>
    private static string DesktopPath()
    {
        var suffix = OperatingSystem.IsWindows() ? ".exe" : string.Empty;

        return Path.Combine(AppContext.BaseDirectory, DesktopName + suffix);
    }

    /// <summary>How the Desktop is started: its streams are pipes, and the directory is this run's.</summary>
    /// <param name="desktop">The program.</param>
    /// <param name="args">What it is told.</param>
    /// <returns>The start.</returns>
    private static ProcessStartInfo Start(string desktop, string[] args)
    {
        var start = new ProcessStartInfo
        {
            FileName = desktop,
            WorkingDirectory = Environment.CurrentDirectory,
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
            StandardOutputEncoding = Encoding.UTF8,
            StandardErrorEncoding = Encoding.UTF8,
        };

        foreach (var arg in args)
        {
            start.ArgumentList.Add(arg);
        }

        return start;
    }

    /// <summary>Reads the Desktop's results a line at a time and writes them to this program's own.</summary>
    /// <param name="running">The Desktop.</param>
    private static void Forward(Process running)
    {
        while (running.StandardOutput.ReadLine() is { } line)
        {
            try
            {
                Console.Out.WriteLine(line);
                Console.Out.Flush();
            }
            catch (Exception error) when (error is IOException or ObjectDisposedException)
            {
                // Nobody is listening any more, which is what a launcher leaving looks like. The
                // Desktop still has work to do, so the reading goes on rather than the pipe being
                // the reason it stops.
            }
        }
    }

    /// <summary>The report a run is given, whether it carried a stack or said nothing at all.</summary>
    /// <param name="code">What the Desktop ended with.</param>
    /// <param name="said">What it wrote where errors go.</param>
    /// <returns>The report.</returns>
    private static string Report(int code, string said)
    {
        var report = new StringBuilder();

        report.AppendLine(
            CultureInfo.InvariantCulture,
            $"Rorolala Desktop ended with code {code}."
        );
        report.AppendLine(
            CultureInfo.InvariantCulture,
            $"Working directory: {Environment.CurrentDirectory}"
        );
        report.AppendLine();
        report.AppendLine(
            said.Length > 0
                ? said
                : "The Desktop program said nothing on its error stream before it ended."
        );

        return report.ToString();
    }

    /// <summary>Writes the report beside the other crash reports, and answers where.</summary>
    /// <param name="report">What to write.</param>
    /// <returns>The file, or nothing when it could not be written.</returns>
    private static string? WriteLog(string report)
    {
        try
        {
            var directory = Path.Combine(Path.GetTempPath(), ReportDirectory);
            var path = Path.Combine(
                directory,
                $"desktop-crash-{DateTime.Now:yyyyMMdd-HHmmss}-{Environment.ProcessId}.log"
            );

            Directory.CreateDirectory(directory);
            File.WriteAllText(path, report);

            return path;
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return null;
        }
    }

    /// <summary>Shows the report in a window, and says so on the error stream when no window can be made.</summary>
    /// <param name="report">What to show.</param>
    /// <param name="log">Where it was written, or nothing.</param>
    private static void Show(string report, string? log)
    {
        try
        {
            CrashWindow.Show(report, log);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            WriteError($"The crash report could not be shown: {error.Message}");
        }
    }

    /// <summary>Writes to the error stream, which a launcher may have let go of by the time it is written.</summary>
    /// <param name="said">What to write.</param>
    private static void WriteError(string said)
    {
        try
        {
            Console.Error.WriteLine(said);
            Console.Error.Flush();
        }
        catch (Exception error) when (error is IOException or ObjectDisposedException)
        {
            // The same as above: the report is also in the window and the log, so a closed stream is
            // a reason to stop writing rather than a reason to lose the report.
        }
    }
}

/// <summary>The Avalonia application a crash report is shown in.</summary>
internal sealed class CrashApp : Application
{
    /// <inheritdoc />
    public override void Initialize()
    {
        Styles.Add(new Avalonia.Themes.Simple.SimpleTheme());
        RequestedThemeVariant = Avalonia.Styling.ThemeVariant.Dark;
    }

    /// <inheritdoc />
    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is Avalonia.Controls.ApplicationLifetimes.IClassicDesktopStyleApplicationLifetime desktop)
        {
            desktop.MainWindow = new CrashWindow();
        }

        base.OnFrameworkInitializationCompleted();
    }
}

/// <summary>The window a crash report is read in.</summary>
internal sealed class CrashWindow : Window
{
    /// <summary>The report shown when this window is made.</summary>
    private static string _report = string.Empty;

    /// <summary>Where the report was written, or nothing.</summary>
    private static string? _log;

    /// <summary>Makes the window a report is shown in.</summary>
    public CrashWindow()
    {
        Title = "Rorolala Desktop stopped";
        Width = 960;
        Height = 680;
        MinWidth = 640;
        MinHeight = 420;
        Background = new SolidColorBrush(Color.Parse("#1A1B20"));
        Foreground = new SolidColorBrush(Color.Parse("#E8E9EC"));
        RequestedThemeVariant = Avalonia.Styling.ThemeVariant.Dark;

        var heading = new TextBlock
        {
            Text = "Rorolala Desktop stopped",
            FontSize = 20,
            FontWeight = FontWeight.SemiBold,
            Foreground = new SolidColorBrush(Color.Parse("#E8E9EC")),
            Margin = new Thickness(0, 0, 0, 6),
        };

        var where = new TextBlock
        {
            Text = _log is { Length: > 0 }
                ? $"The full report is also in {_log}"
                : "The full report could not be written to a file.",
            TextWrapping = TextWrapping.Wrap,
            Foreground = new SolidColorBrush(Color.Parse("#96979B")),
            Margin = new Thickness(0, 0, 0, 12),
        };

        var body = new Border
        {
            Background = new SolidColorBrush(Color.Parse("#121318")),
            BorderBrush = new SolidColorBrush(Color.Parse("#34363E")),
            BorderThickness = new Thickness(1),
            Margin = new Thickness(0, 0, 0, 12),
            Child = new ScrollViewer
            {
                HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
                VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
                Padding = new Thickness(8),
                Content = new TextBlock
                {
                    Text = _report,
                    TextWrapping = TextWrapping.NoWrap,
                    FontFamily = new FontFamily(
                        "Cascadia Mono, Consolas, DejaVu Sans Mono, monospace"
                    ),
                    FontSize = 12,
                    Foreground = new SolidColorBrush(Color.Parse("#E8E9EC")),
                    VerticalAlignment = VerticalAlignment.Top,
                },
            },
        };

        var copy = new Button
        {
            Content = "Copy report",
            Foreground = new SolidColorBrush(Color.Parse("#E8E9EC")),
            Background = new SolidColorBrush(Color.Parse("#121318")),
            BorderBrush = new SolidColorBrush(Color.Parse("#34363E")),
        };

        copy.Click += async (_, _) =>
        {
            if (Clipboard is { } clipboard)
            {
                await clipboard.SetTextAsync(_report);
            }
        };

        var close = new Button
        {
            Content = "Close",
            Foreground = new SolidColorBrush(Color.Parse("#E8E9EC")),
            Background = new SolidColorBrush(Color.Parse("#121318")),
            BorderBrush = new SolidColorBrush(Color.Parse("#34363E")),
        };

        close.Click += (_, _) => Close();

        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Right,
            Children = { copy, close },
        };

        var layout = new Grid
        {
            RowDefinitions = new RowDefinitions("Auto,Auto,*,Auto"),
            Margin = new Thickness(16),
        };

        Grid.SetRow(heading, 0);
        Grid.SetRow(where, 1);
        Grid.SetRow(body, 2);
        Grid.SetRow(buttons, 3);
        layout.Children.Add(heading);
        layout.Children.Add(where);
        layout.Children.Add(body);
        layout.Children.Add(buttons);

        Content = layout;
    }

    /// <summary>Shows a report and waits until it is closed.</summary>
    /// <param name="report">What to show.</param>
    /// <param name="log">Where it was written, or nothing.</param>
    public static void Show(string report, string? log)
    {
        _report = report;
        _log = log;

        AppBuilder
            .Configure<CrashApp>()
            .UsePlatformDetect()
            .LogToTrace()
            .StartWithClassicDesktopLifetime([]);
    }
}
