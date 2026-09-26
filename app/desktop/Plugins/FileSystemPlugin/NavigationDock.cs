using System.IO;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace FileSystemPlugin;

/// <summary>The File System navigation dock: the toolbar, as a dock of its own.</summary>
/// <remarks>
/// It is a dock rather than a part of the browser so that it can be placed where the user wants it —
/// along the top by default — and so that a second browser does not bring a second toolbar.
/// </remarks>
internal sealed class NavigationDock : IDockView
{
    /// <summary>Makes the navigation dock.</summary>
    /// <param name="host">The host, for logging what navigation cannot do.</param>
    /// <param name="browser">The location, which this dock shows and switches.</param>
    public NavigationDock(IPluginHost host, Browser browser) =>
        View = new NavigationControl(host, browser);

    /// <inheritdoc />
    public Control View { get; }

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderCommand> HeaderCommands => [];
}

/// <summary>
/// The navigation toolbar: back, forward, up, refresh, and an address to type.
/// </summary>
/// <remarks>
/// It is a view of the location like any browser dock, and the one place the location can be typed
/// into: the address reads what is being looked at and writes where to look, and the arrows walk the
/// history that changing it leaves behind (Section 7.5).
/// <para>
/// The view switch is not here, because it is not navigation: which layout entries are read in is a
/// property of the dock reading them, and each dock keeps its own.
/// </para>
/// </remarks>
internal sealed class NavigationControl : UserControl
{
    /// <summary>The name the toolbar opens with, which is the program's own and not a translation.</summary>
    private const string Brand = "Rorolala";

    /// <summary>The host, which is where a failure navigation cannot handle is reported.</summary>
    private readonly IPluginHost _host;

    /// <summary>The location, which this toolbar shows and switches.</summary>
    private readonly Browser _browser;

    /// <summary>The path being shown, and the place to type another.</summary>
    private readonly TextBox _address = new();

    /// <summary>The address as crumbs, which is how it is read; the field beside them is how it is typed.</summary>
    private readonly StackPanel _crumbs = new()
    {
        Orientation = Orientation.Horizontal,
        Spacing = 2,
        VerticalAlignment = VerticalAlignment.Center,
    };

    private readonly Button _back = Arrow("\u2190");
    private readonly Button _forward = Arrow("\u2192");
    private readonly Button _up = Arrow("\u2191");
    private readonly Button _refresh = Arrow("\u27f3");

    /// <summary>Makes the navigation toolbar.</summary>
    /// <param name="host">The host, for logging what navigation cannot do.</param>
    /// <param name="browser">The location, which this toolbar shows and switches.</param>
    public NavigationControl(IPluginHost host, Browser browser)
    {
        _host = host;
        _browser = browser;

        _back.Click += (_, _) => _browser.Back();
        _forward.Click += (_, _) => _browser.Forward();
        _up.Click += (_, _) => _browser.Up();
        _refresh.Click += (_, _) => _browser.Refresh();

        // The field is as wide as the design's own address: the crumbs beside it carry the whole path, so
        // the field only has to be wide enough to type into rather than to read.
        _address.Width = 210;
        _address.KeyDown += (_, args) =>
        {
            if (args.Key == Key.Enter)
            {
                Go(_address.Text);
            }
        };
        _address.LostFocus += (_, _) => _address.Text = Address(_browser.Current);

        var tools = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            VerticalAlignment = VerticalAlignment.Center,
        };
        tools.Children.Add(_back);
        tools.Children.Add(_forward);
        tools.Children.Add(_up);
        tools.Children.Add(_refresh);
        tools.Children.Add(_address);

        var mark = new Border
        {
            Width = 16,
            Height = 16,
            CornerRadius = new CornerRadius(5),
            BorderThickness = new Thickness(1),
            VerticalAlignment = VerticalAlignment.Center,
        };
        mark[!Border.BackgroundProperty] = new DynamicResourceExtension("rorolala.primary");
        mark[!Border.BorderBrushProperty] = new DynamicResourceExtension("rorolala.border.strong");

