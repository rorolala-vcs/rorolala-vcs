using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Platform.Storage;
using Avalonia.Threading;
using FileSystemPlugin;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace RorolalaVCSPlugin;

/// <summary>
/// The accounts on this machine: which one the work acts as, and what may be done about them.
/// </summary>
/// <remarks>
/// A table of the accounts the command line lists, one row each — its name, whether it is the one in use, and
/// what may be done about it: name it, export its public key, take it away — with a field above for making a
/// new one. What each row does is a run of `rola`, so the dock answers what the command line would answer and
/// nothing here is a second account of the same work.
/// <para>
/// The runs are made off the window's thread, because each one asks the command line, which reads the keys and
/// may write a pair; what is drawn when they answer is drawn through the dispatcher, since a control belongs to
/// the thread that made it. The table is drawn again after every run, so what is on screen is what the command
/// line last said rather than what this side guessed it became.
/// </para>
/// </remarks>
internal sealed class AccountsPanel : UserControl
{
    /// <summary>How wide the line said in place of a table is allowed to read.</summary>
    private const double Widest = 420;

    /// <summary>The host, which is what a button reaches to run and to report, and where the picker is.</summary>
    private readonly IPluginHost _host;

    /// <summary>Where a run that could not be made is said.</summary>
    private readonly ILog _log;

    /// <summary>Where the account rows go, one to an account the command line listed.</summary>
    private readonly StackPanel _rows = new() { Spacing = 2 };

    /// <summary>The line said when there is no table to draw.</summary>
    private readonly TextBlock _notice = new()
    {
        TextWrapping = TextWrapping.Wrap,
        TextAlignment = TextAlignment.Center,
        HorizontalAlignment = HorizontalAlignment.Center,
        VerticalAlignment = VerticalAlignment.Center,
        MaxWidth = Widest,
        Classes = { "muted" },
    };

    /// <summary>The name a new pair is made under.</summary>
    private readonly TextBox _name = new()
    {
        PlaceholderText = RolaI18N.Get("rorolala_vcs.accounts.name_watermark"),
        VerticalAlignment = VerticalAlignment.Center,
    };

    /// <summary>What makes the pair, and what every other button waits on while one is going.</summary>
    private readonly Button _create = new()
    {
        Content = RolaI18N.Get("rorolala_vcs.accounts.create"),
        Classes = { "ghost" },
        VerticalAlignment = VerticalAlignment.Center,
    };

    /// <summary>Every button of the table, with whether it answers when no run is going.</summary>
    /// <remarks>
    /// The two are kept together because holding the table and restoring it are one answer each: a button that
    /// is not offered — the one naming the account the work already acts as — must be let go of as pressed-up
    /// as it was, and a list of buttons alone would answer it as free again the moment the table was restored.
    /// </remarks>
    private readonly List<(Button Button, bool Free)> _buttons = [];

    /// <summary>Whether one of the runs is going, which is what holds the buttons.</summary>
    private bool _busy;

    /// <summary>Makes the control over the host it runs through.</summary>
    /// <param name="host">The host, for the dialogs, the icons and the reports.</param>
    /// <param name="log">Where a run that could not be made is said.</param>
    public AccountsPanel(IPluginHost host, ILog log)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(log);

        _host = host;
        _log = log;

        _create.Click += (_, _) => Make();

        // A name typed and entered is a pair made, the way the button does it: a field with a button beside it
        // that takes the keyboard's enter as nothing is a field a reader types twice.
        _name.KeyDown += (_, pressed) =>
        {
            if (pressed.Key == Key.Enter)
            {
                Make();
            }
        };

        _buttons.Add((_create, true));

