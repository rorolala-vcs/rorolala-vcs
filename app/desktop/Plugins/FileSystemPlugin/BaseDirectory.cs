namespace FileSystemPlugin;

/// <summary>
/// The directory the tree is rooted at, published for the plugins that depend on this one.
/// </summary>
/// <remarks>
/// A dependent plugin reaches this plugin's extension points by depending on it — the manifest's
/// dependency is what has the host start that one after this — and what crosses is a catalogue, as
/// the hide providers and the lock providers are. What crosses here is where the work is: the base
/// is one answer for the whole plugin, and a view that is not the listing — Rorolala's own docks
/// among them — has no other way to learn it.
/// <para>
/// A catalogue rather than a member of the plugin, because there is no plugin instance a dependent
/// may hold: <c>Initialize</c> is called once and holds nothing public afterwards, and a second
/// answer kept beside the browser would be one to keep in step by hand. What is read before this
/// plugin starts is nothing, which a dependent need not handle — a plugin that depends on this one
/// is started after it.
/// </para>
/// </remarks>
public static class BaseDirectory
{
    /// <summary>
    /// The directory the tree is rooted at, or nothing before this plugin has started.
    /// </summary>
    /// <remarks>
    /// The whole plugin's answer and not a location's: a dock taken out of step looks at a directory
    /// of its own, and the tree is still rooted where the user rooted it.
    /// </remarks>
    public static string? Current { get; private set; }

    /// <summary>
    /// Raised when the tree is rooted somewhere else.
    /// </summary>
    /// <remarks>
    /// A view that draws the work rather than the listing follows this so that it says the same thing
    /// after the user roots the tree elsewhere as it did before.
    /// </remarks>
    public static event Action? Changed;

    /// <summary>
    /// Says where the tree is rooted now.
    /// </summary>
    /// <remarks>
    /// Said from the one place that owns the answer — the plugin's own shared state, which is written
    /// when it is made and whenever the base moves — so that what is published cannot drift from what
    /// the browser is rooted at. A base that is already the one published is not said again: a user
    /// who roots the tree where it already is asked for nothing, and a listener told otherwise would
    /// read the work again for a change that did not happen.
    /// </remarks>
    /// <param name="directory">The directory the tree is rooted at.</param>
    internal static void Publish(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        if (string.Equals(Current, directory, StringComparison.Ordinal))
        {
            return;
        }

        Current = directory;
        Changed?.Invoke();
    }
}
