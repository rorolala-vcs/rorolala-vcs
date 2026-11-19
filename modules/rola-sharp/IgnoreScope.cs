using System.Runtime.InteropServices;

namespace RolaSharp;

/// <summary>The ignore rules a tree is read by.</summary>
public sealed class IgnoreScope : Scope, IDisposable
{
    private readonly string _rules;
    private readonly string _root;
    private nint _handle;

    /// <summary>Makes a scope over the rule files in a directory.</summary>
    /// <param name="rules">The directory holding the rule files, read recursively.</param>
    /// <param name="root">The tree's root, which a relative rule is read from.</param>
    public IgnoreScope(string rules, string root)
    {
        _rules = Path.GetFullPath(rules);
        _root = Path.GetFullPath(root);
    }

    /// <summary>Whether a path is ignored.</summary>
    /// <param name="path">The path, relative to the root.</param>
    /// <param name="directory">Whether it is a directory.</param>
    /// <returns>Whether the rules ignore it.</returns>
    public bool IsIgnored(string path, bool directory = false)
    {
        ArgumentNullException.ThrowIfNull(path);
        if (_handle == 0)
        {
            throw new InvalidOperationException("call Read first");
        }

        var text = Utf8(path);
        try
        {
            return RorolalaBinding.rola_ignore_rules_is_ignore(_handle, text, directory);
        }
        finally
        {
            Marshal.FreeCoTaskMem(text);
        }
    }

    /// <inheritdoc/>
    protected override void IsValid()
    {
        if (!Directory.Exists(_root))
        {
            throw new RolaReadError(
                RolaReadErrorKind.InvalidArgument,
                _root,
                "the rules' root is not there"
            );
        }
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        Dispose();
        cancellationToken.ThrowIfCancellationRequested();

        var rules = Utf8(_rules);
        var root = Utf8(_root);
        try
        {
            var handle = RorolalaBinding.load_rola_ignore_rules(rules, root);
            if (handle == 0)
            {
                throw new RolaReadError(
                    RolaReadErrorKind.Unreadable,
                    _root,
                    "the ignore rules could not be read"
                );
            }

            _handle = handle;
        }
        finally
        {
            Marshal.FreeCoTaskMem(rules);
            Marshal.FreeCoTaskMem(root);
        }
    }

    /// <summary>Gives the native rules back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_ignore_rules(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}
