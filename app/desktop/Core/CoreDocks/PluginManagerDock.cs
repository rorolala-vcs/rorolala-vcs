using Avalonia;
using Avalonia.Animation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Threading;
using Avalonia.VisualTree;
using RorolalaDesktop.Configuration;
using RorolalaDesktop.Contract;
using RorolalaDesktop.Hosting;
using RorolalaDesktop.Plugins;
using RorolalaDesktop.Theming;

namespace RorolalaDesktop.CoreDocks;

/// <summary>
/// The Plugin Manager: the plugins that were found, their state, and what went wrong.
/// </summary>
/// <remarks>
/// A kernel dock, always available from <c>Window</c> and not disableable. Editing here writes
/// <c>plugins.json</c>; the change takes effect on the next start, since a plugin assembly is never
/// unloaded in-process. That is also why the dock is kernel: the natural place to repair plugin
/// configuration must not itself be a plugin that could fail to load.
/// </remarks>
internal sealed class PluginManagerDock : IDockView
{
    /// <summary>The view this dock shows.</summary>
    private readonly Control _view;

    /// <summary>Makes the dock over the plugins it shows.</summary>
    /// <param name="manager">What was discovered and what went wrong.</param>
    /// <param name="i18n">The host's translations.</param>
    /// <param name="configuration">The user state that edits are written back to.</param>
    public PluginManagerDock(
        PluginManager manager,
        I18nService i18n,
        PluginsConfiguration configuration
    ) => _view = new PluginManagerView(manager, i18n, configuration);

    /// <inheritdoc />
    public Control View => _view;

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderCommand> HeaderCommands => [];
}

/// <summary>
/// The Plugin Manager's control: one draggable card per plugin, in the order they load.
/// </summary>
/// <remarks>
/// The order is the user's and the cards are where it is made. A card can be picked up and put
/// anywhere in the column; the cards around it step aside as the pointer nears a gap, and the card
/// settles where it was let go of. Nothing is decided for the user behind their back, so an order
/// that leaves a plugin unable to load is shown red on the cards it affects — while the card is
/// still in hand, and after it is dropped — rather than being silently rearranged (Section 14.2).
/// <para>
/// A press is not a drag: what separates the two is how far the pointer travels first, the way the
/// shell treats a dock's header. Only the vertical travel matters, because a card has one column to
/// be in and moving it across would be moving it nowhere.
/// </para>
/// </remarks>
internal sealed class PluginManagerView : UserControl
{
    /// <summary>How far the pointer travels before a press becomes a drag.</summary>
    private const double DragThreshold = 4.0;

    /// <summary>How long a card takes to settle, or to step aside for one being dragged.</summary>
    private static readonly TimeSpan Settle = TimeSpan.FromMilliseconds(150);

    /// <summary>The column of cards, which is also the load order.</summary>
    private readonly StackPanel _list = new() { Spacing = 8 };

    /// <summary>The cards, in the same order as the list's children.</summary>
    private readonly List<PluginCard> _cards = [];

    /// <summary>What the plugins say about themselves, and which of them were discovered.</summary>
    private readonly PluginManager _manager;

    /// <summary>The host's translations.</summary>
    private readonly I18nService _i18n;

    /// <summary>The user state edits are written back to.</summary>
    private readonly PluginsConfiguration _configuration;

    /// <summary>The card the pointer went down on, until it is let go of or the drag begins.</summary>
    private PluginCard? _pressed;

    /// <summary>Where the pointer was when the press happened, in the list's own coordinates.</summary>
    private double _from;

    /// <summary>Where the pointer was last seen, which the card in hand is kept under.</summary>
    private double _at;

    /// <summary>
    /// Where the card in hand sat in the layout when it was picked up.
    /// </summary>
    /// <remarks>
    /// The layout moves under a drag whenever something above changes its height — a name that
    /// rewraps, the pane resized, a scrollbar arriving — and a card held by the pointer must not go
    /// with it. The anchor is what lets the shift be the pointer's travel net of that movement rather
    /// than of the layout's.
    /// </remarks>
    private double _anchor;

    /// <summary>The place the pressed card started in.</summary>
    private int _source;

