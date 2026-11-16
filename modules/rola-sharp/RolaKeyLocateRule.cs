namespace RolaSharp;

/// <summary>Which key directories a search may read, and therefore what scope it covers.</summary>
/// <remarks>
/// The four scopes are the ones the command line knows: the machine's keys under the filesystem root,
/// the keys beside the Vault a run is in, the user's own keys, and the keys the environment names.
/// A rule made by <see cref="New"/> has all four on, and the wrapper owns it until
/// <see cref="Dispose"/>.
/// </remarks>
public sealed class RolaKeyLocateRule : IDisposable
{
    /// <summary>The native rule, owned by this instance while it lives.</summary>
    private nint _handle;

    /// <summary>Makes a wrapper over a native rule, which it takes ownership of.</summary>
    /// <param name="handle">The native rule, which must not be null.</param>
    private RolaKeyLocateRule(nint handle) => _handle = handle;

    /// <summary>The native rule, for the searches that read it.</summary>
    internal nint Handle
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return _handle;
        }
    }

    /// <summary>Makes a rule that searches every directory.</summary>
    /// <returns>The rule, which the caller releases.</returns>
    public static RolaKeyLocateRule New() => new(RorolalaBinding.rola_key_locate_rule_key_locate_rule_new());

    /// <summary>Whether the filesystem-root keys are searched.</summary>
    public bool FindGlobal
    {
        get => Read(RorolalaBinding.rola_key_locate_rule_key_locate_rule_find_global);
        set => Write(RorolalaBinding.rola_key_locate_rule_key_locate_rule_set_find_global, value);
    }

    /// <summary>Whether the keys beside the current Vault are searched.</summary>
    public bool FindLocal
    {
        get => Read(RorolalaBinding.rola_key_locate_rule_key_locate_rule_find_local);
        set => Write(RorolalaBinding.rola_key_locate_rule_key_locate_rule_set_find_local, value);
    }

    /// <summary>Whether the user's local data keys are searched.</summary>
    public bool FindUser
    {
        get => Read(RorolalaBinding.rola_key_locate_rule_key_locate_rule_find_user);
        set => Write(RorolalaBinding.rola_key_locate_rule_key_locate_rule_set_find_user, value);
    }

    /// <summary>Whether the keys the environment names are searched.</summary>
    public bool FindEnv
    {
        get => Read(RorolalaBinding.rola_key_locate_rule_key_locate_rule_find_env);
        set => Write(RorolalaBinding.rola_key_locate_rule_key_locate_rule_set_find_env, value);
    }

    /// <summary>Gives the native rule back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_key_locate_rule(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }

    /// <summary>Reads one of the rule's switches.</summary>
    /// <param name="read">The native reader.</param>
    /// <returns>Whether the scope is on.</returns>
    private bool Read(Func<nint, bool> read)
    {
        ObjectDisposedException.ThrowIf(_handle == 0, this);

        return read(_handle);
    }

    /// <summary>Turns one of the rule's switches on or off.</summary>
    /// <param name="write">The native setter.</param>
    /// <param name="value">Whether the scope is to be on.</param>
    private void Write(Action<nint, bool> write, bool value)
    {
        ObjectDisposedException.ThrowIf(_handle == 0, this);

        write(_handle, value);
    }
}
