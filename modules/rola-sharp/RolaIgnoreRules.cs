using System.Runtime.InteropServices;

namespace RolaSharp;

/// <summary>
/// The ignore rules a Workspace or a Vault is managed by.
/// </summary>
/// <remarks>
/// A hand-written wrapper over the generated bindings, as <see cref="RolaWorkspace"/> is: what crosses
/// is an opaque handle the native side owns, and what this adds is the shape a caller wants — a set is
/// loaded from a directory rather than built from a pointer, absence is <see langword="null"/>, and the
/// handle is given back on <see cref="Dispose"/>.
/// <para>
/// The rules themselves are the native side's: the syntax is gitignore's, the order the files apply in
/// is the loader's, and nothing of either is reimplemented here. What a caller gets is one question —
/// whether a path is ignored — asked as often as a listing needs it.
/// </para>
/// </remarks>
public sealed class RolaIgnoreRules : IDisposable
{
    /// <summary>The native rule set, owned by this instance while it lives.</summary>
    private nint _handle;

    /// <summary>Makes a wrapper over a native rule set, which it takes ownership of.</summary>
    /// <param name="handle">The native rule set, which must not be null.</param>
    private RolaIgnoreRules(nint handle) => _handle = handle;

    /// <summary>
    /// Loads the rules held in <paramref name="rules"/>, matched against paths written from <paramref name="root"/>.
    /// </summary>
    /// <remarks>
    /// A directory that is not there is an empty set rather than nothing: a Workspace none of whose
    /// rules have been written to yet is a Workspace that ignores nothing, and that is an answer. What
    /// is nothing is the rules that could not be read at all.
    /// </remarks>
    /// <param name="rules">The directory holding the rule files, read recursively.</param>
    /// <param name="root">The directory the rules are written from, which paths are matched against.</param>
    /// <returns>The rules, or nothing when they could not be read.</returns>
    public static RolaIgnoreRules? Load(string rules, string root)
    {
        ArgumentNullException.ThrowIfNull(rules);
        ArgumentNullException.ThrowIfNull(root);

        // A path crosses as bytes, and the platform's own encoding is UTF-8 — which is what this
        // writes. Both buffers are this side's to free, which is why they are in a `finally`.
        var rulesPath = Marshal.StringToCoTaskMemUTF8(rules);
        var rootPath = Marshal.StringToCoTaskMemUTF8(root);

        try
        {
            var handle = RorolalaBinding.load_rola_ignore_rules(rulesPath, rootPath);

            return handle == 0 ? null : new RolaIgnoreRules(handle);
        }
        finally
        {
            Marshal.FreeCoTaskMem(rulesPath);
            Marshal.FreeCoTaskMem(rootPath);
        }
    }

    /// <summary>
    /// Whether a path is ignored: the last rule that matches it says it is.
    /// </summary>
    /// <remarks>
    /// <paramref name="isDirectory"/> is what a rule naming a directory only — one ending in `/` — is
    /// decided by. A path that is absolute and under the root the set was loaded with is matched
    /// against its place under that root, so a listing's system paths and the Layout's own spelling ask
    /// the same question.
    /// </remarks>
    /// <param name="path">The path to ask about.</param>
    /// <param name="isDirectory">Whether the path is a directory.</param>
    /// <returns>Whether the rules ignore it.</returns>
    public bool IsIgnored(string path, bool isDirectory)
    {
        ObjectDisposedException.ThrowIf(_handle == 0, this);
        ArgumentNullException.ThrowIfNull(path);

        var text = Marshal.StringToCoTaskMemUTF8(path);

        try
        {
            return RorolalaBinding.rola_ignore_rules_is_ignore(_handle, text, isDirectory);
        }
        finally
        {
            Marshal.FreeCoTaskMem(text);
        }
    }

    /// <summary>Gives the native rule set back.</summary>
    public void Dispose()
    {
        // Nothing to give back the second time, and nothing was lost the first: the handle is forgotten
        // as soon as it is released, so a double `Dispose` releases nothing twice.
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_ignore_rules(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }
}
