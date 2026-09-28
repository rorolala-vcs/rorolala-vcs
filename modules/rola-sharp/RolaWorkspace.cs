using System.Runtime.InteropServices;

namespace RolaSharp;

/// <summary>
/// A workspace: the directory a run works in, and everything under it.
/// </summary>
/// <remarks>
/// A hand-written wrapper over the generated bindings, <see cref="RorolalaBinding"/>, which are the
/// private half of this assembly: pointers, plain values and nothing that names what they mean. What
/// this adds is the shape a caller wants instead. A workspace is found from a directory rather than
/// built from a pointer; absence — no workspace holds the directory — is <see langword="null"/> rather
/// than a pointer to remember to check; and the native workspace is owned while this lives and given
/// back on <see cref="Dispose"/>, so a caller holds one the way it holds any other resource.
/// <para>
/// It is the pattern the rest of the hand-written layer follows: one class per native type, holding
/// its handle, releasing it when it is done, and reaching into the bindings for everything else.
/// </para>
/// </remarks>
public sealed class RolaWorkspace : IDisposable
{
    /// <summary>The native workspace, owned by this instance while it lives.</summary>
    private nint _handle;

    /// <summary>Makes a wrapper over a native workspace, which it takes ownership of.</summary>
    /// <param name="handle">The native workspace, which must not be null.</param>
    private RolaWorkspace(nint handle) => _handle = handle;

    /// <summary>
    /// Finds the workspace holding a directory, by searching upwards from it.
    /// </summary>
    /// <param name="directory">The directory to search from.</param>
    /// <returns>The workspace, or nothing when no workspace holds the directory.</returns>
    public static RolaWorkspace? Locate(string directory)
    {
        ArgumentNullException.ThrowIfNull(directory);

        // A path crosses as bytes, and the platform's own encoding is UTF-8 — which is what this
        // writes. The buffer is this side's to free, which is why it is in a `finally`: the search
        // raising is no reason to lose it.
        var path = Marshal.StringToCoTaskMemUTF8(directory);

        try
        {
            var handle = RorolalaBinding.locate_rola_workspace(path);

            return handle == 0 ? null : new RolaWorkspace(handle);
        }
        finally
        {
            Marshal.FreeCoTaskMem(path);
        }
    }

    /// <summary>
    /// Whether the workspace is locked.
    /// </summary>
    /// <remarks>
    /// A workspace locked by another run and one whose lock was left behind by a run that did not
    /// finish are both answered as locked: a lock is the file, and not a claim about who holds it.
    /// </remarks>
    public bool IsLocking
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return RorolalaBinding.RolaWorkspace_is_locking(_handle);
        }
    }

    /// <summary>Gives the native workspace back.</summary>
    public void Dispose()
    {
        // Nothing to give back the second time, and nothing was lost the first: the handle is
        // forgotten as soon as it is released, so a double `Dispose` releases nothing twice.
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_workspace(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }
}