    /// <summary>The place the card in hand would land, which is where it is let go of.</summary>
    private int _target;

    /// <summary>
    /// Whether a card is being settled into place, so that losing the capture is not a cancel.
    /// </summary>
    private bool _settling;

    /// <summary>The timer that finishes a settle once the animation has run its course.</summary>
    private DispatcherTimer? _settle;

    /// <summary>Makes the control over the plugins it shows.</summary>
    /// <param name="manager">What was discovered and what went wrong.</param>
    /// <param name="i18n">The host's translations.</param>
    /// <param name="configuration">The user state that edits are written back to.</param>
    public PluginManagerView(
        PluginManager manager,
        I18nService i18n,
        PluginsConfiguration configuration
    )
    {
        _manager = manager;
        _i18n = i18n;
        _configuration = configuration;

        var byId = manager.Discovered.ToDictionary(plugin => plugin.Id);

        foreach (var state in configuration.Plugins)
        {
            if (byId.TryGetValue(state.Id, out var plugin))
            {
                _cards.Add(Card(plugin.Id, i18n.Get(plugin.Manifest.DisplayNameKey)));
            }
        }

        // The configuration holds every discovered plugin once it has been reconciled, so this only
        // catches a plugin the manager knows and the file somehow does not. Showing it at the end is
        // better than hiding a plugin the user installed.
        foreach (var plugin in manager.Discovered)
        {
            if (!configuration.Contains(plugin.Id))
            {
                _cards.Add(Card(plugin.Id, i18n.Get(plugin.Manifest.DisplayNameKey)));
            }
        }

        foreach (var card in _cards)
        {
            _list.Children.Add(card.Root);
        }

        // The layout can move under a drag — a name rewrapping, the pane resized — and the room made
        // for the card in hand, and the card itself, have to stay right as it does.
        _list.LayoutUpdated += (_, _) => Regap();

        var panel = new StackPanel { Spacing = 24, Margin = new Thickness(16, 16, 24, 60) };

        panel.Children.Add(Section(i18n.Get("plugin_manager.plugins"), _list));

        var problems = manager.Problems.ToArray();

        if (problems.Length > 0)
        {
            var notes = new StackPanel { Spacing = 8 };
            var state = i18n.Get("plugin_manager.not_loaded");

            for (var i = 0; i < problems.Length; i++)
            {
                // The rule sits above every note but the first, which is already under the heading's own.
                if (i > 0)
                {
                    notes.Children.Add(Divider());
                }

                notes.Children.Add(Note(problems[i].Subject, problems[i].Description, state));
            }

            panel.Children.Add(Section(i18n.Get("plugin_manager.problems"), notes));
        }

        panel.Children.Add(
            new TextBlock
            {
                Text = i18n.Get("plugin_manager.next_start"),
                TextWrapping = TextWrapping.Wrap,
                Classes = { "caption", "faint" },
            }
        );

        Content = new ScrollViewer { Content = panel };

        Refresh(Current());
    }

    /// <summary>A section: its heading, the rule under it, and what it holds.</summary>
    private static Control Section(string heading, Control content)
    {
        var rule = Divider();
        rule.Margin = new Thickness(0, 0, 0, 8);

        return new StackPanel
        {
            Children =
            {
                // The design sets a heading in capitals with `text-transform`, which Avalonia does
                // not have, so the capitals are put on the word here instead.
                new TextBlock
                {
                    Text = heading.ToUpperInvariant(),
                    Classes = { "label" },
                    Margin = new Thickness(0, 0, 0, 4),
                },
                rule,
                content,
            },
        };
    }

    /// <summary>The hairline separating a heading from what follows it, in the theme's border colour.</summary>
    private static Border Divider() =>
        new()
        {
            BorderThickness = new Thickness(0, 0, 0, 1),
            [!Border.BorderBrushProperty] = new DynamicResourceExtension("rorolala.border"),
        };