        var brand = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            VerticalAlignment = VerticalAlignment.Center,
        };
        brand.Children.Add(mark);
        brand.Children.Add(
            new TextBlock
            {
                Text = Brand,
                FontWeight = FontWeight.Bold,
                VerticalAlignment = VerticalAlignment.Center,
            }
        );

        // The crumbs are clipped rather than wrapped: a path longer than the band is cut off at the end
        // rather than pushing the tools off the bar.
        _crumbs.ClipToBounds = true;

        var bar = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("Auto,*,Auto"),
            ColumnSpacing = 12,
            VerticalAlignment = VerticalAlignment.Center,
        };
        Grid.SetColumn(brand, 0);
        Grid.SetColumn(_crumbs, 1);
        Grid.SetColumn(tools, 2);
        bar.Children.Add(brand);
        bar.Children.Add(_crumbs);
        bar.Children.Add(tools);

        var band = new Border
        {
            Height = 48,
            Padding = new Thickness(12, 0),
            VerticalAlignment = VerticalAlignment.Top,
            BorderThickness = new Thickness(0, 0, 0, 1),
            Child = bar,
        };
        band[!Border.BackgroundProperty] = new DynamicResourceExtension("rorolala.bg.elevated");
        band[!Border.BorderBrushProperty] = new DynamicResourceExtension("rorolala.border");

        Content = band;

        // Listening while it is on screen, like a browser dock: hidden is still attached, so the
        // address is already right the moment the dock is shown again.
        AttachedToVisualTree += (_, _) =>
        {
            _browser.Changed += Update;
            Update();
        };
        DetachedFromVisualTree += (_, _) => _browser.Changed -= Update;

        Update();
    }

    /// <summary>The toolbar buttons: a glyph, since they are arrows and a cycle.</summary>
    /// <remarks>
    /// Each is a square of its own, so the four read as one block of equal targets rather than as four
    /// words of different lengths, and the glyph is centred in it.
    /// </remarks>
    private static Button Arrow(string glyph) =>
        new()
        {
            Content = glyph,
            Classes = { "tool" },
            VerticalAlignment = VerticalAlignment.Center,
        };

    /// <summary>Brings the toolbar in step with the location.</summary>
    private void Update()
    {
        _back.IsEnabled = _browser.CanGoBack;
        _forward.IsEnabled = _browser.CanGoForward;
        _up.IsEnabled = _browser.CanGoUp;
        _address.Text = Address(_browser.Current);
        ShowCrumbs();
    }

    /// <summary>
    /// Writes the address out as crumbs, the last of them the place being looked at.
    /// </summary>
    /// <remarks>
    /// The address is split on the platform's own separators but always shown with <c>/</c>, since a crumb
    /// is a step of the path rather than the character the system happens to write it with.
    /// </remarks>
    private void ShowCrumbs()
    {
        _crumbs.Children.Clear();

        var parts = Address(_browser.Current).Split(
            [Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar],
            StringSplitOptions.RemoveEmptyEntries
        );

        if (parts.Length == 0)
        {
            return;
        }

        for (var at = 0; at < parts.Length; at++)
        {
            if (at > 0)
            {
                _crumbs.Children.Add(
                    new TextBlock
                    {
                        Text = "/",
                        Classes = { "faint" },
                        VerticalAlignment = VerticalAlignment.Center,
                    }
                );
            }

            var current = at == parts.Length - 1;
            var crumb = new TextBlock
            {
                Text = parts[at],
                VerticalAlignment = VerticalAlignment.Center,
            };

            if (current)
            {
                crumb.FontWeight = FontWeight.SemiBold;
            }
            else
            {
                crumb.Classes.Add("muted");
            }

            _crumbs.Children.Add(crumb);
        }
    }

    /// <summary>
    /// What the address says a location is.
    /// </summary>
    /// <remarks>
    /// The computer is the one place that has no path to type, so it is named instead — and the name
    /// is read back in <see cref="Go"/>, which is what a field that shows a word has to do.
    /// </remarks>
    private static string Address(string directory) =>
        Browser.IsComputer(directory)
            ? RolaI18N.Get("rorolala_file_system.computer")
            : directory;

    /// <summary>Goes to a directory typed into the address bar.</summary>
    private void Go(string? path)
    {
        if (string.IsNullOrWhiteSpace(path))
        {
            _address.Text = Address(_browser.Current);

            return;
        }

        var directory = path == RolaI18N.Get("rorolala_file_system.computer")
            ? Browser.Computer
            : path;

        // The address writes the location, and a path that is not a directory is refused by it: what
        // is left here is saying so and putting back what the location says.
        if (_browser.Go(directory))
        {
            return;
        }

        _host.Log.Warn(RolaI18N.Get("rorolala_file_system.not_a_directory", path));
        _address.Text = Address(_browser.Current);
    }
}
