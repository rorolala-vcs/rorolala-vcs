using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Platform.Storage;
using RorolalaDesktop.I18n;
using RorolalaLauncher.Controls;
using RorolalaLauncher.Rola;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Dialogs;

/// <summary>
/// The one thing the launcher does about an account: makes sure there is one.
/// </summary>
/// <remarks>
/// It is not the Desktop program's accounts dock: that one lists every key on the machine, chooses among
/// them and takes them away, and this asks one question — who the person on this machine is — because a
/// launcher is the first thing they meet and an account is what a Workspace is worked as.
/// <para>
/// A name is not a password: it is the label on a key, it is what other people are shown, and it is not
/// something the program can change once a Vault has been told it. So it is asked for once, with the fact
/// that it is hard to change said beside it, and a machine that has an account is greeted rather than
/// asked again.
/// </para>
/// <para>
/// The public half of the key is exported here because a person with an account but no way to hand the
/// public half to whoever runs the Vault has nothing: the export is the one thing the greeting offers.
/// </para>
/// </remarks>
internal sealed class AccountDialog : DialogWindow
{
    /// <summary>Where the keys are found and where a run is made.</summary>
    private readonly LauncherFiles _files;

    /// <summary>The accounts, as the command line names them.</summary>
    private readonly Accounts _accounts;

    /// <summary>What the command line said about the accounts when the window opened.</summary>
    private readonly AccountList _spoke;

    /// <summary>The colours in force.</summary>
    private readonly Skin _skin;

    /// <summary>What the window is saying went wrong, or that it is busy.</summary>
    private readonly TextBlock _notice;

    /// <summary>The field a name is written in, on a machine that has no account.</summary>
    private TextBox? _name;

    /// <summary>The button the window is asking for, where it is asking for one.</summary>
    private FlatButton? _action;

    /// <summary>What an action is currently doing, so that a second press is not a second run.</summary>
    private bool _busy;

    /// <summary>What the last action said while it was running.</summary>
    private string? _lastBusy;

    /// <summary>Makes the window over what the command line said.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="files">Where the launcher keeps its files.</param>
    /// <param name="accounts">The accounts.</param>
    public AccountDialog(Skin skin, LauncherFiles files, Accounts accounts)
        : base(skin, RolaI18N.Get("launcher.account.title"), 440, 360)
    {
        ArgumentNullException.ThrowIfNull(files);
        ArgumentNullException.ThrowIfNull(accounts);

        _skin = skin;
        _files = files;
        _accounts = accounts;
        _spoke = accounts.List(files.Root);

        // The window is as tall as what it holds: the greeting is a line and a button, and the question is a
        // field and its warnings — one height for both would either crowd the question or leave the greeting
        // standing over a band of nothing.
        Height = double.NaN;
        SizeToContent = SizeToContent.Height;

        _notice = Line(skin, string.Empty, 12);
        _notice.IsVisible = false;

        ShowState();
    }

    /// <summary>The account this window made, if it made one.</summary>
    public string? Created { get; private set; }

    /// <summary>Draws the window as either a greeting or a question.</summary>
    private void ShowState()
    {
        Body.Children.Clear();

        var held = Created ?? _spoke.Current;

        if (held is not null)
        {
            Retitle(RolaI18N.Get("launcher.account.title"));

            Greet(held);

            return;
        }

        Retitle(RolaI18N.Get("launcher.account.title_new"));

        Ask();
    }

    /// <summary>Greets a person who has an account, and offers the one thing left to do.</summary>
    /// <param name="name">The account.</param>
    private void Greet(string name)
    {
        var hello = Line(_skin, RolaI18N.Get("launcher.account.hello", name), 22);

        hello.FontWeight = FontWeight.Bold;

        Body.Children.Add(hello);
        Body.Children.Add(
            Line(
                _skin,
                RolaI18N.Get("launcher.account.greeting"),
                13,
                muted: true
            )
        );

        var export = new FlatButton(Line(_skin, RolaI18N.Get("launcher.account.export")), _skin, ButtonKind.Primary);

        export.Clicked += (_, _) => _ = Export(name);

        Body.Children.Add(_notice);
        Body.Children.Add(Row(CloseButton(), export));
    }

    /// <summary>Asks for a name, on a machine that has no account.</summary>
    /// <remarks>
    /// The name is asked for and nothing else: what it will mean to have that name — that a Vault told it will
    /// be hard to tell otherwise — is said in the window that asks whether to go ahead with it, because a
    /// sentence that was already standing there when the name was typed is a sentence the eye has gone past.
    /// </remarks>
    private void Ask()
    {
        Body.Children.Add(Line(_skin, RolaI18N.Get("launcher.account.intro"), 18));

        _name = Field(_skin, RolaI18N.Get("launcher.account.name"));

        Body.Children.Add(_name);

        _action = new FlatButton(Line(_skin, RolaI18N.Get("launcher.account.create")), _skin, ButtonKind.Primary);

        _action.Clicked += (_, _) => _ = Make();

        Body.Children.Add(_notice);
        Body.Children.Add(Row(CloseButton(), _action));
    }