    /// <summary>One problem: what it is about and what is wrong, with the state it leaves at the right.</summary>
    private static Control Note(string subject, string description, string state)
    {
        var row = new Grid { ColumnDefinitions = new ColumnDefinitions("*,Auto") };

        row.Children.Add(
            new StackPanel
            {
                Spacing = 4,
                Children =
                {
                    new TextBlock
                    {
                        Text = subject,
                        TextWrapping = TextWrapping.Wrap,
                        Classes = { "mono" },
                    },
                    new TextBlock
                    {
                        Text = description,
                        TextWrapping = TextWrapping.Wrap,
                        Classes = { "caption", "faint" },
                    },
                },
            }
        );

        var tag = StateTag(state);
        Grid.SetColumn(tag, 1);
        row.Children.Add(tag);

        return row;
    }

    /// <summary>The state a problem leaves a plugin in, as a small tag.</summary>
    private static Control StateTag(string state)
    {
        var text = new TextBlock
        {
            Text = state,
            Classes = { "caption" },
            VerticalAlignment = VerticalAlignment.Center,
        };
        text[!TextBlock.ForegroundProperty] = new DynamicResourceExtension(RorolalaTheme.DelInk);

        return new Border
        {
            CornerRadius = new CornerRadius(5),
            BorderThickness = new Thickness(1),
            Padding = new Thickness(8, 2),
            VerticalAlignment = VerticalAlignment.Top,
            [!Border.BackgroundProperty] = new DynamicResourceExtension(
                RorolalaTheme.DelBackground
            ),
            [!Border.BorderBrushProperty] = new DynamicResourceExtension(RorolalaTheme.DelInk),
            Child = text,
        };
    }

    /// <summary>Puts a control in a grid column, centred vertically.</summary>
    private static Control Place(Control control, int column)
    {
        control.VerticalAlignment = VerticalAlignment.Center;
        control.Margin = new Thickness(12, 0, 0, 0);

        Grid.SetColumn(control, column);

        return control;
    }

    /// <summary>Whether a pointer landed on a control inside the card rather than on the card.</summary>
    /// <param name="source">What the pointer event says it came from.</param>
    /// <returns>Whether one of the card's own controls is on the way up from it.</returns>
    private static bool FromInner(object? source)
    {
        for (var node = source as Visual; node is not null; node = node.GetVisualParent())
        {
            if (node is CheckBox or Button)
            {
                return true;
            }
        }

        return false;
    }

    /// <summary>
    /// Paints one card's wash: the accent while the pointer is on it or it is in hand, the red when
    /// it cannot load.
    /// </summary>
    /// <remarks>
    /// The red wins over the accent, because an error a hover could hide is an error the user loses
    /// the moment they reach for the card that has it.
    /// </remarks>
    /// <param name="card">The card to paint.</param>
    private static void Wash(PluginCard card)
    {
        var faulted = card.Root.Classes.Contains(PluginCard.ErrorClass);
        var dragging = card.Root.Classes.Contains(PluginCard.DraggingClass);

        card.Wash[!Border.BackgroundProperty] = new DynamicResourceExtension(
            faulted
                ? RorolalaTheme.DelWash
                : dragging
                    ? RorolalaTheme.AccentBrightWash
                    : RorolalaTheme.AccentWash
        );

        card.Wash.Opacity = faulted || dragging || card.Hovered ? 1.0 : 0.0;
    }

    /// <summary>Marks a card as the one in hand and lets it follow the pointer without lag.</summary>
    /// <param name="card">The card being dragged.</param>
    private static void Lift(PluginCard card)
    {
        card.Root.Classes.Add(PluginCard.DraggingClass);
        card.Root.ZIndex = 1;

        // Following the pointer is the one movement that must not be animated: a transition here
        // would leave the card trailing where the pointer has already been.
        card.Shift.Transitions = null;

        Wash(card);
    }

