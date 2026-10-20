using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Platform.Storage;
using RorolalaDesktop.I18n;
using RorolalaLauncher.Controls;
using RorolalaLauncher.Rola;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Dialogs;

/// <summary>
/// Creates a Workspace: an address to work against, a directory to work in, and a check that the first can
/// be reached before the second is made.
/// </summary>
/// <remarks>
/// What is checked here is checked twice over, because a creation is not something to find out about
/// afterwards: the address is read as an address as it is typed, the directory is read as one that may be
/// created as it is typed, and the address is reached once — by a handshake spoken as the account in use —
/// before the button that makes anything is allowed to be pressed. Editing the address takes the check back,
/// since a check is about the words that were checked.
/// <para>
/// A creation that fails does not close the window: what a person typed is theirs, and losing it over a
/// daemon that was not up would be losing more than the failure is worth.
/// </para>
/// </remarks>
internal sealed class CreateWorkspaceDialog : DialogWindow
{
    /// <summary>The colours in force.</summary>
    private readonly Skin _skin;

    /// <summary>The command line program.</summary>
    private readonly RolaRunner _rola;

    /// <summary>What the connection is checked through.</summary>
    private readonly Connection _connection;

    /// <summary>Where the address is written.</summary>
    private readonly TextBox _address;

    /// <summary>Where the directory is written.</summary>
    private readonly TextBox _directory;

    /// <summary>What is wrong with the address.</summary>
    private readonly TextBlock _addressSays;

    /// <summary>What is wrong with the directory.</summary>
    private readonly TextBlock _directorySays;

    /// <summary>
    /// What the last thing this window did came to, which is the check of the connection or the creation.
    /// </summary>
    /// <remarks>
    /// One line rather than two: the two are never said at once — a creation began after a check that
    /// answered — and a window this short has room for one line at the bottom, not a stack of them.
    /// </remarks>
    private readonly TextBlock _status;

    /// <summary>The button that asks the platform for a directory.</summary>
    private readonly FlatButton _browse;

    /// <summary>The button that reaches the daemon.</summary>
    private readonly FlatButton _check;

    /// <summary>The button that makes the Workspace.</summary>
    private readonly FlatButton _create;

    /// <summary>Whether something is being done, so that nothing else is.</summary>
    private bool _busy;

    /// <summary>The address the handshake last answered for, if one did.</summary>
    private string? _reached;

    /// <summary>Whether this window made the directory, which is what a retry must not try again.</summary>
    private bool _created;

    /// <summary>Makes the window.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="rola">The command line program.</param>
    /// <param name="connection">What the address is reached through.</param>
    public CreateWorkspaceDialog(Skin skin, RolaRunner rola, Connection connection)
        : base(skin, RolaI18N.Get("launcher.create.title"), 500, 280)
    {
        ArgumentNullException.ThrowIfNull(rola);
        ArgumentNullException.ThrowIfNull(connection);

        _skin = skin;
        _rola = rola;
        _connection = connection;

        _address = Field(skin, RolaI18N.Get("launcher.create.address"));
        _directory = Field(skin, RolaI18N.Get("launcher.create.directory"));

        _addressSays = Says(skin, alarm: true);
        _directorySays = Says(skin, alarm: true);
        _status = Says(skin, alarm: false);

        // The window is a fixed 280 tall, so every line in it is one line tall: a field that says what is
        // wrong with it says it to the width it has, and the line at the bottom — which is where a reason the
        // command line gave goes — has two lines and then an ellipsis. A window that grew as it was filled in
        // would be a window that moved the buttons under the hand that was reaching for them.
        _addressSays.TextWrapping = TextWrapping.NoWrap;
        _addressSays.TextTrimming = TextTrimming.CharacterEllipsis;
        _directorySays.TextWrapping = TextWrapping.NoWrap;
        _directorySays.TextTrimming = TextTrimming.CharacterEllipsis;
        _status.MaxLines = 2;
        _status.TextTrimming = TextTrimming.CharacterEllipsis;

        Body.Margin = new Thickness(24, 6, 24, 16);

        _check = new FlatButton(Line(skin, RolaI18N.Get("launcher.create.check")), skin, ButtonKind.Surface);
        _create = new FlatButton(Line(skin, RolaI18N.Get("launcher.create.create")), skin, ButtonKind.Primary);

        var cancel = new FlatButton(Line(skin, RolaI18N.Get("launcher.common.cancel")), skin);

        _browse = new FlatButton(Line(skin, RolaI18N.Get("launcher.create.browse")), skin, ButtonKind.Surface)
        {
            Focusable = true,
        };

        _check.Clicked += (_, _) => _ = Check();
        _create.Clicked += (_, _) => _ = Make();
        _browse.Clicked += (_, _) => _ = Choose();
        cancel.Clicked += (_, _) => Close();

        _address.TextChanged += (_, _) => Edited();
        _directory.TextChanged += (_, _) => Edited();

        // The two fields are two rows of one grid, so that the fields stand in one column and the two buttons
        // in the one after it: a label written above its field would make the window taller than the two
        // questions in it need, and a label beside it makes the two fields read as one form.
        var labels = new ColumnDefinitions("Auto,*,Auto");

        var grid = new Grid
        {
            ColumnDefinitions = labels,
            RowDefinitions = new RowDefinitions("Auto,Auto,Auto,Auto"),
            ColumnSpacing = 10,
            RowSpacing = 4,
        };

        Body.Spacing = 8;

        var address = Label(skin, RolaI18N.Get("launcher.create.address_label"));
        var directory = Label(skin, RolaI18N.Get("launcher.create.directory_label"));

        _check.HorizontalAlignment = HorizontalAlignment.Left;
        _browse.HorizontalAlignment = HorizontalAlignment.Left;

        grid.Children.Add(address);
        Grid.SetColumn(_address, 1);
        grid.Children.Add(_address);
        Grid.SetColumn(_check, 2);
        grid.Children.Add(_check);

        Grid.SetColumn(_addressSays, 1);
        Grid.SetRow(_addressSays, 1);
        grid.Children.Add(_addressSays);

        Grid.SetRow(directory, 2);
        grid.Children.Add(directory);
        Grid.SetColumn(_directory, 1);
        Grid.SetRow(_directory, 2);
        grid.Children.Add(_directory);
        Grid.SetColumn(_browse, 2);
        Grid.SetRow(_browse, 2);
        grid.Children.Add(_browse);

        Grid.SetColumn(_directorySays, 1);
        Grid.SetRow(_directorySays, 3);
        grid.Children.Add(_directorySays);

        Body.Children.Add(grid);
        Body.Children.Add(_status);
        Body.Children.Add(Row(cancel, _create));

        Update();
    }

