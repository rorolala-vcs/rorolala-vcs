using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// A command's failure, shown as the notification its report was written to become.
/// </summary>
/// <remarks>
/// The command is asked for the form a program reads — see <c>--theme-choice=desktop-friendly</c> — and what
/// comes back is read into the shapes its lines mean rather than put in a window of its own. The notification
/// stack is where anything that is said rather than asked belongs, and a failure is exactly that: it has no
/// answer to give, only something to read.
/// </remarks>
internal static class FailureNotice
{
    /// <summary>Shows what a command said about why it failed.</summary>
    /// <param name="dialogs">Where the notification is put.</param>
    /// <param name="icons">Where the marks in the report are read from.</param>
    /// <param name="title">What the failure is called.</param>
    /// <param name="said">What the command wrote, in the theme it was asked for.</param>
    public static void Show(IDialogs dialogs, IIconLibrary icons, string title, string said) =>
        dialogs.Notify(
            new Notification(
                NotificationLevel.Error,
                title,
                () =>
                    ReportView.Of(
                        ReportLines.Parse(said),
                        key => icons.Find(key),
                        ReportView.Ink
                    )
            )
        );
}
