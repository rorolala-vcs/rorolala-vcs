using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Platform.Storage;
using RorolalaDesktop.I18n;
using RorolalaLauncher.Controls;
using RorolalaLauncher.Rola;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Dialogs;

/// <summary>
/// Creates a Workspace: the Vault it is bound to, and the directory it is made in.
/// </summary>
/// <remarks>
/// The address is chosen from the dock rather than typed: an address is not something a person should have
/// to remember when the launcher already knows every one it can reach. What the picker says about
/// reachability is a warning rather than a gate. Binding does not open the Vault, so a Workspace can be made
/// against a daemon that is not answering; the connection is needed later, when something is read or
/// written, and refusing here would refuse a creation that would have worked.
/// </remarks>
internal sealed class CreateWorkspaceDialog : DialogWindow
{
    /// <summary>The colours in force.</summary>
    private readonly Skin _skin;

    /// <summary>The command line program.</summary>
    private readonly RolaRunner _rola;

    /// <summary>The Workspaces already on the list, which is what a directory is compared against.</summary>
    private readonly WorkspacesFile _opened;

    /// <summary>Every address the picker offers.</summary>
    private readonly IReadOnlyList<VaultEntry> _vaults;

    /// <summary>The rows the picker drew, so that a handshake that lands can redraw one.</summary>
    private readonly List<VaultPick> _rows = [];

    /// <summary>Where the directory is written.</summary>
    private readonly TextBox _directory;

    /// <summary>What is wrong with the directory.</summary>
    private readonly TextBlock _directorySays;

    /// <summary>The button that asks the platform for a directory.</summary>
    private readonly FlatButton _browse;

    /// <summary>The button that makes the Workspace.</summary>
    private readonly FlatButton _create;

    /// <summary>The address chosen, or nothing when there is nothing to choose.</summary>
    private VaultEntry? _selected;

    /// <summary>Whether something is being done, so that nothing else is.</summary>
    private bool _busy;

    /// <summary>Whether this window made the directory, which is what a retry must not try again.</summary>
    private bool _created;

    /// <summary>Makes the window.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="rola">The command line program.</param>
    /// <param name="vaults">Every address the picker offers.</param>
    /// <param name="opened">The Workspaces already on the list.</param>
    public CreateWorkspaceDialog(
        Skin skin,
        RolaRunner rola,
        IReadOnlyList<VaultEntry> vaults,
        WorkspacesFile opened
    )
        : base(skin, RolaI18N.Get("launcher.create.title"), 520, 430)
    {
        ArgumentNullException.ThrowIfNull(rola);
        ArgumentNullException.ThrowIfNull(vaults);
        ArgumentNullException.ThrowIfNull(opened);

        _skin = skin;
        _rola = rola;
        _vaults = vaults;
        _opened = opened;

        Height = double.NaN;
        SizeToContent = SizeToContent.Height;
        Body.Margin = new Thickness(18, 8, 18, 16);
        Body.Spacing = 13;

        foreach (var entry in vaults)
        {
            if (entry.Status == VaultStatus.Reachable)
            {
                _selected = entry;

                break;
            }
        }

        _selected ??= vaults.Count > 0 ? vaults[0] : null;

        var picker = new StackPanel();

        foreach (var entry in vaults)
        {
            var row = new VaultPick(
                entry,
                ReferenceEquals(entry, _selected),
                skin,
                () => Choose(entry)
            );

            _rows.Add(row);
            picker.Children.Add(row);

            entry.Changed += OnEntryChanged;
        }

        if (vaults.Count == 0)
        {
            picker.Children.Add(
                new TextBlock
                {
                    Text = RolaI18N.Get("launcher.create.vaults_empty"),
                    FontSize = 12,
                    Foreground = skin.Faint,
                    Margin = new Thickness(14, 16, 14, 16),
                }
            );
        }

        var pickHost = new Border
        {
            Background = skin.Dark,
            BorderBrush = skin.Splitter,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(7),
            MaxHeight = 212,
            ClipToBounds = true,
            Child = new ScrollViewer
            {
                Content = picker,
                VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
                HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
            },
        };

        _directory = Field(skin, string.Empty);

        _directory.Height = 34;
        _directory.CornerRadius = new CornerRadius(5);
        _directory.TextChanged += (_, _) =>
        {
            // A directory that changed is a directory a creation has not been run in: what was made
            // before is not the place the button would bind now.
            _created = false;

            Update();
        };

        _directorySays = Line(skin, string.Empty, 11.5, muted: true);

        _directorySays.IsVisible = false;
        _directorySays.TextWrapping = TextWrapping.NoWrap;
        _directorySays.TextTrimming = TextTrimming.CharacterEllipsis;

        _browse = new FlatButton(Line(skin, RolaI18N.Get("launcher.create.browse")), skin, ButtonKind.Surface)
        {
            Height = 34,
            CornerRadius = new CornerRadius(5),
            Padding = new Thickness(12, 0, 12, 0),
            VerticalAlignment = VerticalAlignment.Center,
            Focusable = true,
        };

        _browse.Clicked += (_, _) => _ = Choose();

        var place = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,Auto"),
            ColumnSpacing = 8,
        };

