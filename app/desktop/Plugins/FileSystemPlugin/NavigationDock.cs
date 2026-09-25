using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
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
    /// <summary>The host, which is where a failure navigation cannot handle is reported.</summary>
    private readonly IPluginHost _host;

    /// <summary>The location, which this toolbar shows and switches.</summary>
    private readonly Browser _browser;

    /// <summary>The path being shown, and the place to type another.</summary>
    private readonly TextBox _address = new();

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

        _address.Width = 320;
        _address.KeyDown += (_, args) =>
        {
            if (args.Key == Key.Enter)
            {
                Go(_address.Text);
            }
        };
        _address.LostFocus += (_, _) => _address.Text = _browser.Current;

        var toolbar = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 4,
            Margin = new Thickness(6),
        };
        toolbar.Children.Add(_back);
        toolbar.Children.Add(_forward);
        toolbar.Children.Add(_up);
        toolbar.Children.Add(_refresh);
        toolbar.Children.Add(_address);

        Content = toolbar;

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
    private static Button Arrow(string glyph) =>
        new()
        {
            Content = glyph,
            Padding = new Thickness(8, 2),
            MinWidth = 30,
        };

    /// <summary>Brings the toolbar in step with the location.</summary>
    private void Update()
    {
        _back.IsEnabled = _browser.CanGoBack;
        _forward.IsEnabled = _browser.CanGoForward;
        _up.IsEnabled = _browser.CanGoUp;
        _address.Text = _browser.Current;
    }

    /// <summary>Goes to a directory typed into the address bar.</summary>
    private void Go(string? path)
    {
        if (string.IsNullOrWhiteSpace(path))
        {
            _address.Text = _browser.Current;

            return;
        }

        // The address writes the location, and a path that is not a directory is refused by it: what
        // is left here is saying so and putting back what the location says.
        if (_browser.Go(path))
        {
            return;
        }

        _host.Log.Warn(RolaI18N.Get("rorolala_file_system.not_a_directory", path));
        _address.Text = _browser.Current;
    }
}