        var make = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,Auto"),
            ColumnSpacing = 6,
        };

        Grid.SetColumn(_name, 0);
        Grid.SetColumn(_create, 1);
        make.Children.Add(_name);
        make.Children.Add(_create);

        var body = new Grid
        {
            Children =
            {
                new ScrollViewer
                {
                    Content = _rows,
                    HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
                },
                _notice,
            },
        };

        var root = new Grid
        {
            RowDefinitions = new RowDefinitions("Auto,*"),
            RowSpacing = 10,
            Margin = new Thickness(12),
        };

        Grid.SetRow(make, 0);
        Grid.SetRow(body, 1);
        root.Children.Add(make);
        root.Children.Add(body);

        Content = root;

        AttachedToVisualTree += (_, _) => Refresh();
    }

    /// <summary>Reads the accounts again and draws them, which is what the dock's own command asks for.</summary>
    public void Refresh()
    {
        if (_busy)
        {
            return;
        }

        Hold(true);

        _ = Task.Run(() =>
        {
            var (done, accounts, said) = AccountActions.List(Directory());

            On(() =>
            {
                Hold(false);
                Show(done, accounts, said);
            });
        });
    }

    /// <summary>Draws the accounts a run listed, or why there are none to draw.</summary>
    /// <param name="done">Whether the run ended well.</param>
    /// <param name="accounts">The accounts it listed.</param>
    /// <param name="said">What it said when it did not end well.</param>
    private void Show(bool done, IReadOnlyList<Account> accounts, string said)
    {
        _rows.Children.Clear();
        _buttons.Clear();
        _buttons.Add((_create, true));

        var line = !done
            ? Said(said)
            : accounts.Count == 0
                ? RolaI18N.Get("rorolala_vcs.accounts.none")
                : null;

        _notice.Text = line;
        _notice.IsVisible = line is { Length: > 0 };

        foreach (var account in accounts)
        {
            _rows.Children.Add(Row(account));
        }
    }

    /// <summary>One account's row: what it is called, whether it is the one, and what may be done about it.</summary>
    /// <param name="account">The account.</param>
    /// <returns>The row.</returns>
    private Control Row(Account account)
    {
        var name = new TextBlock
        {
            Text = account.Name,
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };

        var mark = new TextBlock
        {
            Text = account.Current ? RolaI18N.Get("rorolala_vcs.accounts.current") : string.Empty,
            Classes = { "muted" },
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(8, 0),
        };

        // Naming the account the work already acts as is nothing to do, so it is the one button a row disables:
        // the reader is being told which one it is rather than offered it as a thing to press.
        var use = Button("rorolala_vcs.accounts.use", () => Use(account.Name), account.Current);
        var export = Button("rorolala_vcs.accounts.export", () => _ = Export(account.Name), false);
        var remove = Button("rorolala_vcs.accounts.remove", () => Ask(account.Name), false);

        var actions = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 2,
            VerticalAlignment = VerticalAlignment.Center,
        };

        actions.Children.Add(use);
        actions.Children.Add(export);
        actions.Children.Add(remove);

        var row = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,Auto,Auto"),
            Height = 30,
        };

        Grid.SetColumn(name, 0);
        Grid.SetColumn(mark, 1);
        Grid.SetColumn(actions, 2);
        row.Children.Add(name);
        row.Children.Add(mark);
        row.Children.Add(actions);

        return row;
    }

    /// <summary>A row's operation as a button, which the table holds while any run is going.</summary>
    /// <param name="labelKey">What the button is called.</param>
    /// <param name="command">What pressing it does.</param>
    /// <param name="disabled">Whether it is drawn unpressable whatever else is.</param>
    /// <returns>The button.</returns>
    private Button Button(string labelKey, Action command, bool disabled)
    {
        var button = new Button
        {
            Content = RolaI18N.Get(labelKey),
            Classes = { "ghost" },
            VerticalAlignment = VerticalAlignment.Center,
            IsEnabled = !disabled,
        };

        button.Click += (_, _) => command();

        _buttons.Add((button, !disabled));

        return button;
    }

    /// <summary>Makes a pair under the name that was typed, if the name is one an account may have.</summary>
    private void Make()
    {
        if (_busy)
        {
            return;
        }

        var name = _name.Text?.Trim() ?? string.Empty;

        if (name.Length == 0)
        {
            return;
        }

        // The two names `rola account` reads as its own words cannot be an account: a pair made under one of
        // them could never be chosen, so it is refused before it is made rather than after.
        if (AccountActions.Forbidden.Contains(name, StringComparer.Ordinal))
        {
            _host.Dialogs.Report(
                new Report(
                    RolaI18N.Get("rorolala_vcs.accounts.failed_create"),
                    RolaI18N.Get("rorolala_vcs.accounts.forbidden", name)
                )
            );

            return;
        }

        Hold(true);

        _ = Task.Run(() =>
        {
            var (done, said) = AccountActions.Create(name, Directory());

            On(() =>
            {
                Hold(false);

                if (!done)
                {
                    FailureNotice.Show(
                        _host.Dialogs,
                        _host.Icons,
                        RolaI18N.Get("rorolala_vcs.accounts.failed_create"),
                        Said(said)
                    );

                    return;
                }

                _name.Text = string.Empty;
                Refresh();
            });
        });
    }

    /// <summary>Names an account as the one the work acts as.</summary>
    /// <param name="name">The account.</param>
    private void Use(string name)
    {
        if (_busy)
        {
            return;
        }

        Hold(true);

        _ = Task.Run(() =>
        {
            var (done, said) = AccountActions.Use(name, Directory());

            On(() =>
            {
                Hold(false);

                if (!done)
                {
                    FailureNotice.Show(
                        _host.Dialogs,
                        _host.Icons,
                        RolaI18N.Get("rorolala_vcs.accounts.failed_use"),
                        Said(said)
                    );

                    return;
                }

                Refresh();

                // The account in use is what "mine" means everywhere else: the Ownership column, whether an entry is
                // already here, and which rows the upstream place may name. Choosing another one makes every reading that
                // asked stale, so they are let go of rather than drawn again out of what they hold.
                _host.Files.Touch();
            });
        });
    }

    /// <summary>Asks about taking an account away, and takes it when the answer is yes.</summary>
    /// <remarks>
    /// A pair is the identity the work acts as, and what it proves cannot be got back — a new pair is a new
    /// identity — so it is asked about rather than deleted. The command line has no word for this, so what is
    /// taken away are the two files the pair is: the private key the account is, and the public one beside it.
    /// </remarks>
    /// <param name="name">The account.</param>
    private void Ask(string name) =>
        _host.Dialogs.Show(
            new Dialog(
                RolaI18N.Get("rorolala_vcs.accounts.remove_title"),
                RolaI18N.Get("rorolala_vcs.accounts.remove_message", name),
                () => Remove(name)
            )
        );

    /// <summary>Takes an account's pair away, in the place the command line finds it.</summary>
    /// <param name="name">The account.</param>
    private void Remove(string name)
    {
        if (_busy)
        {
            return;
        }

        Hold(true);

        _ = Task.Run(() =>
        {
            var (done, paths, said) = AccountActions.Paths(secret: true, Directory());

            var found = done
                ? paths.FirstOrDefault(path => string.Equals(AccountActions.Named(path), name, StringComparison.Ordinal))
                : null;

            if (!done)
            {
                On(() =>
                {
                    Hold(false);
                    FailureNotice.Show(
                        _host.Dialogs,
                        _host.Icons,
                        RolaI18N.Get("rorolala_vcs.accounts.failed_remove"),
                        Said(said)
                    );
                });

                return;
            }

            if (found is null)
            {
                On(() =>
                {
                    Hold(false);
                    _host.Dialogs.Report(new Report(RolaI18N.Get("rorolala_vcs.accounts.failed_remove"), RolaI18N.Get("rorolala_vcs.accounts.no_key", name)));
                });

                return;
            }

            var public_ = Path.ChangeExtension(found, "pub");

            try
            {
                File.Delete(found);

                if (File.Exists(public_))
                {
                    File.Delete(public_);
                }
            }
            catch (Exception error) when (error is not OutOfMemoryException)
            {
                _log.Warn($"the key of `{name}` could not be removed: {error.Message}");

                On(() =>
                {
                    Hold(false);
                    _host.Dialogs.Report(new Report(RolaI18N.Get("rorolala_vcs.accounts.failed_remove"), error.Message));
                });

                return;
            }

            On(() =>
            {
                Hold(false);
                Refresh();
            });
        });
    }

    /// <summary>
    /// Saves an account's public key where the user says, through the platform's own picker.
    /// </summary>
    /// <remarks>
    /// The picker is the platform's, reached through the window this dock is in. A platform that offers none —
    /// a headless one, or one with no desktop portal — is answered with a report rather than with a second
    /// picker of this plugin's own: what to save and where is the host's to ask, and a plugin with a window of
    /// its own would be one the reader has to be told about twice.
    /// </remarks>
    /// <param name="name">The account whose public key is saved.</param>
    /// <returns>What the run came to, which nothing reads.</returns>
    private async Task Export(string name)
    {
        if (_busy)
        {
            return;
        }

        if (TopLevel.GetTopLevel(this)?.StorageProvider is not { } picker)
        {
            _host.Dialogs.Report(
                new Report(RolaI18N.Get("rorolala_vcs.accounts.failed_export"), RolaI18N.Get("rorolala_vcs.accounts.no_picker"))
            );

            return;
        }

        Hold(true);

        var (done, paths, said) = await Task.Run(() => AccountActions.Paths(secret: false, Directory()));

        var found = done
            ? paths.FirstOrDefault(path => string.Equals(AccountActions.Named(path), name, StringComparison.Ordinal))
            : null;

        if (!done || found is null)
        {
            Hold(false);

            if (!done)
            {
                FailureNotice.Show(
                    _host.Dialogs,
                    _host.Icons,
                    RolaI18N.Get("rorolala_vcs.accounts.failed_export"),
                    Said(said)
                );
            }
            else
            {
                _host.Dialogs.Report(
                    new Report(
                        RolaI18N.Get("rorolala_vcs.accounts.failed_export"),
                        RolaI18N.Get("rorolala_vcs.accounts.no_key", name)
                    )
                );
            }

            return;
        }

        try
        {
            var chosen = await picker.SaveFilePickerAsync(
                new FilePickerSaveOptions
                {
                    Title = RolaI18N.Get("rorolala_vcs.accounts.export_title"),
                    SuggestedFileName = name + ".pub",
                }
            );

            if (chosen is null)
            {
                Hold(false);
                return;
            }

            await using (var write = await chosen.OpenWriteAsync())
            {
                if (write.CanSeek)
                {
                    write.SetLength(0);
                }

                await using var read = File.OpenRead(found);
                await read.CopyToAsync(write);
            }
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Warn($"the public key of `{name}` could not be saved: {error.Message}");

            Hold(false);

            _host.Dialogs.Report(
                new Report(RolaI18N.Get("rorolala_vcs.accounts.failed_export"), error.Message)
            );

            return;
        }

        Hold(false);
    }

    /// <summary>Holds the table's buttons, or lets them answer again.</summary>
    /// <param name="busy">Whether a run is going.</param>
    private void Hold(bool busy)
    {
        _busy = busy;

        foreach (var (button, free) in _buttons)
        {
            button.IsEnabled = !busy && free;
        }

        _name.IsEnabled = !busy;
    }

    /// <summary>Where the runs are made, which is the base the browser is rooted at.</summary>
    /// <remarks>
    /// The scopes a key is looked for in follow the run's directory — a Workspace's keys and the Vault's come
    /// first — so a run made where the work is lists what a run of the command line there would.
    /// </remarks>
    /// <returns>The directory to run in.</returns>
    private static string Directory() =>
        BaseDirectory.Current is { Length: > 0 } at ? at : Environment.CurrentDirectory;

    /// <summary>What to say when a run failed, which is the command's own words or why there were none.</summary>
    /// <param name="said">What the command said.</param>
    /// <returns>The message to show.</returns>
    private static string Said(string said) =>
        said is { Length: > 0 } ? said : RolaI18N.Get("rorolala_vcs.accounts.no_program");

    /// <summary>Runs something on the window's thread, wherever it was called from.</summary>
    /// <param name="said">What to do.</param>
    private static void On(Action said) => Dispatcher.UIThread.Post(said);
}
