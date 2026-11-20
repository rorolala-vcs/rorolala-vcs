using System.Diagnostics.CodeAnalysis;
using System.Runtime.InteropServices;

using Rorolala;

namespace RolaSharp;

/// <summary>The user's own data: the accounts, keys and daemon registrations under a userdata name.</summary>
/// <remarks>
/// The name is the one this assembly was built for, which is <c>release</c> unless the build named
/// another; the constructor names one explicitly. Where a name sits is
/// <see cref="UserDataPaths"/>'s to say, so this class says what a name holds rather than where the
/// platform keeps it.
/// </remarks>
public sealed class UserScope : Scope
{
    private readonly string _name;
    private readonly string _root;
    private readonly string _keys;
    private IReadOnlyList<Account> _accounts = [];
    private IReadOnlyList<PublicKey> _publicKeys = [];
    private IReadOnlyList<PrivateKey> _privateKeys = [];

    /// <summary>Makes a scope over a userdata name.</summary>
    /// <param name="profile">
    /// The name to read, or nothing for the one this assembly was built for. It is one directory
    /// segment: letters, digits, <c>-</c> and <c>_</c>, not starting with <c>-</c>.
    /// </param>
    /// <exception cref="ArgumentException">The name is not one directory segment.</exception>
    public UserScope(string? profile = null)
    {
        _name = string.IsNullOrEmpty(profile) ? UserDataPaths.Name : profile;

        if (!IsName(_name))
        {
            throw new ArgumentException(
                "a userdata name is one directory segment of letters, digits, `-` and `_`",
                nameof(profile)
            );
        }

        _root = UserDataPaths.RootFor(_name);
        _keys = UserDataPaths.KeysFor(_name);
    }

    /// <summary>The current user's scope, under the name the assembly was built for.</summary>
    public static UserScope Current => new();

    /// <summary>The account the work acts as, as Rorolala's own file for the user names it.</summary>
    /// <remarks>
    /// An account that was never named answers empty, which is the same answer the command line gives.
    /// </remarks>
    public static string CurrentAccount =>
        NativeText.Taken(RorolalaBinding.rola_current_account());

    /// <summary>The userdata name this scope reads.</summary>
    public string Profile => _name;

    /// <summary>The directory this name's own files sit in.</summary>
    public DirectoryInfo Root => new(_root);

    /// <summary>Where this name's keys are kept.</summary>
    public DirectoryInfo KeysDirectory => new(_keys);

    /// <summary>The accounts the user holds.</summary>
    public IReadOnlyList<Account> Accounts
    {
        get
        {
            EnsureLoaded();
            return _accounts;
        }
    }

    /// <summary>The public keys the user holds.</summary>
    public IReadOnlyList<PublicKey> PublicKeys
    {
        get
        {
            EnsureLoaded();
            return _publicKeys;
        }
    }

    /// <summary>The private keys the user holds.</summary>
    public IReadOnlyList<PrivateKey> PrivateKeys
    {
        get
        {
            EnsureLoaded();
            return _privateKeys;
        }
    }

    /// <summary>The daemon this user's registrations are read through.</summary>
    /// <returns>The daemon scope.</returns>
    [SuppressMessage(
        "Design",
        "CA1024:Use properties where appropriate",
        Justification = "the issue names GetDaemonScope, and a method reads as the action of asking for a scope"
    )]
    public DaemonScope GetDaemonScope() => new(this);

    /// <inheritdoc/>
    protected override void IsValid()
    {
        // A name's directory need not be there yet: reading a directory no run has written is an
        // empty answer, the one a fresh machine gives, rather than a failure.
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        using var rule = KeyRule.New();
        var accounts = new List<Account>();
        var publicKeys = new List<PublicKey>();
        var privateKeys = new List<PrivateKey>();

        var keys = Utf8(_keys);
        try
        {
            var handle = RorolalaBinding.rola_user_accounts(keys, rule.Handle);
            if (handle != 0)
            {
                try
                {
                    var count = (int)RorolalaBinding.rola_accounts_len(handle);
                    for (var index = 0; index < count; index++)
                    {
                        cancellationToken.ThrowIfCancellationRequested();

                        var account = RorolalaBinding.read_rola_accounts_by_index(handle, (nuint)index);
                        if (account == 0)
                        {
                            continue;
                        }

                        try
                        {
                            var name = NativeText.Taken(RorolalaBinding.rola_account_account_name(account));
                            var path = NativeText.Taken(RorolalaBinding.rola_account_account_key_path(account));
                            var privateKey = new PrivateKey(name, new FileInfo(path));
                            var publicPath = Path.ChangeExtension(path, ".pub");
                            var publicKey = File.Exists(publicPath)
                                ? new PublicKey(name, new FileInfo(publicPath))
                                : null;

                            accounts.Add(new Account(name, privateKey, publicKey));
                            privateKeys.Add(privateKey);
                            if (publicKey is not null)
                            {
                                publicKeys.Add(publicKey);
                            }
                        }
                        finally
                        {
                            RorolalaBinding.free_rola_account(account);
                        }
                    }
                }
                finally
                {
                    RorolalaBinding.free_rola_accounts(handle);
                }
            }

            var members = RorolalaBinding.rola_user_members(keys, rule.Handle);
            if (members != 0)
            {
                try
                {
                    var count = (int)RorolalaBinding.rola_members_len(members);
                    for (var index = 0; index < count; index++)
                    {
                        cancellationToken.ThrowIfCancellationRequested();

                        var member = RorolalaBinding.read_rola_members_by_index(members, (nuint)index);
                        if (member == 0)
                        {
                            continue;
                        }

                        try
                        {
                            var name = NativeText.Taken(RorolalaBinding.rola_member_member_name(member));
                            var path = NativeText.Taken(RorolalaBinding.rola_member_member_key_path(member));
                            var key = new PublicKey(name, new FileInfo(path));
                            if (publicKeys.All(existing => existing.Name != name))
                            {
                                publicKeys.Add(key);
                            }
                        }
                        finally
                        {
                            RorolalaBinding.free_rola_member(member);
                        }
                    }
                }
                finally
                {
                    RorolalaBinding.free_rola_members(members);
                }
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(keys);
        }

        _accounts = accounts;
        _publicKeys = publicKeys;
        _privateKeys = privateKeys;
    }

    private void EnsureLoaded()
    {
        if (!Loaded)
        {
            throw new InvalidOperationException("call Read first");
        }
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);

    /// <summary>Whether a name is one directory segment.</summary>
    /// <param name="name">The name to check.</param>
    /// <returns>Whether the name may be used.</returns>
    private static bool IsName(string name) =>
        name.Length > 0
        && name[0] != '-'
        && name.All(character =>
            char.IsAsciiLetterOrDigit(character) || character is '-' or '_'
        );
}
