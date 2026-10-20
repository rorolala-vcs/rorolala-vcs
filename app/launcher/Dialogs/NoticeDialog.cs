using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaLauncher.Controls;
using RorolalaLauncher.Theming;

namespace RorolalaLauncher.Dialogs;

/// <summary>
/// What the launcher says to the person using it when there is nothing to fill in.
/// </summary>
/// <remarks>
/// A notice is one thing said and one way on; a question is the same with two ways, one of which is taking
/// back. Both are modal over the launcher, so what is said is read before anything else is done.
/// </remarks>
internal sealed class NoticeDialog : DialogWindow
{
    /// <summary>Which way the person went.</summary>
    private bool _answered;

    /// <summary>Makes a notice or a question.</summary>
    /// <param name="skin">The colours in force.</param>
    /// <param name="heading">What the window is about.</param>
    /// <param name="message">What it says.</param>
    /// <param name="yes">The way on, or nothing for a notice.</param>
    /// <param name="no">The way back, when there is one.</param>
    private NoticeDialog(Skin skin, string heading, string message, string? yes, string? no)
        : base(skin, heading, 460, 220)
    {
        // What a notice says is a sentence sometimes and a reason from another program sometimes, so the
        // window is as tall as what it was handed rather than as tall as a guess: a reason three lines
        // longer than the window would be a reason nobody can read.
        Height = double.NaN;
        SizeToContent = SizeToContent.Height;

        Body.Children.Add(Line(skin, message));

        var on = new FlatButton(Line(skin, yes ?? heading, 13), skin, ButtonKind.Primary);

        on.Clicked += (_, _) =>
        {
            _answered = true;
            Close();
        };

        if (no is null)
        {
            Body.Children.Add(Row(on));
            return;
        }

        var back = new FlatButton(Line(skin, no, 13), skin);

        back.Clicked += (_, _) => Close();

        Body.Children.Add(Row(back, on));
    }

    /// <summary>Says one thing.</summary>
    /// <param name="owner">What it is said over.</param>
    /// <param name="skin">The colours in force.</param>
    /// <param name="heading">What the window is about.</param>
    /// <param name="message">What it says.</param>
    /// <param name="on">What the one button says.</param>
    /// <returns>A task that ends when it is read.</returns>
    public static Task Show(
        Window owner,
        Skin skin,
        string heading,
        string message,
        string on
    )
    {
        var dialog = new NoticeDialog(skin, heading, message, on, null);

        return dialog.ShowDialog(owner);
    }

    /// <summary>Asks one thing, and answers how it was answered.</summary>
    /// <param name="owner">What it is asked over.</param>
    /// <param name="skin">The colours in force.</param>
    /// <param name="heading">What the window is about.</param>
    /// <param name="message">What it asks.</param>
    /// <param name="on">The way on.</param>
    /// <param name="back">The way back.</param>
    /// <returns>Whether the way on was taken.</returns>
    public static async Task<bool> Ask(
        Window owner,
        Skin skin,
        string heading,
        string message,
        string on,
        string back
    )
    {
        var dialog = new NoticeDialog(skin, heading, message, on, back);

        await dialog.ShowDialog(owner);

        return dialog._answered;
    }
}
