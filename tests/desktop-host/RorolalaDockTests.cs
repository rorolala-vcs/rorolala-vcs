using Avalonia.Controls;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaVCSPlugin;
using Vcs = global::RorolalaVCSPlugin.RorolalaVCSPlugin;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// The Rorolala plugin's own docks: which three there are, and what one says about where the work is.
/// </summary>
/// <remarks>
/// Nothing here draws, and nothing here makes a window. What is exercised is what the <c>Window</c> menu
/// is built from, the judgement a dock's view is filled by, and the control that judgement is put into —
/// which is what a run with no Workspace under its base directory comes to.
/// </remarks>
public sealed class RorolalaDockTests
{
    /// <summary>Begins each test from no lock provider at all, since starting the plugin adds one.</summary>
    public RorolalaDockTests() => EntryLockProviders.Clear();

    /// <summary>The plugin adds one dock for each view of the work, named and titled apart.</summary>
    [Fact]
    public void ThePluginRegistersTheThreeViewsOfTheWork()
    {
        var services = Host.Services();
        var plugin = new Vcs();

        plugin.Initialize(services.For(plugin.Manifest.Id, 1));

        Assert.Equal(
            [
                ("rorolala.vcs.overview", "rorolala_vcs.dock.overview"),
                ("rorolala.vcs.workspace", "rorolala_vcs.dock.workspace"),
                ("rorolala.vcs.version_graph", "rorolala_vcs.dock.version_graph"),
            ],
            services.Docks.Registrations.Select(dock => (dock.DockNameId, dock.DisplayNameKey))
        );

        // One view of the work is one dock: asking for one that is open shows it rather than making a
        // second, which is what the Window menu's list of them is read against.
        Assert.All(
            services.Docks.Registrations,
            dock =>
            {
                Assert.Equal(plugin.Manifest.Id, dock.Owner);
                Assert.Equal(DockOpenMode.Toggle, dock.OpenMode);
            }
        );
    }

    /// <summary>A base directory no Workspace holds is said so rather than left blank.</summary>
    /// <remarks>
    /// A scratch directory is under no Workspace, and a directory that was never said — nothing, or the
    /// computer, which is not a path at all — is under none either. What a dock does with the answer is
    /// that it shows these words, so what is checked is that there are words to show.
    /// </remarks>
    [Fact]
    public void ABaseDirectoryNoWorkspaceHoldsIsSaidSo()
    {
        var services = Host.Services();
        var plugin = new Vcs();
        var host = services.For(plugin.Manifest.Id, 1);

        string?[] bases = [Scratch.New("outside"), null, string.Empty];

        foreach (var directory in bases)
        {
            var said = WorkspaceNotice.Notice(directory, host.Log);

            Assert.False(string.IsNullOrEmpty(said));
        }
    }

    /// <summary>
    /// A dock made while no Workspace holds the base is a control holding the words, not an empty one.
    /// </summary>
    /// <remarks>
    /// The judgement and the control are two halves, and a dock that decides rightly and carries nothing
    /// is a window the user reads as an empty work: what is checked here is that what the registration
    /// makes is a control whose content is the line, drawn rather than hidden. Nothing about a Workspace
    /// is asked, since the base published by an earlier test of this run is no more a Workspace than a
    /// fresh one is — and the line is what a base outside every Workspace comes to either way.
    /// </remarks>
    [Fact]
    public void ADockOutsideEveryWorkspaceCarriesTheWordsItIsMadeWith()
    {
        var services = Host.Services();
        var plugin = new Vcs();

        plugin.Initialize(services.For(plugin.Manifest.Id, 1));

        // A base nothing holds, which is the state a run outside every Workspace is in.
        BaseDirectory.Publish(Scratch.New("outside"));

        foreach (var registration in services.Docks.Registrations)
        {
            var view = registration.Create(DockPlacement.Center).View;

            var line = Assert.IsType<TextBlock>(Assert.IsAssignableFrom<UserControl>(view).Content);

            Assert.True(line.IsVisible);
            Assert.False(string.IsNullOrEmpty(line.Text));
        }
    }
}

