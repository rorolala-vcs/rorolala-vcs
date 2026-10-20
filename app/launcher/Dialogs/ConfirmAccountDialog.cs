using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using RorolalaDesktop.I18n;
using RorolalaLauncher.Controls;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Dialogs;

/// <summary>
/// The window that asks whether the name just typed is the one to be met by.
/// </summary>
/// <remarks>
/// It is a window of its own rather than a line under the field it was typed in: a name is what a Vault is
/// told, and a Vault that knows a name is hard to tell another. A sentence standing beside the field when the
/// name is typed has been read before the name exists; a window that stops the person between the two is the
/// only thing that gets read at the moment the decision is made.
/// <para>
/// The name is the largest thing on it, because the name is the thing being agreed to: what is being asked is
/// not whether to make an account, which was asked already, but whether this is the one.
/// </para>
/// </remarks>
internal sealed class ConfirmAccountDialog : DialogWindow
{
    /// <summary>Makes the window for one name.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="name">The name that was typed.</param>
    public ConfirmAccountDialog(Skin skin, string name)
        : base(skin, RolaI18N.Get("launcher.account.confirm_title"), 380, 220)
    {
        ArgumentNullException.ThrowIfNull(skin);
        ArgumentNullException.ThrowIfNull(name);

        var said = Line(skin, name, 20);

        said.FontWeight = FontWeight.Bold;

        Body.Children.Add(said);
        Body.Children.Add(Line(skin, RolaI18N.Get("launcher.account.warning"), 13, muted: true));

        var create = new FlatButton(Line(skin, RolaI18N.Get("launcher.account.create")), skin, ButtonKind.Primary);

        create.Clicked += (_, _) => Close(true);

        var cancel = new FlatButton(Line(skin, RolaI18N.Get("launcher.common.cancel")), skin);

        cancel.Clicked += (_, _) => Close(false);

        Body.Children.Add(Row(cancel, create));

        KeyDown += (_, e) =>
        {
            if (e.Key == Key.Escape)
            {
                Close(false);
            }
        };
    }
}