    /// <summary>The button that puts the window away.</summary>
    /// <returns>The button.</returns>
    private FlatButton CloseButton()
    {
        var close = new FlatButton(Line(_skin, RolaI18N.Get("launcher.common.close")), _skin);

        close.Clicked += (_, _) => Close();

        return close;
    }

    /// <summary>
    /// Makes the account the name asks for, and makes it the one the work acts as.
    /// </summary>
    private async Task Make()
    {
        if (_busy)
        {
            return;
        }

        var name = _name?.Text?.Trim() ?? string.Empty;

        if (name.Length == 0)
        {
            Say(RolaI18N.Get("launcher.account.empty"), alarm: true);

            return;
        }

        // The two names `rola account` reads as its own words cannot be an account, and a name an account
        // already has would be a key written over one that is somebody's.
        if (Accounts.Forbidden.Contains(name, StringComparer.Ordinal))
        {
            Say(RolaI18N.Get("launcher.account.forbidden", name), alarm: true);

            return;
        }

        if (_spoke.Names.Contains(name, StringComparer.Ordinal))
        {
            Say(RolaI18N.Get("launcher.account.exists", name), alarm: true);

            return;
        }

        // The question is put in a window of its own rather than in a line beside the field: making an account
        // is the one thing in this program that a Vault is told about and cannot be taken back, and it is
        // worth interrupting someone for.
        if (await new ConfirmAccountDialog(_skin, name).ShowDialog<bool>(this) != true)
        {
            return;
        }

        Busy(RolaI18N.Get("launcher.account.creating"));

        var (done, said) = await Task.Run(() => _accounts.Create(name, _files.Root));

        Busy(null);

        if (!done)
        {
            Say(RolaI18N.Get("launcher.account.failed", said), alarm: true);

            return;
        }

        Created = name;

        ShowState();
    }

    /// <summary>
    /// Saves the public half of an account's key where the person says, and says what to do with it.
    /// </summary>
    /// <param name="name">The account.</param>
    private async Task Export(string name)
    {
        if (_busy)
        {
            return;
        }

        Busy(RolaI18N.Get("launcher.account.exporting"));

        var path = await Task.Run(() => _accounts.PublicKey(name, _files.Root));

        Busy(null);

        if (path is null)
        {
            Say(RolaI18N.Get("launcher.account.no_key", name), alarm: true);

            return;
        }

        var choice = await StorageProvider.SaveFilePickerAsync(
            new FilePickerSaveOptions
            {
                Title = RolaI18N.Get("launcher.account.export"),
                SuggestedFileName = name + ".pub",
                DefaultExtension = "pub",
                ShowOverwritePrompt = true,
                FileTypeChoices =
                [
                    new FilePickerFileType(RolaI18N.Get("launcher.account.export_type"))
                    {
                        Patterns = ["*.pub"],
                    },
                ],
            }
        );

        var target = choice?.TryGetLocalPath();

        if (target is null)
        {
            return;
        }

        if (string.Equals(target, path, StringComparison.OrdinalIgnoreCase))
        {
            // Saving over the key it is saving would empty the file the copy is read from.
            return;
        }

        try
        {
            File.Copy(path, target, overwrite: true);
        }
        catch (Exception error)
            when (error is IOException or UnauthorizedAccessException or NotSupportedException)
        {
            Say(RolaI18N.Get("launcher.account.export_failed", error.Message), alarm: true);

            return;
        }

        await NoticeDialog.Show(
            this,
            _skin,
            RolaI18N.Get("launcher.account.exported"),
            RolaI18N.Get("launcher.account.exported_help"),
            RolaI18N.Get("launcher.common.ok")
        );
    }

    /// <summary>Puts a line under the window's own content, or takes it away.</summary>
    /// <param name="says">What it says.</param>
    /// <param name="alarm">Whether it is something that went wrong.</param>
    private void Say(string says, bool alarm)
    {
        _notice.Text = says;
        _notice.Foreground = alarm ? _skin.Alarm : _skin.Muted;
        _notice.IsVisible = says.Length > 0;
    }

    /// <summary>Holds the window while an action runs, or lets it go.</summary>
    /// <param name="says">What the action is doing, or nothing when it is done.</param>
    private void Busy(string? says)
    {
        _busy = says is not null;

        if (says is not null)
        {
            _lastBusy = says;
            Say(says, alarm: false);
        }
        else if (_notice.Text == _lastBusy)
        {
            Say(string.Empty, alarm: false);
        }

        if (_action is null)
        {
            return;
        }

        _action.Enabled = !_busy;
        _action.Child = Line(
            _skin,
            says ?? RolaI18N.Get("launcher.account.create")
        );

        // The label is a new control, and a button draws what it carries in the ink its kind is read in.
        _action.Repaint(_skin);
    }
}
