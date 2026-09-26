using Avalonia;
using Avalonia.Controls;
using Avalonia.Layout;
using Avalonia.Media;
using RorolalaDesktop.I18n;

namespace RorolalaFSAgent;

/// <summary>What the window was answered with.</summary>
/// <param name="Action">The answer.</param>
/// <param name="ApplyToAll">Whether the answer is to stand for every remaining conflict too.</param>
internal readonly record struct ConflictChoice(ConflictAction Action, bool ApplyToAll);

/// <summary>The answers a conflict window offers.</summary>
internal enum ConflictAction
{
    /// <summary>Remove the existing target, then run.</summary>
    Replace,

    /// <summary>Run nothing for the item.</summary>
    Skip,

    /// <summary>Run against a free name beside the existing target.</summary>
    Rename,

    /// <summary>Call off the whole run.</summary>
    Cancel,
}

/// <summary>
/// The one window a run may show: one conflict, its four answers and the "apply to the rest" box.
/// </summary>
/// <remarks>
/// The answer is handed back as a task rather than through an event, so the flow that shows the
/// window reads one conflict at a time as a straight line. Closing the window answers Cancel, which
/// is why the answer is set once and the same window cannot answer twice.
/// </remarks>
internal sealed class ConflictWindow : Window
{
    /// <summary>The one answer this window was given.</summary>
    private readonly TaskCompletionSource<ConflictChoice> _choice = new();

    /// <summary>Builds the window for one conflict.</summary>
    /// <param name="item">The conflicting item.</param>
    /// <param name="remaining">How many conflicts would still have to be asked about after this one.</param>
    public ConflictWindow(Item item, int remaining)
    {
        Title = RolaI18N.Get(Key("title"));
        Width = 520;
        SizeToContent = SizeToContent.Height;
        CanResize = false;
        WindowStartupLocation = WindowStartupLocation.CenterScreen;

        var message = new TextBlock
        {
            Text = RolaI18N.Get(Key("message"), item.From, item.To),
            TextWrapping = TextWrapping.Wrap,
        };

        var apply = new CheckBox
        {
            Content = RolaI18N.Get(Key("apply_remaining"), remaining),
            // Nothing is asked about the rest when there is no rest to ask about.
            IsVisible = remaining > 0,
            Margin = new Thickness(0, 4, 0, 0),
        };

        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            HorizontalAlignment = HorizontalAlignment.Right,
            Spacing = 8,
            Children =
            {
                Action(Key("replace"), ConflictAction.Replace, apply, isDefault: true),
                Action(Key("skip"), ConflictAction.Skip, apply),
                Action(Key("rename"), ConflictAction.Rename, apply),
                Action(Key("cancel"), ConflictAction.Cancel, apply, isCancel: true),
            },
        };

        Content = new StackPanel
        {
            Margin = new Thickness(16),
            Spacing = 12,
            Children = { message, apply, buttons },
        };

        // The window closing is an answer too: a dialog nobody filled in means nothing runs.
        Closing += (_, _) => _choice.TrySetResult(new ConflictChoice(ConflictAction.Cancel, false));
    }

    /// <summary>The answer, once the window has been answered.</summary>
    public Task<ConflictChoice> Choice => _choice.Task;

    /// <summary>Builds one answer button.</summary>
    private Button Action(
        string node,
        ConflictAction action,
        CheckBox apply,
        bool isDefault = false,
        bool isCancel = false
    )
    {
        var button = new Button
        {
            Content = RolaI18N.Get(node),
            IsDefault = isDefault,
            IsCancel = isCancel,
            MinWidth = 88,
        };

        // The action the window exists for is the one the look raises, and it is said the same way the
        // Desktop's own look says it.
        if (isDefault)
        {
            button.Classes.Add("primary");
        }

        button.Click += (_, _) =>
        {
            // The first answer wins, so a second click of a closing window changes nothing.
            if (_choice.TrySetResult(new ConflictChoice(action, apply.IsChecked == true)))
            {
                Close();
            }
        };

        return button;
    }

    /// <summary>The key a word is stored under, namespaced by this program's identity.</summary>
    private static string Key(string name) => $"rorolala_fs_agent.{name}";
}
