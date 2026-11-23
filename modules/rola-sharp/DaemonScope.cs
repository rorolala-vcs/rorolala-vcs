namespace RolaSharp;

/// <summary>The daemon this user's registrations are read through.</summary>
public sealed class DaemonScope : Scope
{
    private readonly UserScope _user;
    private IReadOnlyDictionary<string, VaultScope> _vaults =
        new Dictionary<string, VaultScope>();

    /// <summary>Makes a daemon scope under a user.</summary>
    /// <param name="user">The user whose registrations are read.</param>
    internal DaemonScope(UserScope user) => _user = user;

    /// <summary>The Vaults the daemon serves, by namespace.</summary>
    public IReadOnlyDictionary<string, VaultScope> RegisteredVaults
    {
        get
        {
            if (!Loaded)
            {
                throw new InvalidOperationException("call Read first");
            }

            return _vaults;
        }
    }

    /// <inheritdoc/>
    protected override void IsValid()
    {
        if (!Directory.Exists(_user.Root.FullName))
        {
            throw new RolaReadError(
                RolaReadErrorKind.InvalidArgument,
                _user.Root.FullName,
                "the user's data directory is not there"
            );
        }
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        var handle = RorolalaBinding.rola_daemon_vaults_new();
        if (handle == 0)
        {
            throw new RolaReadError(
                RolaReadErrorKind.Unreadable,
                null,
                "the daemon's registry could not be read"
            );
        }

        try
        {
            var vaults = new Dictionary<string, VaultScope>(StringComparer.Ordinal);
            var count = RorolalaBinding.rola_daemon_vaults_count(handle);
            for (nuint index = 0; index < count; index++)
            {
                cancellationToken.ThrowIfCancellationRequested();
                var item = RorolalaBinding.rola_daemon_vaults_at(handle, index);
                if (item == 0)
                {
                    continue;
                }

                try
                {
                    var name = NativeText.Taken(RorolalaBinding.rola_daemon_vault_name(item));
                    var path = NativeText.Taken(RorolalaBinding.rola_daemon_vault_path(item));
                    if (!string.IsNullOrEmpty(name) && !string.IsNullOrEmpty(path))
                    {
                        vaults[name] = new VaultScope(path);
                    }
                }
                finally
                {
                    RorolalaBinding.free_rola_daemon_vault(item);
                }
            }

            _vaults = vaults;
        }
        finally
        {
            RorolalaBinding.free_rola_daemon_vaults(handle);
        }
    }
}
