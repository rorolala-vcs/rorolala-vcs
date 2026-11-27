using System.Diagnostics;

using Avalonia.Controls;
using Avalonia.Headless;
using Avalonia.Threading;
using Avalonia.VisualTree;

using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.I18n;
using RorolalaVCSPlugin;
using Vcs = global::RorolalaVCSPlugin.RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The Workspace dock drawn over a real Workspace: its rules, its log, and the line for a base that
/// is not one.
/// </summary>
/// <remarks>
/// The fixture is made by the command line rather than written here, since the journal and the
/// default rules are the writer's and the program's. What is checked is that the two panes are what
/// the view draws once a Workspace holds the base, and that the counts and the row read what the
/// files hold.
/// </remarks>
public sealed class WorkspaceViewTests
{
    /// <summary>The two panes are drawn for a Workspace, and what they say is what its files hold.</summary>
    [Fact]
    public void AWorkspaceIsDrawnAsItsRulesAndItsLog()
    {
        var parent = Scratch.New("workspace-view");
        var workspace = Path.Combine(parent, "ws");

        Given(parent, "create", workspace);
        Given(workspace, "key", "generate", "--install", "alice");
        Given(workspace, "account", "alice");

        File.WriteAllText(Path.Combine(workspace, "art.txt"), "one\n");
        Given(workspace, "track", "art.txt", "--message", "first", "--no-editor");

        // One rule beside the default ones, so the count says the file was read rather than assumed.
        File.AppendAllText(RolaPaths.WorkspaceIgnoreRuleFile(workspace), "*.custom\n");

        var services = Host.Services();
        var plugin = new Vcs();

        plugin.Initialize(services.For(plugin.Manifest.Id, 1));
        BaseDirectory.Publish(workspace);

        var registration = services.Docks.Registrations.Single(dock =>
            dock.DockNameId == RorolalaDocks.WorkspaceNameId
        );
        var view = Assert.IsType<WorkspaceView>(registration.Create().View);
        var window = new Window
        {
            Width = 900,
            Height = 600,
            Content = view,
        };

        window.Show();

        try
        {
            Pump();

            Assert.False(view.ShowsNotice);

            var words = view.GetVisualDescendants()
                .OfType<TextBlock>()
                .Select(text => text.Text)
                .ToHashSet(StringComparer.Ordinal);

            // The default rules and the one added here, and the run the command line recorded. The
            // catalogue states nothing in this suite, so a name reads as the key it is sent to.
            Assert.Contains("32", words);
            Assert.Contains(RolaI18N.Get("rorolala_vcs.workspace.cmd_track"), words);
        }
        finally
        {
            window.Close();
        }
    }

    /// <summary>Runs one command in a directory, failing the test when it does not end successfully.</summary>
    /// <param name="directory">Where to run it.</param>
    /// <param name="arguments">What the command is told.</param>
    private static void Given(string directory, params string[] arguments)
    {
        var start = new ProcessStartInfo(Program()) { WorkingDirectory = directory };

        foreach (var argument in arguments)
        {
            start.ArgumentList.Add(argument);
        }

        using var command = Process.Start(start)!;

        command.WaitForExit();

        Assert.Equal(0, command.ExitCode);
    }

    /// <summary>The built command line, whose directory the suites are told.</summary>
    /// <returns>The program to run.</returns>
    private static string Program() =>
        Path.Combine(
            Environment.GetEnvironmentVariable("ROLA_BIN_DIR")
                ?? throw new InvalidOperationException("the suites are run with ROLA_BIN_DIR"),
            OperatingSystem.IsWindows() ? "rola.exe" : "rola"
        );

    /// <summary>Lets the layout and the drawing catch up.</summary>
    private static void Pump()
    {
        AvaloniaHeadlessPlatform.ForceRenderTimerTick();
        Dispatcher.UIThread.RunJobs();
        Thread.Sleep(10);
    }
}
