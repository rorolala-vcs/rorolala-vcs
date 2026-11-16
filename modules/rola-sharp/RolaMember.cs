namespace RolaSharp;

/// <summary>A member: a public key a Vault or a Workspace admits by, and the name it goes by.</summary>
/// <remarks>
/// A member is what an account proves itself as: the public half of a key pair, read from the key
/// directories the search was scoped to. The wrapper owns the member and gives it back on
/// <see cref="Dispose"/>.
/// </remarks>
public sealed class RolaMember : IDisposable
{
    /// <summary>The native member, owned by this instance while it lives.</summary>
    private nint _handle;

    /// <summary>Makes a wrapper over a native member, which it takes ownership of.</summary>
    /// <param name="handle">The native member, which must not be null.</param>
    internal RolaMember(nint handle) => _handle = handle;

    /// <summary>The member's name, which is the stem of its public key file.</summary>
    public string Name
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return NativeText.Taken(RorolalaBinding.rola_member_member_name(_handle));
        }
    }

    /// <summary>Where the member's public key file was found.</summary>
    public string KeyPath
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return NativeText.Taken(RorolalaBinding.rola_member_member_key_path(_handle));
        }
    }

    /// <summary>Gives the native member back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_member(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }
}

/// <summary>The members a search found, in the order it ranked them.</summary>
/// <remarks>
/// A member is a public key, which is what a Vault admits a caller by and what an administrator is
/// named as. The list is read one member at a time, and each member it hands out is the caller's to
/// release.
/// </remarks>
public sealed class RolaMembers : IDisposable
{
    /// <summary>The native member list, owned by this instance while it lives.</summary>
    private nint _handle;

    /// <summary>Makes a wrapper over a native member list, which it takes ownership of.</summary>
    /// <param name="handle">The native list, which must not be null.</param>
    private RolaMembers(nint handle) => _handle = handle;

    /// <summary>Finds the members a key scope holds.</summary>
    /// <param name="vault">
    /// The Vault the search is scoped to, when one is being worked in. It is what the local scope
    /// searches beside, and nothing is searched there for nothing.
    /// </param>
    /// <param name="rule">Which key directories the search may read.</param>
    /// <returns>The members found, or nothing when the list could not be made.</returns>
    public static RolaMembers? Locate(RolaVault? vault, RolaKeyLocateRule rule)
    {
        ArgumentNullException.ThrowIfNull(rule);

        var handle = RorolalaBinding.rola_member_locate(vault?.Handle ?? 0, rule.Handle);

        return handle == 0 ? null : new RolaMembers(handle);
    }

    /// <summary>How many members the search found.</summary>
    public int Count
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return (int)RorolalaBinding.rola_members_len(_handle);
        }
    }

    /// <summary>Whether the search found no members at all.</summary>
    public bool IsEmpty
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            return RorolalaBinding.is_rola_members_empty(_handle);
        }
    }

    /// <summary>The member at a position, counting from zero.</summary>
    /// <param name="index">The position.</param>
    /// <returns>The member, which the caller releases, or nothing past the end.</returns>
    public RolaMember? this[int index]
    {
        get
        {
            ObjectDisposedException.ThrowIf(_handle == 0, this);

            if (index < 0 || index >= Count)
            {
                return null;
            }

            var handle = RorolalaBinding.read_rola_members_by_index(_handle, (nuint)index);

            return handle == 0 ? null : new RolaMember(handle);
        }
    }

    /// <summary>Gives the native list back.</summary>
    public void Dispose()
    {
        if (_handle != 0)
        {
            RorolalaBinding.free_rola_members(_handle);
            _handle = 0;
        }

        GC.SuppressFinalize(this);
    }
}
