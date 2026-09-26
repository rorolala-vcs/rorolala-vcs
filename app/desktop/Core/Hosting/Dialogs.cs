using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Threading;
using RorolalaDesktop.Contract;

namespace RorolalaDesktop.Hosting;

/// <summary>
/// Questions put to the user over the host's windows, drawn the way the rest of the program is.
/// </summary>
/// <remarks>
/// One instance for the whole host rather than one per plugin, because a dialog belongs to a window
/// rather than to whoever asked: the window is the host's, and which plugin wrote the words says nothing
/// about where the question is shown.
/// <para>
/// Shown on the main window and only there, so that a dock a user floated into a window of its own still
/// puts its question where the menu bar and everything else is. A plugin may ask from any thread, so the
/// showing is moved onto the loop's own.
/// </para>
/// </remarks>
internal sealed class Dialogs : IDialogs
{
    /// <summary>The window a question is shown on.</summary>
    private readonly Shell _shell;

    /// <summary>Where the two answers are written in the language the program speaks.</summary>
    private readonly I18nService _i18n;

    /// <summary>Makes the dialogs over the window they are shown on and the words they use.</summary>
    /// <param name="shell">The window a question is shown on.</param>
    /// <param name="i18n">Where the two answers are written.</param>
    public Dialogs(Shell shell, I18nService i18n)
    {
        _shell = shell;
        _i18n = i18n;
    }

    /// <inheritdoc />
    public void Show(Dialog dialog)
    {
        if (Dispatcher.UIThread.CheckAccess())
        {
            Put(dialog);

            return;
        }

        Dispatcher.UIThread.Post(() => Put(dialog));
    }

    /// <summary>Puts one question to the user.</summary>
    /// <remarks>
    /// Shown on the main window and waited for, which is what makes it modal: what it asks is about
    /// something the user is in the middle of, and letting them act on the window behind it would be
    /// acting before the question is answered.
    /// </remarks>
    /// <param name="dialog">The question.</param>
    private async void Put(Dialog dialog)
    {
        // A question with nothing to show it on is left unasked rather than answered by the host: what
        // is handed over is a consequence, and running it without the user having agreed is the one
        // thing a dialog exists to prevent. The window is up by the time a plugin asks, so this is a
        // guard rather than a path.
        if (_shell.Window is not { IsVisible: true } owner)
        {
            return;
        }

        var message = new TextBlock
        {
            Text = dialog.Message,
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(16, 16, 16, 8),
        };

        var confirm = new Button
        {
            Content = _i18n.Get("window.dialog.confirm"),
            Classes = { "primary" },
            IsDefault = true,
            MinWidth = 88,
        };

        var cancel = new Button
        {
            Content = _i18n.Get("window.dialog.cancel"),
            IsCancel = true,
            MinWidth = 88,
        };

        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            HorizontalAlignment = HorizontalAlignment.Right,
            Spacing = 8,
            Margin = new Thickness(16, 8, 16, 16),
            Children = { cancel, confirm },
        };

        var panel = new DockPanel { LastChildFill = true };
        DockPanel.SetDock(buttons, Dock.Bottom);
        panel.Children.Add(buttons);
        panel.Children.Add(message);

        var window = new Window
        {
            Title = dialog.Title,
            Width = 480,
            SizeToContent = SizeToContent.Height,
            CanResize = false,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Content = panel,
        };

        // A card of its own, so it takes the elevated ground the design puts a card on rather than the
        // ground the window behind it is drawn on — the same division as the host's notice.
        window[!Window.BackgroundProperty] = new DynamicResourceExtension("rorolala.bg.elevated");

        var confirmed = false;

        confirm.Click += (_, _) =>
        {
            confirmed = true;
            window.Close();
        };
        cancel.Click += (_, _) => window.Close();

        await window.ShowDialog(owner);

        // Run only once it is off the screen: what it does rebuilds views, and rebuilding the view that
        // has just answered would be doing it while the dialog still owns the loop.
        if (confirmed)
        {
            dialog.Confirmed();
        }
    }
}
