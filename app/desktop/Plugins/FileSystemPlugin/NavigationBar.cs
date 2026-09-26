using System.IO;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Controls.Templates;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Markup.Xaml.MarkupExtensions;
using Avalonia.Media;
using Avalonia.Threading;
using Avalonia.VisualTree;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

namespace FileSystemPlugin;

/// <summary>
/// The browser's own toolbar: the arrows, and the address.
/// </summary>
/// <remarks>
/// It is a control rather than a dock, because navigation is not a place of its own: the arrows and the
/// address act on the one browser, and they belong at the top of the view whose entries they act on. A dock
/// that could be put anywhere is the wrong home for them — and the dock reading them hands in whatever else
/// it keeps, so the toolbar does not have to know what a dock remembers (Section 7.5).
/// <para>
/// The address is two things in one place, the way an address bar is: a line of crumbs that says where the
/// browser is, and — once the crumb of the place being looked at is clicked — a field with the whole path in
/// it, chosen, that can be typed over. A crumb before the last goes to the directory it names. Nothing is
/// typed into until it is asked for, so the path is read rather than edited by accident.
/// </para>
/// </remarks>
internal sealed class NavigationBar : UserControl
{
    /// <summary>
    /// How many completions are offered at once.
    /// </summary>
    /// <remarks>
    /// Enough to pick from and few enough to take in: a list longer than this is a list nobody reads down,
    /// and the point is to save typing rather than to browse.
    /// </remarks>
    private const int Completions = 20;

    /// <summary>The host, which is where a failure navigation cannot handle is reported.</summary>
    private readonly IPluginHost _host;

    /// <summary>The location, which this toolbar shows and switches.</summary>
    private Browser _browser;

    /// <summary>Whether the toolbar is listening to its location, which it does while it is on screen.</summary>
    private bool _watching;

    /// <summary>The path being read, which is what the address is until it is clicked.</summary>
    private readonly StackPanel _crumbs = new()
    {
        Orientation = Orientation.Horizontal,
        Spacing = 2,
        VerticalAlignment = VerticalAlignment.Center,
    };

    /// <summary>
    /// What the crumbs are read through, so that a path longer than the bar keeps its end on screen.
    /// </summary>
    /// <remarks>
    /// A path is read from its end, which is where the browser is and where the address is opened from: the
    /// beginning is what is worth losing when there is not room for both, and it is scrolled out rather than
    /// cut off.
    /// </remarks>
    private readonly ScrollViewer _breadth = new()
    {
        HorizontalScrollBarVisibility = ScrollBarVisibility.Hidden,
        VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
        VerticalAlignment = VerticalAlignment.Center,
    };

    /// <summary>The path being typed, shown in place of the crumbs while there is one.</summary>
    private readonly TextBox _address = new() { IsVisible = false };

    /// <summary>What the typed path could be, asked of the filesystem as it is typed.</summary>
    private readonly ListBox _suggestions = new() { MaxHeight = 240, MinWidth = 360, Focusable = false };

    /// <summary>The list of completions, under the field while there is anywhere to go.</summary>
    private readonly Popup _drop = new()
    {
        Placement = PlacementMode.BottomEdgeAlignedLeft,
        IsLightDismissEnabled = true,
        IsOpen = false,
    };

    private readonly Button _back = Arrow("\u2190");
    private readonly Button _forward = Arrow("\u2192");
    private readonly Button _up = Arrow("\u2191");
    private readonly Button _refresh = Arrow("\u27f3");

