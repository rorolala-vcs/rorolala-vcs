using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Styling;
using Avalonia.Threading;

namespace FileSystemPlugin;

/// <summary>
/// The bubble a name is typed in, under the name it belongs to.
/// </summary>
/// <remarks>
/// A row is not rebuilt around a box: the row stays the row it is, and the field is a popup under the name.
/// That keeps the listing's own layout out of the edit, and the bubble is where the keyboard is, so the
/// dock's other keys are not read while a name is being typed.
/// <para>
/// Enter takes the name and Escape leaves it; both close the bubble. A press anywhere else leaves the field,
/// which closes it without taking the name, the same answer as Escape.
/// </para>
/// </remarks>
internal sealed class RenameBubble
{
    /// <summary>How wide the bubble is, which is wider than a name and narrower than a dock.</summary>
    private const double Width_ = 260;

    /// <summary>What the field is, and what the popup holds it.</summary>
    private readonly TextBox _box;

    /// <summary>The flyout under the name, which is what puts the field in a window of its own.</summary>
    private readonly Flyout _flyout;

    /// <summary>What is done with the name that was typed.</summary>
    private readonly Action<string> _commit;

    /// <summary>What is done when the name is left as it was.</summary>
    private readonly Action _cancel;

    /// <summary>Whether this bubble has already said which of the two it is.</summary>
    private bool _done;

    /// <summary>The bubble on screen, of which there is at most one.</summary>
    private static RenameBubble? _current;

    /// <summary>The bubble on screen, or nothing while no name is being typed.</summary>
    internal static RenameBubble? Current => _current;

    /// <summary>The field a name is typed in, which a test reads the way the eye reads the bubble.</summary>
    internal TextBox Field => _box;

    /// <summary>Makes a bubble under a name, and puts the keyboard in it.</summary>
    /// <param name="anchor">The name the bubble stands under.</param>
    /// <param name="name">The name the field starts with.</param>
    /// <param name="selected">How much of that name is chosen, from the start.</param>
    /// <param name="commit">What to do with the typed name.</param>
    /// <param name="cancel">What to do when the edit is left.</param>
    public RenameBubble(
        Control anchor,
        string name,
        int selected,
        Action<string> commit,
        Action cancel
    )
    {
        ArgumentNullException.ThrowIfNull(anchor);
        ArgumentNullException.ThrowIfNull(commit);
        ArgumentNullException.ThrowIfNull(cancel);

        _commit = commit;
        _cancel = cancel;
        _current = this;

        _box = new TextBox
        {
            Text = name,
            Width = Width_,
            MinHeight = 34,
            Padding = new Thickness(12, 0),
            BorderThickness = new Thickness(0),
            CornerRadius = new CornerRadius(0),
            Background = Brushes.Transparent,
            HorizontalContentAlignment = HorizontalAlignment.Left,
            VerticalContentAlignment = VerticalAlignment.Center,
            TextAlignment = TextAlignment.Left,
        };

        // The look draws a ring and a border on a focused field. This field is the bubble itself, so
        // both are taken off here: the popup presenter is the one frame, and a second one inside it
        // reads as a field in a field rather than as a name in a bubble.
        _box.Styles.Add(
            new Style(selector => selector.Class(":focus").Template().Name("border"))
            {
                Setters =
                {
                    new Setter(Border.BorderThicknessProperty, new Thickness(0)),
                    new Setter(Border.BoxShadowProperty, default(BoxShadows)),
                },
            }
        );

        // The flyout's own presenter is the one frame the bubble wears. The field has no border or ground
        // of its own, and its padding is the room a reader needs on either side of the name.
        _flyout = new Flyout
        {
            Placement = PlacementMode.Bottom,
            Content = _box,
        };

        // Taken on the way down, before the field reads them: a field that took Enter for itself would be a
        // name nobody could finish. Every other key is the field's, which is what makes a shortcut elsewhere
        // in the dock wait while a name is being typed.
        _box.AddHandler(InputElement.KeyDownEvent, Keyed, RoutingStrategies.Tunnel);
        _box.LostFocus += (_, _) => Cancel();

        _flyout.ShowAt(anchor);

        // The field is focused after the flyout has made its own window, not while it is being shown: a field
        // focused before there is a window to hold it is a field the keyboard never reaches.
        Dispatcher.UIThread.Post(
            () =>
            {
                if (_done)
                {
                    return;
                }

                _box.Focus();
                _box.SelectionStart = 0;
                _box.SelectionEnd = Math.Clamp(selected, 0, name.Length);
            },
            DispatcherPriority.Loaded
        );
    }

    /// <summary>Whether the bubble is still up.</summary>
    public bool IsOpen => !_done;

    /// <summary>Takes the bubble down without saying anything about the name.</summary>
    public void Dismiss()
    {
        if (_done)
        {
            return;
        }

        _done = true;
        _flyout.Hide();

        if (ReferenceEquals(_current, this))
        {
            _current = null;
        }
    }

    /// <summary>Reads the two keys the bubble answers.</summary>
    /// <param name="sender">The field.</param>
    /// <param name="e">The key.</param>
    private void Keyed(object? sender, KeyEventArgs e)
    {
        if (e.Key == Key.Enter)
        {
            e.Handled = true;
            Commit();

            return;
        }

        if (e.Key == Key.Escape)
        {
            e.Handled = true;
            Cancel();
        }
    }

    /// <summary>Takes what the field says as the name and closes the bubble.</summary>
    private void Commit()
    {
        if (_done)
        {
            return;
        }

        var text = _box.Text ?? string.Empty;

        _done = true;
        _flyout.Hide();

        if (ReferenceEquals(_current, this))
        {
            _current = null;
        }

        _commit(text);
    }

    /// <summary>Closes the bubble and leaves the name as it was.</summary>
    private void Cancel()
    {
        if (_done)
        {
            return;
        }

        _done = true;
        _flyout.Hide();

        if (ReferenceEquals(_current, this))
        {
            _current = null;
        }

        _cancel();
    }
}
