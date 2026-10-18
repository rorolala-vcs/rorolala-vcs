using Avalonia.Threading;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// One fetch of the Vault's Layout at a time, shared by the views that read the work.
/// </summary>
/// <remarks>
/// The copy every reading is made from is one file, so two views asking at once would be two fetches for
/// a thing one of them is about to write. A request made while a fetch is going is run when it ends — it
/// asked for what that fetch is bringing — rather than started beside it.
/// <para>
/// What the fetch wrote is what every reading of the work is made from, so the files are said to have
/// changed when it ends: the columns of a listing, the Overview and the Version Graph all read the copy
/// this run brought over.
/// </para>
/// </remarks>
internal sealed class VaultFetch
{
    /// <summary>The host, for the run's card, for the files having changed, and for a failure's report.</summary>
    private readonly IPluginHost _host;

    /// <summary>What the run is drawn by, beside every other command this plugin runs.</summary>
    private readonly ProgressHub _hub;

    /// <summary>What asked to be read once the fetch is over, in the order they asked.</summary>
    private readonly List<Action> _after = [];

    /// <summary>Whether a fetch is going, which is what a second request waits for.</summary>
    private bool _fetching;

    /// <summary>Makes the fetcher.</summary>
    /// <param name="host">The host, for the run's card and for what a failure is said with.</param>
    /// <param name="hub">What every command this plugin runs reports itself to.</param>
    public VaultFetch(IPluginHost host, ProgressHub hub)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(hub);

        _host = host;
        _hub = hub;
    }

    /// <summary>
    /// Fetches the Vault's Layout under <paramref name="directory"/>, and reads what was asked for once
    /// it is over.
    /// </summary>
    /// <remarks>
    /// A window opened on its own has no program to fetch with, so what there is to read is read: a
    /// reader who asked to see the work is not answered with nothing because of how the window started.
    /// </remarks>
    /// <param name="directory">Where the work is rooted, and where the run is made.</param>
    /// <param name="done">What to run on the window's thread once the copy is here.</param>
    public void Fetch(string directory, Action done)
    {
        ArgumentNullException.ThrowIfNull(directory);
        ArgumentNullException.ThrowIfNull(done);

        if (_fetching)
        {
            _after.Add(done);

            return;
        }

        if (Environment.GetEnvironmentVariable(Sync.Program) is not { Length: > 0 } program)
        {
            done();

            return;
        }

        _fetching = true;

        var run = _hub.Begin("rorolala_vcs.progress.fetch");

        _ = Task.Run(() =>
        {
            // The static runner is named in full: this class's own Fetch would be the one looked for.
            var (done_well, said) = global::RorolalaVCSPlugin.Fetch.Run(program, directory);

            Dispatcher.UIThread.Post(() =>
            {
                _fetching = false;

                // The card is let go of here rather than when the run is over, because the panel keeps it for a
                // few seconds after: what ended is drawn as ended before it is taken away.
                _hub.End(run);

                // What was fetched is what every reading is made from, so what the columns answered about the
                // entries is let go of with the files, and the listing and the reading are read again.
                _host.Files.Touch();

                if (!done_well)
                {
                    // Drawn as every other command's failure is, and not as one line of raw text: what the
                    // command wrote says which of its lines went wrong and which can be acted on, and the window
                    // draws those marks rather than the words they are written with.
                    _host.Dialogs.Present(
                        RolaI18N.Get("rorolala_vcs.fetch.failed"),
                        () =>
                            ReportView.Of(
                                ReportLines.Parse(said),
                                key => _host.Icons.Find(key),
                                ReportView.Ink
                            )
                    );
                }

                done();

                // A request made while this fetch was going asked for the copy it brought, so each one reads
                // now rather than fetching the same layout again. The list is taken before it is run, since a
                // callback is free to ask for a fetch of its own.
                var after = _after.ToArray();

                _after.Clear();

                foreach (var read in after)
                {
                    read();
                }
            });
        });
    }
}