    /// <summary>Makes the toolbar.</summary>
    /// <param name="host">The host, for logging what navigation cannot do.</param>
    /// <param name="browser">The location, which this toolbar shows and switches.</param>
    /// <param name="trailing">
    /// What the dock reading the entries keeps, put at the far end of the bar. The toolbar places it rather
    /// than owning it: what a dock remembers is the dock's, and a toolbar that knew about it would be a
    /// toolbar that could only be used by one.
    /// </param>
    public NavigationBar(IPluginHost host, Browser browser, Control trailing)
    {
        _host = host;
        _browser = browser;

        _back.Click += (_, _) => _browser.Back();
        _forward.Click += (_, _) => _browser.Forward();
        _up.Click += (_, _) => _browser.Up();
        _refresh.Click += (_, _) => _browser.Refresh();

        FillAddress();

        // Clicking a crumb is what starts an edit, or goes where the crumb names; nothing else about the
        // address answers the pointer, so the path is read rather than edited by accident.
        _breadth.Content = _crumbs;

        var area = new Panel
        {
            Background = Brushes.Transparent,
            VerticalAlignment = VerticalAlignment.Center,
            Children = { _breadth, _address, _drop },
        };

        var tools = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            VerticalAlignment = VerticalAlignment.Center,
            Children = { _back, _forward, _up, _refresh },
        };

        trailing.VerticalAlignment = VerticalAlignment.Center;

        var bar = new Grid
        {
            ColumnDefinitions = new ColumnDefinitions("Auto,*,Auto"),
            ColumnSpacing = 12,
            VerticalAlignment = VerticalAlignment.Center,
        };
        Grid.SetColumn(tools, 0);
        Grid.SetColumn(area, 1);
        Grid.SetColumn(trailing, 2);
        bar.Children.Add(tools);
        bar.Children.Add(area);
        bar.Children.Add(trailing);

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

        // Listening while it is on screen, like the dock it sits in: hidden is still attached, so the
        // address is already right the moment the dock is shown again.
        AttachedToVisualTree += (_, _) =>
        {
            _watching = true;
            _browser.Changed += Update;
            Update();
        };
        DetachedFromVisualTree += (_, _) =>
        {
            _watching = false;
            _browser.Changed -= Update;
        };

