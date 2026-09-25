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
    /// <param name="navigator">The browsers, of which it drives the active one.</param>
    public NavigationDock(IPluginHost host, Navigator navigator) =>
        View = new NavigationControl(host, navigator);

    /// <inheritdoc />
    public Control View { get; }

    /// <inheritdoc />
    public IReadOnlyList<DockHeaderCommand> HeaderCommands => [];
}

/// <summary>
/// The navigation toolbar: back, forward, up, refresh, and an address to type.
/// </summary>
/// <remarks>
/// It drives whichever browser is active, and says so by being disabled while none is: the controls
/// are greyed rather than hidden, so the dock still shows what it would offer.
/// <para>
/// The view switch is not here, because it is not navigation: which layout entries are read in is a
/// property of the browser reading them, and each browser keeps its own (Section 7.5).
/// </para>
/// </remarks>
internal sealed class NavigationControl : UserControl
{
    /// <summary>The host, which is where a failure navigation cannot handle is reported.</summary>
    private readonly IPluginHost _host;

    /// <summary>The browsers, of which this toolbar drives the active one.</summary>
    private readonly Navigator _navigator;

    /// <summary>The path being shown, and the place to type another.</summary>
    private readonly TextBox _address = new();

    private readonly Button _back = Arrow("\u2190");
    private readonly Button _forward = Arrow("\u2192");
    private readonly Button _up = Arrow("\u2191");
    private readonly Button _refresh = Arrow("\u27f3");

    /// <summary>The browser being driven, or nothing while none is shown.</summary>
    private Browser? _browser;

    /// <summary>Makes the navigation toolbar.</summary>
    /// <param name="host">The host, for logging what navigation cannot do.</param>
    /// <param name="navigator">The browsers, of which this toolbar drives the active one.</param>
    public NavigationControl(IPluginHost host, Navigator navigator)
    {
        _host = host;
        _navigator = navigator;

        _back.Click += (_, _) => _browser?.Back();
        _forward.Click += (_, _) => _browser?.Forward();
        _up.Click += (_, _) => _browser?.Up();
        _refresh.Click += (_, _) => _browser?.Refresh();

        _address.Width = 320;
        _address.KeyDown += (_, args) =>
        {
            if (args.Key == Key.Enter)
            {
                Go(_address.Text);
            }
        };
        _address.LostFocus += (_, _) => _address.Text = _browser?.Current ?? "";

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

        _navigator.Changed += Bind;
        Bind();
    }

    /// <summary>The toolbar buttons: a glyph, since they are arrows and a cycle.</summary>
    private static Button Arrow(string glyph) =>
        new()
        {
            Content = glyph,
            Padding = new Thickness(8, 2),
            MinWidth = 30,
        };

    /// <summary>Drives the browser that is active now rather than the one that was.</summary>
    private void Bind()
    {
        if (_browser is not null)
        {
            _browser.Changed -= Update;
        }

        _browser = _navigator.Active;

        if (_browser is not null)
        {
            _browser.Changed += Update;
        }

        Update();
    }

    /// <summary>Brings the toolbar in step with the browser it drives.</summary>
    private void Update()
    {
        var browser = _browser;

        IsEnabled = browser is not null;
        _back.IsEnabled = browser?.CanGoBack ?? false;
        _forward.IsEnabled = browser?.CanGoForward ?? false;
        _up.IsEnabled = browser?.CanGoUp ?? false;
        _address.Text = browser?.Current ?? "";
    }

    /// <summary>Goes to a directory typed into the address bar.</summary>
    private void Go(string? path)
    {
        if (_browser is not { } browser || string.IsNullOrWhiteSpace(path))
        {
            _address.Text = _browser?.Current ?? "";

            return;
        }

        if (!System.IO.Directory.Exists(path))
        {
            _host.Log.Warn(RolaI18N.Get("rorolala_file_system.not_a_directory", path));
            _address.Text = browser.Current;

            return;
        }

        browser.Go(path);
    }
}