    /// <summary>One plugin's card: its name and identity, its switch, and why it cannot load.</summary>
    /// <param name="id">The plugin's identity.</param>
    /// <param name="name">The plugin's display name, already translated.</param>
    private PluginCard Card(PluginId id, string name)
    {
        var identity = new StackPanel
        {
            Spacing = 4,
            VerticalAlignment = VerticalAlignment.Center,
            Children =
            {
                // One line, always: a name that wrapped would make the card taller on a narrow pane
                // or when the mark beside it took room, and the column's height is what makes the
                // order readable.
                new TextBlock
                {
                    Text = name,
                    TextWrapping = TextWrapping.NoWrap,
                    TextTrimming = TextTrimming.CharacterEllipsis,
                },
                new TextBlock { Text = id.Value, Classes = { "mono", "faint" } },
            },
        };

        // The switch is set before it is listened to, so that building the card does not look like
        // the user changing it and write the file back unchanged.
        var toggle = new CheckBox { IsChecked = _configuration.Enabled(id) };
        toggle.IsCheckedChanged += (_, _) =>
        {
            Store();
            Refresh(Current());
        };

        // What stops the plugin is a mark beside the switch rather than a line under the name: a
        // card that grew a line whenever it turned red would move every card under it, and the
        // column's order is the thing being read. The mark says only that there is something to
        // know; the words for it are on the mark, where a hover finds them.
        var details = new TextBlock
        {
            TextWrapping = TextWrapping.Wrap,
            MaxWidth = 320,
            Classes = { "caption", "plugin-fault" },
        };

        var error = new Button
        {
            Content = _i18n.Get("plugin_manager.error"),
            Classes = { PluginCard.ErrorBadgeClass },
            IsVisible = false,
            VerticalAlignment = VerticalAlignment.Center,
        };

        ToolTip.SetTip(error, details);

        // The card's edge belongs to the surface and its wash, which fills it; the words and the
        // switch are set 16 in from that edge so a card reads as a card rather than as a line of text
        // inside a box.
        var content = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("*,Auto,Auto"),
            Margin = new Thickness(16),
        };
        content.Children.Add(identity);
        content.Children.Add(Place(error, 1));
        content.Children.Add(Place(toggle, 2));

        var wash = new Border
        {
            Classes = { PluginCard.WashClass },
            IsHitTestVisible = false,
        };

        var root = new Border
        {
            Classes = { PluginCard.CardClass },
            Child = new Panel { Children = { wash, content } },
        };

        var card = new PluginCard(id, root, toggle, error, details, wash);

        root.PointerPressed += (_, e) => Press(card, e);
        root.PointerMoved += (_, e) => Move(card, e);
        root.PointerReleased += (_, e) => Release(card, e);
        root.PointerCaptureLost += (_, _) => Cancel();

        // A card's own layout is the surest sign it was laid out again — text reflow is per card —
        // and the list's is there for a change no single card accounts for.
        root.LayoutUpdated += (_, _) => Regap();
        root.PointerEntered += (_, _) =>
        {
            card.Hovered = true;
            Wash(card);
        };
        root.PointerExited += (_, _) =>
        {
            card.Hovered = false;
            Wash(card);
        };