        Update();
    }

    /// <summary>
    /// Shows and switches another location.
    /// </summary>
    /// <remarks>
    /// A dock that is taken out of step keeps one toolbar and hands it the location it has instead of being
    /// built again: the bar is the same bar, and handing the control it was made with over a second time would
    /// not work while that control is still the bar's own to lay out.
    /// </remarks>
    /// <param name="location">The location to show and switch.</param>
    public void Reading(Browser location)
    {
        if (ReferenceEquals(_browser, location))
        {
            return;
        }

        if (_watching)
        {
            _browser.Changed -= Update;
        }

        _browser = location;

        if (_watching)
        {
            _browser.Changed += Update;
            Update();
        }
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

    /// <summary>Sets the field and the list of completions up, and what each of them does.</summary>
    private void FillAddress()
    {
        _suggestions.ItemTemplate = new FuncDataTemplate<string>(
            (path, _) =>
                new TextBlock
                {
                    Text = path,
                    Classes = { "mono", "caption" },
                    VerticalAlignment = VerticalAlignment.Center,
                },
            true
        );

        _drop.Child = _suggestions;
        _drop.PlacementTarget = _address;

        _address.KeyDown += (_, args) => Keyed(args);
        _address.TextChanged += (_, _) => Suggest();

        // Leaving the field is leaving the edit: what was typed was not committed, so the address goes back
        // to saying where the browser actually is.
        _address.LostFocus += (_, _) => Rest();

        // Taken on the way down, before the item under the pointer can take the keyboard: the field is what a
        // user is typing into, and focus leaving it would end the edit rather than take what was picked.
        _suggestions.AddHandler(
            InputElement.PointerPressedEvent,
            (_, args) => Pick(args),
            RoutingStrategies.Tunnel
        );
    }

    /// <summary>Starts an edit: the whole path in a field, chosen, ready to be typed over.</summary>
    /// <remarks>
    /// Chosen and focused in the next turn of the loop rather than here, because a field that has just been
    /// made visible has not been laid out yet, and a selection asked of it now is a selection it forgets.
    /// </remarks>
    private void Edit()
    {
        if (_address.IsVisible)
        {
            return;
        }

        _address.Text = Address(_browser.Current);
        _crumbs.IsVisible = false;
        _breadth.IsVisible = false;
        _address.IsVisible = true;

        Dispatcher.UIThread.Post(() =>
        {
            _address.Focus();
            _address.SelectAll();
        });
    }

    /// <summary>Ends an edit: the crumbs, and no completions.</summary>
    private void Rest()
    {
        Close();
        _address.IsVisible = false;
        _breadth.IsVisible = true;
        _crumbs.IsVisible = true;
    }

    /// <summary>
    /// What a key means while the address is being typed into.
    /// </summary>
    /// <remarks>
    /// The arrows walk the completions rather than the text, the way an address bar does: the caret stays
    /// where it is and the list is what moves, so that Return can take what is picked without the hand
    /// leaving the row.
    /// </remarks>
    /// <param name="args">The key.</param>
    private void Keyed(KeyEventArgs args)
    {
        switch (args.Key)
        {
            case Key.Escape:
                args.Handled = true;
                Rest();

                break;

            case Key.Enter:
                args.Handled = true;

                // A picked completion is a whole path and is gone to as one; otherwise what was typed is.
                Go(Picked() ?? _address.Text);

                break;

            case Key.Down:
                args.Handled = true;
                Walk(1);

                break;

            case Key.Up:
                args.Handled = true;
                Walk(-1);

                break;

            case Key.Tab:
                if (Picked() is { } choice)
                {
                    args.Handled = true;
                    Take(choice);
                }

                break;

            default:
                break;
        }
    }

    /// <summary>Brings the toolbar in step with the location.</summary>
    private void Update()
    {
        _back.IsEnabled = _browser.CanGoBack;
        _forward.IsEnabled = _browser.CanGoForward;
        _up.IsEnabled = _browser.CanGoUp;

        ShowCrumbs();

        // An edit is abandoned when the location changes under it: what was being typed was a direction
        // somewhere else, and the browser has just gone elsewhere by another way.
        Rest();
    }

    /// <summary>
    /// Writes the address out as crumbs, the last of them the place being looked at.
    /// </summary>
    /// <remarks>
    /// The crumbs are the path walked down from the top, which is asked of the platform rather than cut out of
    /// the address text: a path is not a string to be split, and a drive, a root and a step are not the same
    /// shape on every system.
    /// <para>
    /// A crumb before the last goes to the directory it names. The last — the one being looked at, which is set
    /// heavier so that where we are is where the eye lands — opens the address for editing instead: a click on
    /// the place you are already in can only mean that what you want is to write somewhere else.
    /// </para>
    /// </remarks>
    private void ShowCrumbs()
    {
        _crumbs.Children.Clear();

        var chain = new List<string>();

        for (var path = _browser.Current; path.Length > 0; path = Path.GetDirectoryName(path) ?? string.Empty)
        {
            chain.Add(path);
        }

        chain.Reverse();

        // The computer is a place with no path at all, so it is one crumb and its whole name.
        if (chain.Count == 0)
        {
            _crumbs.Children.Add(Crumb(Address(_browser.Current), _browser.Current, current: true));

            return;
        }

        for (var at = 0; at < chain.Count; at++)
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

            var name = Path.GetFileName(chain[at]);

            // A root is a directory with nothing for a name: it is shown as the step it is, which is the
            // separator on Unix and the drive on Windows.
            if (name.Length == 0)
            {
                name = chain[at].TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
                name = name.Length > 0 ? name : "/";
            }

            _crumbs.Children.Add(Crumb(name, chain[at], at == chain.Count - 1));
        }

        // Scrolled to its end once it has been laid out, since how much of it there is to scroll is not known
        // until then: the last crumb is the one being looked at and the one the address is opened from.
        Dispatcher.UIThread.Post(() =>
            _breadth.Offset = new Vector(
                Math.Max(0, _breadth.Extent.Width - _breadth.Viewport.Width),
                0
            )
        );
    }

    /// <summary>
    /// One step of the address: a word that goes where it names, or opens the address when it is where we are.
    /// </summary>
    /// <remarks>
    /// A button rather than a line of text, because a step is a target: it is what is pressed, and it wears the
    /// design's own quiet button — nothing of it until the pointer is on it.
    /// </remarks>
    /// <param name="name">What the step is called.</param>
    /// <param name="path">The directory it names.</param>
    /// <param name="current">Whether it is the directory being looked at.</param>
    private Button Crumb(string name, string path, bool current)
    {
        var crumb = new Button
        {
            Content = name,
            Classes = { "ghost" },
            Padding = new Thickness(6, 2),
            VerticalAlignment = VerticalAlignment.Center,
        };

        if (current)
        {
            crumb.FontWeight = FontWeight.Bold;
        }
        else
        {
            crumb[!TemplatedControl.ForegroundProperty] = new DynamicResourceExtension("rorolala.fg.muted");
        }

        crumb.Click += (_, _) =>
        {
            if (current)
            {
                Edit();
            }
            else
            {
                _browser.Go(path);
            }
        };

        return crumb;
    }

    /// <summary>
    /// Offers what the typed path could be, asked of the filesystem as it is typed.
    /// </summary>
    /// <remarks>
    /// What is typed is split into the directory it is under and the beginning of a name, and that directory
    /// is listed: an address bar completes a name rather than searching, and the directory in hand is what a
    /// filesystem can answer for at every keystroke.
    /// <para>
    /// A directory that cannot be listed offers nothing rather than a reason: the path is still being typed,
    /// and a complaint over it would be noise. Hiding entries hides them from the completions too, because
    /// what is offered is what is in the listing.
    /// </para>
    /// </remarks>
    private void Suggest()
    {
        if (!_address.IsVisible)
        {
            return;
        }

        var split = Split(_address.Text ?? string.Empty, _browser.Current);

        if (split is not { } where)
        {
            Close();

            return;
        }

        var (directory, prefix) = where;

        string[] found;

        try
        {
            found = System.IO.Directory
                .EnumerateFileSystemEntries(directory)
                .Where(entry => Path.GetFileName(entry).StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
                .Where(entry => _browser.ShowHidden || !Browser.IsHidden(entry))
                .OrderBy(entry => Path.GetFileName(entry), StringComparer.OrdinalIgnoreCase)
                .Take(Completions)
                .Select(entry => (Path: entry, Directory: System.IO.Directory.Exists(entry)))
                // Directories first, then by name, which is the order a listing reads in rather than a second
                // one of its own.
                .OrderByDescending(entry => entry.Directory)
                .ThenBy(entry => Path.GetFileName(entry.Path), StringComparer.OrdinalIgnoreCase)
                .Select(entry => entry.Path)
                .ToArray();
        }
        catch (Exception error)
            when (error is IOException or UnauthorizedAccessException or ArgumentException or NotSupportedException)
        {
            Close();

            return;
        }

        if (found.Length == 0)
        {
            Close();

            return;
        }

        _suggestions.ItemsSource = found;
        _suggestions.SelectedIndex = -1;
        _drop.IsOpen = true;
    }

    /// <summary>Puts the completions away.</summary>
    private void Close()
    {
        _drop.IsOpen = false;
        _suggestions.ItemsSource = null;
        _suggestions.SelectedIndex = -1;
    }

    /// <summary>
    /// The directory a typed path is under, and the beginning of the name in it.
    /// </summary>
    /// <remarks>
    /// A path that ends with a separator has no name in it yet, so the whole of it is the directory; anything
    /// else is split at the last separator. What has no directory in front of it is read against the one being
    /// looked at, which is what makes a relative address complete at all — and a directory that is not one
    /// completes nothing, since there is nothing to list.
    /// </remarks>
    /// <param name="typed">What is in the field.</param>
    /// <param name="current">The directory being looked at, which a relative path is against.</param>
    /// <returns>The directory and the name, or nothing when there is nothing to list.</returns>
    private static (string Directory, string Prefix)? Split(string typed, string current)
    {
        if (typed.Length == 0)
        {
            return null;
        }

        var ends = Ends(typed);
        var cut = ends
            ? typed.Length
            : typed.LastIndexOfAny([Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar]) + 1;

        var head = typed[..cut];
        var name = ends ? string.Empty : typed[cut..];

        var directory = head.Length == 0
            ? current
            : Path.IsPathRooted(head)
                ? head
                : Path.Combine(current, head);

        // A root trims to nothing, and nothing names no directory: what a path was rooted at is the root.
        directory = directory.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);

        if (directory.Length == 0)
        {
            directory = Path.GetPathRoot(head) ?? string.Empty;
        }

        return directory.Length > 0 && System.IO.Directory.Exists(directory)
            ? (directory, name)
            : null;
    }

    /// <summary>Whether a typed path ends at a step of its own rather than in the middle of a name.</summary>
    /// <param name="typed">What is in the field.</param>
    private static bool Ends(string typed) =>
        typed[^1] == Path.DirectorySeparatorChar || typed[^1] == Path.AltDirectorySeparatorChar;

    /// <summary>The completion that is picked, or nothing while none is.</summary>
    private string? Picked() => _drop.IsOpen ? _suggestions.SelectedItem as string : null;

    /// <summary>Moves the pick through the completions, wrapping at either end.</summary>
    /// <param name="step">Which way.</param>
    private void Walk(int step)
    {
        var count = _suggestions.ItemCount;

        if (count == 0)
        {
            return;
        }

        var at = _suggestions.SelectedIndex + step;

        _suggestions.SelectedIndex = at < 0 ? count - 1 : at % count;
    }

    /// <summary>
    /// Takes a completion into the field.
    /// </summary>
    /// <remarks>
    /// A directory is written with the separator after it, which is how a hand continues from one step into the
    /// next; a file is written as it is, since there is nowhere under it to go. The list is asked again from
    /// what was written, by the keystroke that follows rather than here.
    /// <para>
    /// Taking rather than going is deliberate: the field is where the path is being built, and filling it lets
    /// a path be walked a step at a time. Return is what commits, whether what is committed was picked or typed.
    /// </para>
    /// </remarks>
    /// <param name="path">The completion taken.</param>
    private void Take(string path) =>
        _address.Text = System.IO.Directory.Exists(path)
            ? path + Path.DirectorySeparatorChar
            : path;

    /// <summary>What a press on the list picked, taken before the item can take the keyboard.</summary>
    /// <param name="args">The press.</param>
    private void Pick(PointerPressedEventArgs args)
    {
        if (
            args.Source is not Visual source
            || source.FindAncestorOfType<ListBoxItem>() is not { } item
            || _suggestions.ItemsSource is not IEnumerable<string> paths
        )
        {
            return;
        }

        var at = _suggestions.IndexFromContainer(item);

        if (at < 0)
        {
            return;
        }

        args.Handled = true;
        Take(paths.ElementAt(at));
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

    /// <summary>
    /// Goes to a path typed into the address, and puts the address back to reading.
    /// </summary>
    /// <remarks>
    /// A path that is not a directory leaves the location where it was: the address is the one thing a user
    /// can get wrong, so it is the one that says so, rather than the browser going somewhere unreadable
    /// (Section 7.5). Either way the field gives way to the crumbs, which say where the browser now is.
    /// </remarks>
    /// <param name="path">What was typed.</param>
    private void Go(string? path)
    {
        if (!string.IsNullOrWhiteSpace(path))
        {
            var directory = path == RolaI18N.Get("rorolala_file_system.computer")
                ? Browser.Computer
                : path;

            if (!_browser.Go(directory))
            {
                _host.Log.Warn(RolaI18N.Get("rorolala_file_system.not_a_directory", path));
            }
        }

        // The browser raising the change puts the crumbs right; this only ends the edit.
        Rest();
    }
}
