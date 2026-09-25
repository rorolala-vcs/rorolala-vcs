namespace FileSystemPlugin;

/// <summary>
/// The browsers that are shown, and which of them the navigation dock drives.
/// </summary>
/// <remarks>
/// A dock of open mode <c>New</c> browses apart from its fellows, so there may be several browsers at
/// once while the navigation dock is a dock of open mode <c>Toggle</c>, of which there is one. That
/// one has to choose, and what it chooses is the active browser: the one the user last reached into,
/// and the newest while none has been touched.
/// <para>
/// The plugin makes one of these, not one per dock: it is the only thing the browser factories and
/// the navigation factory have in common, and sharing it is what lets a browser dock made later be
/// driven by a navigation dock made earlier.
/// </para>
/// </remarks>
internal sealed class Navigator
{
    /// <summary>The browsers being shown, in the order they joined.</summary>
    private readonly List<Browser> _browsers = [];

    /// <summary>The browser the navigation dock drives, or nothing while none is shown.</summary>
    private Browser? _active;

    /// <summary>Raised when the browser to drive changes, which is also what a view redraws on.</summary>
    public event Action? Changed;

    /// <summary>The browser the navigation dock drives.</summary>
    public Browser? Active => _active;

    /// <summary>Adds a browser that has just been shown, and makes it the one to drive.</summary>
    /// <param name="browser">The browser that was shown.</param>
    public void Add(Browser browser)
    {
        if (_browsers.Contains(browser))
        {
            return;
        }

        _browsers.Add(browser);
        _active = browser;

        Changed?.Invoke();
    }

    /// <summary>Forgets a browser that is no longer shown, handing the dock to another if it was driven.</summary>
    /// <param name="browser">The browser that is gone.</param>
    public void Remove(Browser browser)
    {
        if (!_browsers.Remove(browser))
        {
            return;
        }

        if (ReferenceEquals(_active, browser))
        {
            _active = _browsers.LastOrDefault();
        }

        Changed?.Invoke();
    }

    /// <summary>Hands the navigation dock to a browser the user reached into.</summary>
    /// <param name="browser">The browser to drive.</param>
    public void Activate(Browser browser)
    {
        if (ReferenceEquals(_active, browser))
        {
            return;
        }

        _active = browser;

        Changed?.Invoke();
    }
}