        return card;
    }

    /// <summary>The user's order as the cards now stand, optionally with one card moved.</summary>
    /// <param name="from">Where the card in hand started, or nothing.</param>
    /// <param name="to">Where it would be put, or nothing.</param>
    /// <returns>One state per card, in the order it would leave.</returns>
    private IReadOnlyList<PluginState> Current(int? from = null, int? to = null)
    {
        var order = _cards
            .Select(card => new PluginState(card.Id, card.Toggle.IsChecked == true))
            .ToList();

        if (from is { } source && to is { } target && source != target)
        {
            var moved = order[source];
            order.RemoveAt(source);
            order.Insert(target, moved);
        }

        return order;
    }

    /// <summary>Writes the order and the switches back to the file.</summary>
    private void Store()
    {
        _configuration.Plugins.Clear();

        foreach (var state in Current())
        {
            _configuration.Plugins.Add(state);
        }

        ConfigurationLoader.WritePlugins(_configuration);
    }

    /// <summary>
    /// Says on every card whether the order leaves it unable to load, and why.
    /// </summary>
    /// <param name="order">The order to answer for, which may be one still being dragged.</param>
    private void Refresh(IReadOnlyList<PluginState> order)
    {
        var byPlugin = _manager
            .Faults(order)
            .GroupBy(fault => fault.Plugin)
            .ToDictionary(group => group.Key, group => group.ToArray());

        foreach (var card in _cards)
        {
            var text = byPlugin.TryGetValue(card.Id, out var faults)
                ? string.Join("\n", faults.Select(FaultText))
                : null;

            card.Details.Text = text ?? string.Empty;
            card.Error.IsVisible = text is not null;
            card.Root.Classes.Set(PluginCard.ErrorClass, text is not null);

            Wash(card);
        }
    }

    /// <summary>What one fault says, in the language the program speaks.</summary>
    /// <param name="fault">The fault to put into words.</param>
    /// <returns>The sentence the card shows.</returns>
    private string FaultText(PluginFault fault) =>
        _i18n.Get(
            fault.Kind switch
            {
                PluginFaultKind.DependencyMissing => "plugin_manager.fault.missing",
                PluginFaultKind.DependencyDisabled => "plugin_manager.fault.disabled",
                PluginFaultKind.DependencyLater => "plugin_manager.fault.later",
                _ => "plugin_manager.fault.unavailable",
            },
            fault.Dependency.Value
        );

    /// <summary>What the pointer does when it goes down on a card.</summary>
    /// <param name="card">The card it landed on.</param>
    /// <param name="e">The press.</param>
    private void Press(PluginCard card, PointerPressedEventArgs e)
    {
        if (_settle is not null || FromInner(e.Source))
        {
            return;
        }

        _pressed = card;
        _source = _cards.IndexOf(card);
        _target = _source;
        _from = e.GetPosition(_list).Y;
        _at = _from;
        _anchor = card.Root.Bounds.Top;

        e.Pointer.Capture(card.Root);
    }

    /// <summary>Lifts the card once the pointer has travelled far enough, and keeps it under it.</summary>
    /// <param name="card">The card the press landed on.</param>
    /// <param name="e">The move.</param>
    private void Move(PluginCard card, PointerEventArgs e)
    {
        if (_pressed != card || _settle is not null)
        {
            return;
        }

        var at = e.GetPosition(_list).Y;

        if (!card.Root.Classes.Contains(PluginCard.DraggingClass))
        {
            if (Math.Abs(at - _from) < DragThreshold)
            {
                return;
            }

            Lift(card);
        }

        // Only the vertical travel moves the card: the column has one place per plugin, so carrying
        // a card across would be carrying it nowhere.
        _at = at;
        card.Shift.Y = Follow(card, at);

        var target = Target(at);

        if (target == _target)
        {
            return;
        }

        _target = target;
        Reserve(card);
        Refresh(Current(_source, _target));
    }

    /// <summary>Where the card in hand would land, judged by where the pointer has reached.</summary>
    /// <param name="at">Where the pointer is, in the list's coordinates.</param>
    /// <returns>The index the card would take.</returns>
    private int Target(double at)
    {
        var target = _source;

        if (at < _cards[_source].Root.Bounds.Center.Y)
        {
            for (var index = _source - 1; index >= 0; index--)
            {
                if (at < _cards[index].Root.Bounds.Center.Y)
                {
                    target = index;
                }
            }

            return target;
        }

        for (var index = _source + 1; index < _cards.Count; index++)
        {
            if (at > _cards[index].Root.Bounds.Center.Y)
            {
                target = index;
            }
        }

        return target;
    }

    /// <summary>Steps the cards around the one in hand aside, so its place is already open.</summary>
    /// <remarks>
    /// The shift is the height of the card in hand plus the gap it leaves behind, because that is
    /// exactly the space one card takes: closing it at the old place and opening it at the new one
    /// moves every card between the two by that much and no more.
    /// </remarks>
    /// <param name="dragged">The card in hand.</param>
    private void Reserve(PluginCard dragged)
    {
        var span = dragged.Root.Bounds.Height + _list.Spacing;

        for (var index = 0; index < _cards.Count; index++)
        {
            if (index == _source)
            {
                continue;
            }

            var offset = _target > _source && index > _source && index <= _target
                ? -span
                : _target < _source && index >= _target && index < _source
                    ? span
                    : 0;

            _cards[index].Shift.Y = offset;
        }
    }

    /// <summary>Puts the room made for the card in hand back in step with the layout.</summary>
    /// <remarks>
    /// The offsets that clear a place for the card in hand are measured against the layout, and the
    /// layout can move under a drag — a name that rewraps, the pane resized, a scrollbar arriving.
    /// The layout pass is the only place that is known, so the offsets are answered for there rather
    /// than only when the target changed.
    /// <para>
    /// They are set without a transition, because the movement they answer to had none. Animating
    /// them would let the layout shift the cards first and the animation pull them back, which is the
    /// correction the eye reads as a card jumping.
    /// </para>
    /// </remarks>
    private void Regap()
    {
        var dragged = _pressed;

        if (
            dragged is null
            || _settle is not null
            || !dragged.Root.Classes.Contains(PluginCard.DraggingClass)
        )
        {
            return;
        }

        foreach (var card in _cards)
        {
            card.Shift.Transitions = null;
        }

        Reserve(dragged);

        // The card in hand is put back under the pointer, since the layout may have moved it while
        // it was being held.
        dragged.Shift.Y = Follow(dragged, _at);

        foreach (var card in _cards)
        {
            card.Shift.Transitions = card.Slide;
        }
    }

    /// <summary>What the pointer does when it is let go of: the card settles where it was put.</summary>
    /// <param name="card">The card the press landed on.</param>
    /// <param name="e">The release.</param>
    private void Release(PluginCard card, PointerReleasedEventArgs e)
    {
        if (_pressed != card || _settle is not null)
        {
            return;
        }

        var lifted = card.Root.Classes.Contains(PluginCard.DraggingClass);
        var source = _source;
        var target = _target;

        _pressed = null;

        if (!lifted)
        {
            // A press that never moved is a press on the card and nothing else.
            e.Pointer.Capture(null);
            _target = source;

            return;
        }

        // Losing the capture is part of settling rather than a cancel, so the flag is up before the
        // capture is given back.
        _settling = true;
        e.Pointer.Capture(null);

        card.Shift.Transitions = card.Slide;
        card.Shift.Y = Settles(card);

        // Every other card is left exactly where the drag put it. It is already standing in the gap
        // it made room for, so there is nowhere for it to go: sending it back to where it came from
        // and letting the move in `Commit` put it right again is two movements where the user asked
        // for none.
        _settle = new DispatcherTimer { Interval = Settle };
        _settle.Tick += (_, _) => Commit(card, source, target);
        _settle.Start();
    }

    /// <summary>
    /// How far the card in hand stands from where the layout now puts it, so that the pointer keeps
    /// holding it where it was picked up.
    /// </summary>
    /// <remarks>
    /// The shift is the pointer's travel less however far the layout has moved the card since it was
    /// picked up: a card that grew above it pushes it down in the layout, and the pointer did not ask
    /// for that.
    /// </remarks>
    /// <param name="card">The card in hand.</param>
    /// <param name="at">Where the pointer is, in the list's coordinates.</param>
    /// <returns>The vertical shift to hold it with.</returns>
    private double Follow(PluginCard card, double at) =>
        at - _from - (card.Root.Bounds.Top - _anchor);

    /// <summary>How far the card in hand travels to reach the place it was let go of in.</summary>
    /// <param name="card">The card in hand.</param>
    /// <returns>The vertical shift, in the list's coordinates.</returns>
    private double Settles(PluginCard card)
    {
        if (_target == _source)
        {
            return 0;
        }

        var dragged = _cards[_source].Root.Bounds;
        var slot = _cards[_target].Root.Bounds;

        // The place a card lands in is the one the card there occupied, pulled up by the space this
        // one is taller or shorter by, so that cards of different heights still meet the gap.
        var top = _target > _source
            ? slot.Top + slot.Height - dragged.Height
            : slot.Top;

        return top - dragged.Top;
    }

    /// <summary>Puts the card in its place once it has visibly arrived there.</summary>
    /// <remarks>
    /// The move is made only after the settle has run, because the layout is what the transform was
    /// standing in for: doing it earlier would move the card twice, once by the layout and once by
    /// the transform still on it. By the time the move happens the two agree, so there is nothing to
    /// see.
    /// </remarks>
    /// <param name="card">The card that was in hand.</param>
    /// <param name="source">Where it started.</param>
    /// <param name="target">Where it was put.</param>
    private void Commit(PluginCard card, int source, int target)
    {
        Stop();
        _settling = false;

        if (source != target)
        {
            _list.Children.Move(source, target);
            _cards.RemoveAt(source);
            _cards.Insert(target, card);
            Store();
        }

        foreach (var other in _cards)
        {
            other.Shift.Transitions = null;
            other.Shift.Y = 0;
            other.Root.Classes.Remove(PluginCard.DraggingClass);
            other.Root.ZIndex = 0;
            other.Shift.Transitions = other.Slide;
        }

        _target = target;

        Refresh(Current());
    }

    /// <summary>Gives up a drag that was ended by something other than letting the card go.</summary>
    private void Cancel()
    {
        if (_settling)
        {
            return;
        }

        Stop();

        _pressed = null;
        _target = _source;

        foreach (var card in _cards)
        {
            card.Shift.Transitions = null;
            card.Shift.Y = 0;
            card.Root.Classes.Remove(PluginCard.DraggingClass);
            card.Root.ZIndex = 0;
            card.Shift.Transitions = card.Slide;
        }

        Refresh(Current());
    }

    /// <summary>Ends the settle timer, if one is running.</summary>
    private void Stop()
    {
        if (_settle is null)
        {
            return;
        }

        _settle.Stop();
        _settle = null;
    }
}