    /// <summary>The Workspace this window made, if it made one.</summary>
    public string? Created { get; private set; }

    /// <summary>The address that was reached, as it was written.</summary>
    private string Address => _address.Text?.Trim() ?? string.Empty;

    /// <summary>The directory that is to be made, as it was written.</summary>
    private string Place => _directory.Text?.Trim() ?? string.Empty;

    /// <summary>A line that says what is wrong with a field, which is hidden until something is.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="alarm">Whether it is something that went wrong.</param>
    /// <returns>The line.</returns>
    private static TextBlock Says(Skin skin, bool alarm)
    {
        var line = Line(skin, string.Empty, 12, muted: !alarm);

        if (alarm)
        {
            line.Foreground = skin.Alarm;
        }

        line.IsVisible = false;

        return line;
    }

    /// <summary>A field's name, read against the field's own left edge.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="text">What it says.</param>
    /// <returns>The label.</returns>
    private static TextBlock Label(Skin skin, string text)
    {
        var label = Line(skin, text, 12, muted: true);

        label.HorizontalAlignment = HorizontalAlignment.Right;
        label.VerticalAlignment = VerticalAlignment.Center;

        return label;
    }

    /// <summary>
    /// Asks the directory picker for a directory, and writes what it answered.
    /// </summary>
    /// <remarks>
    /// What the picker answers is written and then the caret is put after it: a field that is still being
    /// edited keeps the caret it had, and a field whose text grew under a caret that stayed where it was is a
    /// field that shows what it showed when the picker was opened. The caret is moved, and the field focused,
    /// so that what was written is what is read.
    /// </remarks>
    private async Task Choose()
    {
        // The keyboard is taken off the field before the picker opens: what a text box is still composing is
        // committed when it loses the keyboard, and a commit that landed after the answer would write over
        // the answer. The button that was pressed is the thing that holds it meanwhile, which is also where
        // the keyboard would go if the window were moved by hand.
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
            // The picker is the platform's, and a platform that cannot show one is a failure this window can
            // at least say out loud: the directory can still be written by hand.
            Say(_status, RolaI18N.Get("launcher.create.picker_failed", error.Message), alarm: true);

            return;
        }

        if (folders.Count == 0)
        {
            return;
        }

        // What the platform answered is asked for as a local path first, and as the path it holds when the
        // platform does not call it local — a portal that hands back a path it will not name otherwise is a
        // directory the window can still work in.
        var picked = folders[0].TryGetLocalPath() ?? folders[0].Path.LocalPath;

