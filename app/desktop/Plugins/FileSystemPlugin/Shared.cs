namespace FileSystemPlugin;

/// <summary>
/// The two answers about looking at files that are the whole plugin's rather than one location's.
/// </summary>
/// <remarks>
/// Where the tree is rooted, and whether the entries the platform hides are shown. Neither is where anything
/// is looking, and both are one answer for the whole File System: a base per dock would be several answers to
/// where the tree is rooted, and a hiding per dock would let two docks list one directory differently.
/// <para>
/// They live here rather than on a <see cref="Browser"/> because a dock may be taken out of step and given a
/// location of its own (Section 7.5). The location is then that dock's alone, while these two stay
/// everybody's — which is what a location reads when it stages a listing, and this is what tells every one of
/// them that the answer changed.
/// </para>
/// </remarks>
internal sealed class Shared
{
    /// <summary>The directory the tree is rooted at.</summary>
    private string _base;

    /// <summary>Whether the entries the platform hides are shown.</summary>
    private bool _hidden;

    /// <summary>Makes the shared answers, with the tree rooted at a directory.</summary>
    /// <param name="baseDir">The directory to root the tree at.</param>
    public Shared(string baseDir) => _base = baseDir;

    /// <summary>
    /// Raised when either answer changes, so that every location reads its directory again.
    /// </summary>
    /// <remarks>
    /// A location is what holds a listing, so a change here has to reach every location there is — including
    /// one a dock was given for being out of step, since a listing follows the base and the hiding wherever it
    /// is.
    /// </remarks>
    public event Action? Changed;

    /// <summary>
    /// The directory the tree is rooted at, which a directory's own menu sets.
    /// </summary>
    /// <remarks>
    /// The tree is rooted here rather than at a location so that stepping through directories does not move
    /// the tree: what a user opens stays open, and the tree is the one view that is not rearranged by where a
    /// browser happens to be.
    /// </remarks>
    public string BaseDir
    {
        get => _base;
        set
        {
            if (string.Equals(_base, value, StringComparison.Ordinal))
            {
                return;
            }

            _base = value;
            Changed?.Invoke();
        }
    }

    /// <summary>
    /// Whether the entries the platform hides are shown.
    /// </summary>
    /// <remarks>
    /// It is the plugin's rather than a view's, because the listing is what it changes: two docks looking at
    /// one directory — and a dock out of step still looks at directories — must not list different things.
    /// What a dock keeps is whether the user asked for them, since a dock is what a toggle sits in
    /// (Section 7.5).
    /// </remarks>
    public bool ShowHidden
    {
        get => _hidden;
        set
        {
            if (_hidden == value)
            {
                return;
            }

            _hidden = value;
            Changed?.Invoke();
        }
    }
}
