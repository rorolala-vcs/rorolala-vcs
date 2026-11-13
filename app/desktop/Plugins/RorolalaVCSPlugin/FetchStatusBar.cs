using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Shapes;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using RorolalaDesktop.Contract;

namespace RorolalaVCSPlugin;

/// <summary>
/// The line under the Overview that says which Vault the work is fetched from, and how the last fetch went.
/// </summary>
/// <remarks>
/// The lamp is the whole of the state: yellow while a fetch is going, green when the copy was last brought over,
/// and red when it could not be. The name is the Vault's own and is written in capitals, since it is a name the
/// Workspace bound rather than a sentence, and the address follows it so that a Vault reached at another address
/// is told from one reached here.
/// </remarks>
internal sealed class FetchStatusBar : Border
{
    /// <summary>The lamp whose colour is the state of the last fetch.</summary>
    private readonly Ellipse _lamp = new()
    {
        Width = 8,
        Height = 8,
        VerticalAlignment = VerticalAlignment.Center,
    };

    /// <summary>The rule between the lamp and the Vault's name.</summary>
    private readonly TextBlock _rule = new()
    {
        Text = "|",
        VerticalAlignment = VerticalAlignment.Center,
        Classes = { "muted" },
    };

    /// <summary>The Vault's name, in the capitals a bound name is read in.</summary>
    private readonly TextBlock _name = new()
    {
        FontWeight = FontWeight.Bold,
        VerticalAlignment = VerticalAlignment.Center,
    };

    /// <summary>The address the Vault answers at.</summary>
    private readonly TextBlock _address = new()
    {
        VerticalAlignment = VerticalAlignment.Center,
        Classes = { "muted" },
    };

    /// <summary>Makes the status bar.</summary>
    public FetchStatusBar()
    {
        Height = 35;
        BorderThickness = new Thickness(0, 1, 0, 0);
        this[!BackgroundProperty] = new DynamicResourceExtension(PaletteKeys.Dark);
        this[!BorderBrushProperty] = new DynamicResourceExtension(PaletteKeys.Splitter);

        var row = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(12, 0),
            Children = { _lamp, _rule, _name, _address },
        };

        Child = row;
        Show(FetchState.Never, string.Empty, string.Empty);
    }

    /// <summary>Shows what the last fetch came to, and which Vault it was for.</summary>
    /// <param name="state">What the last fetch came to.</param>
    /// <param name="name">The Vault's name, empty when the work names none.</param>
    /// <param name="address">The address the Vault answers at, empty when it is not known.</param>
    public void Show(FetchState state, string name, string address)
    {
        _lamp[!Shape.FillProperty] = new DynamicResourceExtension(
            state switch
            {
                FetchState.Ready => PaletteKeys.Success,
                FetchState.Failed => PaletteKeys.Error,
                _ => PaletteKeys.Warning,
            }
        );

        _name.Text = name.ToUpperInvariant();
        _address.Text = address;

        var named = name.Length > 0;

        _rule.IsVisible = named;
        _name.IsVisible = named;
        _address.IsVisible = named && address.Length > 0;
    }
}
