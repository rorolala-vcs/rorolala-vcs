using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Presenters;
using Avalonia.Controls.Primitives;
using Avalonia.Controls.Shapes;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Styling;
using RorolalaDesktop.Theming;

namespace RorolalaDesktopHost.IntegrationTests;

/// <summary>
/// What is written on a button whose fill the user's own colour decides.
/// </summary>
/// <remarks>
/// The fill of such a button is not known until the user picks a colour, so what is written on it is not a
/// colour this theme can write down once: each state that fills with the primary names the ink that goes with
/// it, and the ink the look derives is black or white — whichever stands out from the fill — so a state that
/// forgot to name one would be words the same colour as what they are written on.
/// <para>
/// What is checked is therefore the list of states that fill with the primary and name no ink of their own.
/// A state belongs on it for a reason — its ink comes from a state it is a variation of, or the fill holds a
/// mark rather than words — and a state added later that belongs to neither is a failure with its own selector
/// in it, which is the whole of what this is for.
/// </para>
/// </remarks>
public sealed class InkTests
{
    /// <summary>Every property a fill is written to.</summary>
    private static readonly AvaloniaProperty[] Fills =
    [
        TemplatedControl.BackgroundProperty,
        Border.BackgroundProperty,
        ContentPresenter.BackgroundProperty,
        Shape.FillProperty,
    ];

    /// <summary>Every property an ink is written to.</summary>
    private static readonly AvaloniaProperty[] Inks =
    [
        TemplatedControl.ForegroundProperty,
        ContentPresenter.ForegroundProperty,
        Shape.FillProperty,
    ];

    /// <summary>
    /// The states filled with the user's primary that name no ink of their own, and why each may.
    /// </summary>
    private static readonly string[] Unnamed =
    [
        // The brighter primary a held or pointed-at button takes: the state it belongs to, and its ink, are
        // the ones it is a variation of.
        "Button:not(.dock-title):not(.dock-close):is(Button).primary:pointerover",
        "Button:not(.dock-title):not(.dock-close):is(Button).tool.on:pointerover",

        // A box that is chosen: the fill is the box and the words are beside it, and the ink inside the box is
        // the mark's, said where the mark is made.
        "CheckBox:checked /template/ #border",
        "CheckBox:indeterminate /template/ #border",
        "RadioButton:checked /template/ #border",

        // A tab being held down while it is the one shown: the fill and the ink are both the selected tab's.
        "Button.dock-title.selected:pressed",
    ];

    /// <summary>A state that fills with the primary names the ink that goes on it, or is one of the named few.</summary>
    [Fact]
    public void EveryStateFilledWithThePrimarySaysWhatIsWrittenOnIt()
    {
        var look = new RorolalaTheme(Color.Parse("#BFFF00"), Color.Parse("#FF6D00"), null);
        var unnamed = new List<string>();

        foreach (var style in look.Styles.OfType<Style>())
        {
            var setters = style.Setters.OfType<Setter>().ToArray();

            if (!States(setters, Fills, "rorolala.primary", "rorolala.primary.bright"))
            {
                continue;
            }

            if (States(setters, Inks, "rorolala.primary.text"))
            {
                continue;
            }

            unnamed.Add(style.Selector?.ToString() ?? "(no selector)");
        }

        Assert.Equal(Unnamed, unnamed);
    }

    /// <summary>Whether these setters write one of the keys to one of these properties.</summary>
    /// <param name="setters">The setters to look through.</param>
    /// <param name="properties">The properties it counts to write to.</param>
    /// <param name="keys">The resource keys it counts to write.</param>
    /// <returns>Whether one of them is written.</returns>
    private static bool States(IEnumerable<Setter> setters, AvaloniaProperty[] properties, params string[] keys) =>
        setters.Any(setter =>
            properties.Any(property => property == setter.Property)
            && setter.Value is DynamicResourceExtension resource
            && keys.Any(key => Equals(resource.ResourceKey, key))
        );
}