/// <summary>
/// One plugin's card: the surface that can be picked up, and the parts that show it.
/// </summary>
/// <remarks>
/// The class names are here rather than in the look, so that the look addresses a card by what the
/// card says it is — the same way a dock says which of its parts is which.
/// </remarks>
internal sealed class PluginCard
{
    /// <summary>The class a plugin's card wears.</summary>
    public const string CardClass = "plugin-card";

    /// <summary>The class the wash laid over a card wears.</summary>
    public const string WashClass = "plugin-card-wash";

    /// <summary>The class a card wears while it is the one in hand.</summary>
    public const string DraggingClass = "plugin-dragging";

    /// <summary>The class a card wears when the order leaves it unable to load.</summary>
    public const string ErrorClass = "plugin-error";

    /// <summary>The class the mark a faulted card carries wears.</summary>
    public const string ErrorBadgeClass = "plugin-error-badge";

    /// <summary>Makes a card for one plugin, over the parts that show it.</summary>
    /// <param name="id">The plugin's identity.</param>
    /// <param name="root">The surface the pointer picks up.</param>
    /// <param name="toggle">The switch that says whether the plugin loads.</param>
    /// <param name="error">The mark that says it cannot, whose tip carries the words for it.</param>
    /// <param name="details">The words a hover on the mark shows.</param>
    /// <param name="wash">The overlay the pointer and the drag are drawn with.</param>
    public PluginCard(
        PluginId id,
        Border root,
        CheckBox toggle,
        Button error,
        TextBlock details,
        Border wash
    )
    {
        Id = id;
        Root = root;
        Toggle = toggle;
        Error = error;
        Details = details;
        Wash = wash;

        // The shift is what the drag moves and what makes room; a card's own is not the column's, so
        // that one card following the pointer does not animate every other one with it.
        Slide = new Transitions
        {
            new DoubleTransition
            {
                Property = TranslateTransform.YProperty,
                Duration = TimeSpan.FromMilliseconds(150),
            },
        };

        Shift = new TranslateTransform { Transitions = Slide };
        Root.RenderTransform = Shift;
    }

    /// <summary>The plugin this card is for.</summary>
    public PluginId Id { get; }

    /// <summary>The surface the pointer picks up, which is the card as the look draws it.</summary>
    public Border Root { get; }

    /// <summary>The switch that says whether the plugin loads.</summary>
    public CheckBox Toggle { get; }

    /// <summary>The mark that says the plugin cannot load.</summary>
    public Button Error { get; }

    /// <summary>The words a hover on the mark shows.</summary>
    public TextBlock Details { get; }

    /// <summary>The overlay the pointer and the drag are drawn with.</summary>
    public Border Wash { get; }

    /// <summary>The card's vertical shift, which is the only direction it moves.</summary>
    public TranslateTransform Shift { get; }

    /// <summary>The transitions the shift is animated with, kept to be put back after a drag.</summary>
    public Transitions Slide { get; }

    /// <summary>Whether the pointer is on this card, which the wash answers to.</summary>
    public bool Hovered { get; set; }
}
