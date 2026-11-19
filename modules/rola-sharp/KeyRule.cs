namespace RolaSharp;

/// <summary>
/// Which key directories a user-scoped read may search.
/// </summary>
/// <remarks>
/// It is internal on purpose: the public surface answers what a user holds, and a caller that had to
/// build a rule to ask would be keeping a second copy of the search's shape. The one place a rule is
/// still named is the native boundary, and this is that wrapper.
/// </remarks>
internal sealed class KeyRule : IDisposable
{
    /// <summary>The native rule, owned by this instance while it lives.</summary>
    private nint _handle;

    /// <summary>Makes a rule with the local root on, which is where accounts live.</summary>
    private KeyRule(nint handle) => _handle = handle;

    /// <summary>The native rule, for the searches that read it.</summary>
    public nint Handle
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return _handle;
        }
    }

    /// <summary>Makes a rule that searches the local root and the user's own keys.</summary>
    public static KeyRule New()
    {
        var rule = new KeyRule(
            RorolalaBinding.rola_key_locate_rule_key_locate_rule_new()
        );
        RorolalaBinding.rola_key_locate_rule_key_locate_rule_set_find_local(rule._handle, true);
        RorolalaBinding.rola_key_locate_rule_key_locate_rule_set_find_user(rule._handle, true);

        return rule;
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
}