        place.Children.Add(_directory);

        Grid.SetColumn(_browse, 1);
        place.Children.Add(_browse);

        Body.Children.Add(Line(skin, RolaI18N.Get("launcher.create.vault_address"), 11.5, muted: true));
        Body.Children.Add(pickHost);
        Body.Children.Add(Block(skin, RolaI18N.Get("launcher.create.directory"), place, _directorySays));

        var cancel = new FlatButton(Line(skin, RolaI18N.Get("launcher.common.cancel")), skin)
        {
            Height = 32,
            CornerRadius = new CornerRadius(5),
        };

        _create = new FlatButton(Line(skin, RolaI18N.Get("launcher.create.create")), skin, ButtonKind.Primary)
        {
            Height = 32,
            CornerRadius = new CornerRadius(5),
        };

        cancel.Clicked += (_, _) => Close();
        _create.Clicked += (_, _) => _ = Make();

        Body.Children.Add(Row(cancel, _create));

        Closed += (_, _) =>
        {
            foreach (var entry in _vaults)
            {
                entry.Changed -= OnEntryChanged;
            }
        };

        Opened += (_, _) =>
        {
            if (_rows.Count > 0)
            {
                _rows[0].Focus();
            }
            else
            {
                _directory.Focus();
            }
        };

        Update();
    }

    /// <summary>The Workspace this window made, if it made one.</summary>
    public string? Created { get; private set; }

    /// <summary>The directory that is to be made, as it was written.</summary>
    private string Place => _directory.Text?.Trim() ?? string.Empty;

    /// <summary>Chooses an address.</summary>
    /// <param name="entry">The address chosen.</param>
    private void Choose(VaultEntry entry)
    {
        _selected = entry;

        foreach (var row in _rows)
        {
            row.SetChosen(ReferenceEquals(row.Entry, entry), _skin);
        }

        Update();
    }

    /// <summary>Draws a row again when its handshake landed, and the button that stands on it.</summary>
    /// <param name="sender">The entry that changed.</param>
    /// <param name="e">The event.</param>
    private void OnEntryChanged(object? sender, EventArgs e)
    {
        foreach (var row in _rows)
        {
            if (ReferenceEquals(row.Entry, sender))
            {
                row.Repaint(_skin);
            }
        }

        Update();
    }

    /// <summary>Makes the Workspace, and binds it to the address chosen.</summary>
    /// <remarks>
    /// The creation is the command line's, in the form a person would write it: <c>rola create</c> in the
    /// directory that is to be made, then <c>rola bind</c> inside it. A creation that made the directory and
    /// a binding that failed is retried as a binding only, since the directory is not the part that failed.
    /// </remarks>
    private async Task Make()
    {
        if (_busy || !Ready)
        {
            return;
        }

        var address = _selected!.Address;
        var directory = Place;

        Busy(true);

        var failure = string.Empty;

        if (!_created)
        {
            var made = await Task.Run(() => _rola.Run(["create", directory], Nearest(directory)));

            if (made.Done)
            {
                _created = true;
            }
            else
            {
                failure = RolaI18N.Get("launcher.create.failed_create", made.Reason);
            }
        }

        if (failure.Length == 0)
        {
            var bound = await Task.Run(() => _rola.Run(["bind", address], directory));

            if (!bound.Done)
            {
                failure = RolaI18N.Get("launcher.create.failed_create", bound.Reason);
            }
        }

        Busy(false);

        if (failure.Length > 0)
        {
            Say(failure);

            return;
        }

        Created = directory;
        Close();
    }

    /// <summary>Whether the window may make anything: an address chosen and a directory that is free.</summary>
    private bool Ready =>
        !_busy
        && _selected is not null
        && Place.Length > 0
        && (_created || Problem(Place) is null)
        && !_opened.Contains(Place);

    /// <summary>Draws what the fields say, which is what the button stands on.</summary>
    private void Update()
    {
        var duplicate = Place.Length > 0 && _opened.Contains(Place);
        var problem = duplicate
            ? RolaI18N.Get("launcher.create.directory_duplicate")
            : _created
                ? null
                : Problem(Place);

        _directorySays.Text = problem ?? string.Empty;
        _directorySays.Foreground = _skin.Alarm;
        _directorySays.IsVisible = problem is not null;
        _directory.BorderBrush = problem is null ? _skin.Splitter : _skin.Alarm;

        var warning = _selected is { Status: VaultStatus.Failed };
        var ready = Ready;

        _create.Enabled = ready;
        _create.Restyle(
            _skin,
            warning && ready ? ButtonKind.Warning : ButtonKind.Primary
        );

        ToolTip.SetTip(
            _create,
            warning && ready ? RolaI18N.Get("launcher.create.may_not_connect") : null
        );
    }

    /// <summary>
    /// What is wrong with a directory that a Workspace would be made in.
    /// </summary>
    /// <remarks>
    /// A directory that is not there is the ordinary case: the creation makes it. A directory that is
    /// there has to be empty, because a creation writes the Workspace's own files into it and a directory
    /// holding somebody's work is not one to write into.
    /// </remarks>
    /// <param name="path">The directory.</param>
    /// <returns>What is wrong with it, or nothing when nothing is.</returns>
    private static string? Problem(string path)
    {
        if (path.Length == 0)
        {
            return null;
        }

        try
        {
            if (File.Exists(path))
            {
                return RolaI18N.Get("launcher.create.directory_is_file");
            }

            if (!Directory.Exists(path))
            {
                return null;
            }

            return Directory.EnumerateFileSystemEntries(path).Any()
                ? RolaI18N.Get("launcher.create.directory_not_empty")
                : null;
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            return RolaI18N.Get("launcher.create.directory_unreadable", error.Message);
        }
    }

    /// <summary>The nearest directory above a path that is there, which is where a creation is run from.</summary>
    /// <param name="path">The path.</param>
    /// <returns>The directory.</returns>
    private static string Nearest(string path)
    {
        var held = Path.GetFullPath(path);

        while (!Directory.Exists(held))
        {
            var parent = Path.GetDirectoryName(held);

            if (parent is null || parent.Length == held.Length)
            {
                break;
            }

            held = parent;
        }

        return held;
    }

    /// <summary>
    /// Asks the platform for a directory, and writes what it answered.
    /// </summary>
    /// <remarks>
    /// The keyboard is taken off the field before the picker opens, because what a text box is still composing
    /// is committed when it loses the keyboard, and a commit that landed after the answer would write over it.
    /// A picker the platform cannot show leaves the field editable by hand rather than failing the window.
    /// </remarks>
    private async Task Choose()
    {
        _browse.Focus();

        IReadOnlyList<IStorageFolder> folders;

        try
        {
            folders = await StorageProvider.OpenFolderPickerAsync(
                new FolderPickerOpenOptions
                {
                    Title = RolaI18N.Get("launcher.create.browse"),
                    AllowMultiple = false,
                }
            );
        }
        catch (Exception error) when (error is InvalidOperationException or NotSupportedException)
        {
            Say(RolaI18N.Get("launcher.create.picker_failed", error.Message));

            return;
        }

        if (folders.Count == 0)
        {
            return;
        }

        var picked = folders[0].TryGetLocalPath() ?? folders[0].Path.LocalPath;

        if (string.IsNullOrWhiteSpace(picked))
        {
            return;
        }

        _directory.Text = picked;
        _directory.CaretIndex = picked.Length;
        _directory.Focus();
    }

    /// <summary>Holds the window while something is done, or lets it go.</summary>
    /// <param name="busy">Whether something is being done.</param>
    private void Busy(bool busy)
    {
        _busy = busy;

        _directory.IsEnabled = !busy;
        _browse.Enabled = !busy;

        _create.Child = Line(
            _skin,
            busy
                ? RolaI18N.Get("launcher.create.creating")
                : RolaI18N.Get("launcher.create.create")
        );

        _create.Repaint(_skin);

        Update();
    }

    /// <summary>A field's name and field, with the line that says what is wrong under it.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="label">What the field is called.</param>
    /// <param name="field">The field.</param>
    /// <param name="says">Where what is wrong is written.</param>
    /// <returns>The block.</returns>
    private static Control Block(Skin skin, string label, Control field, TextBlock says)
    {
        var block = new StackPanel { Spacing = 5 };

        block.Children.Add(Line(skin, label, 11.5, muted: true));
        block.Children.Add(field);
        block.Children.Add(says);

        return block;
    }

    /// <summary>Puts a line under the directory field.</summary>
    /// <param name="says">What it says.</param>
    private void Say(string says)
    {
        _directorySays.Text = says;
        _directorySays.Foreground = _skin.Alarm;
        _directorySays.IsVisible = true;
    }
}