        if (string.IsNullOrWhiteSpace(picked))
        {
            return;
        }

        _directory.Text = picked;
        _directory.CaretIndex = picked.Length;
        _directory.Focus();
    }

    /// <summary>Speaks the handshake to the address, as the account the work acts as.</summary>
    private async Task Check()
    {
        if (_busy || !VaultAddressSyntax.Valid(Address))
        {
            return;
        }

        var address = Address;

        Busy(RolaI18N.Get("launcher.create.checking"));

        var report = await Task.Run(() => _connection.Check(address));

        Busy(null);

        if (report.Reached)
        {
            _reached = address;
            Say(_status, RolaI18N.Get("launcher.create.reached"), alarm: false);
            Update();

            return;
        }

        _reached = null;

        if (report.State == ConnectionState.NoAccount)
        {
            Say(_status, RolaI18N.Get("launcher.create.need_account"), alarm: true);
            Update();

            return;
        }

        Say(_status, RolaI18N.Get("launcher.create.failed", report.Reason), alarm: true);
        Update();
    }

    /// <summary>
    /// Makes the Workspace, and puts it against the address.
    /// </summary>
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

        var address = Address;
        var directory = Place;

        Busy(RolaI18N.Get("launcher.create.creating"));

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

        Busy(null);

        if (failure.Length > 0)
        {
            Say(_status, failure, alarm: true);

            return;
        }

        Created = directory;
        Close();
    }

    /// <summary>Whether the window may make anything: both fields read, and the address reached.</summary>
    private bool Ready =>
        !_busy
        && VaultAddressSyntax.Valid(Address)
        && (_created || Problem(Place) is null)
        && _reached == Address
        && Address.Length > 0;

    /// <summary>
    /// Reads both fields again after either changed.
    /// </summary>
    /// <remarks>
    /// An address that changed takes the check back with it: a handshake that answered for one address does
    /// not answer for another, and a creation would otherwise be let through on a check of something else.
    /// </remarks>
    private void Edited()
    {
        if (_reached is not null && _reached != Address)
        {
            _reached = null;
        }

        // The line at the bottom is the last thing this window said, and what was said about a field that has
        // since changed is no longer about anything: it is cleared here, and the window says again what stands
        // once the two fields are read.
        Say(_status, string.Empty, alarm: false);
        Update();
    }

    /// <summary>Draws what the two fields say, which is what the buttons stand on.</summary>
    private void Update()
    {
        var address = Address;
        var directory = Place;

        var addressProblem = address.Length == 0
            ? RolaI18N.Get("launcher.create.address_empty")
            : VaultAddressSyntax.Valid(address)
                ? null
                : RolaI18N.Get("launcher.create.address_invalid");

        var directoryProblem = _created ? null : Problem(directory);

        Say(_addressSays, addressProblem ?? string.Empty, alarm: true);
        Say(_directorySays, directoryProblem ?? string.Empty, alarm: true);

        _address.BorderBrush = addressProblem is null ? _skin.Splitter : _skin.Alarm;
        _directory.BorderBrush = directoryProblem is null ? _skin.Splitter : _skin.Alarm;

        if (_reached is null && string.IsNullOrEmpty(_status.Text))
        {
            Say(_status, RolaI18N.Get("launcher.create.unchecked"), alarm: false);
        }

        _check.Enabled = !_busy && addressProblem is null;
        _create.Enabled = Ready;
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
            return RolaI18N.Get("launcher.create.directory_empty");
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

    /// <summary>Holds the window while something is done, or lets it go.</summary>
    /// <param name="says">What is being done, or nothing when it is done.</param>
    private void Busy(string? says)
    {
        _busy = says is not null;

        _check.Child = Line(_skin, says ?? RolaI18N.Get("launcher.create.check"));
        _create.Child = Line(
            _skin,
            says ?? RolaI18N.Get("launcher.create.create")
        );

        _check.Repaint(_skin);
        _create.Repaint(_skin);

        Update();
    }

    /// <summary>
    /// Says one thing about a field, or takes the saying away.
    /// </summary>
    /// <remarks>
    /// The line keeps its room when it says nothing: a line that folded away would move the two fields and the
    /// buttons under it every time a question was answered, and a form that moves while it is being filled in
    /// is a form that is harder to fill in.
    /// </remarks>
    /// <param name="line">The line.</param>
    /// <param name="says">What it says.</param>
    /// <param name="alarm">Whether it is something that went wrong.</param>
    private void Say(TextBlock line, string says, bool alarm)
    {
        line.Text = says;
        line.Foreground = alarm ? _skin.Alarm : _skin.Muted;
        line.IsVisible = true;
    }
}
