using System.Text.Json;

namespace RolaSharp.V2;

/// <summary>The daemon this user's registrations are read through.</summary>
public sealed class DaemonScoop : Scoop
{
    private readonly UserScoop _user;
    private IReadOnlyDictionary<string, VaultScoop> _vaults =
        new Dictionary<string, VaultScoop>();

    /// <summary>Makes a daemon scope under a user.</summary>
    /// <param name="user">The user whose registrations are read.</param>
    internal DaemonScoop(UserScoop user) => _user = user;

    /// <summary>The Vaults the daemon serves, by namespace.</summary>
    public IReadOnlyDictionary<string, VaultScoop> RegisteredVaults
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
        if (!Directory.Exists(_user.Home.FullName))
        {
            throw new RolaReadError(
                RolaReadErrorKind.InvalidArgument,
                _user.Home.FullName,
                "the user's home is not there"
            );
        }
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        var json = NativeText.Taken(RorolalaBinding.rola_daemon_vaults_json());
        if (string.IsNullOrEmpty(json) || json == "null")
        {
            throw new RolaReadError(
                RolaReadErrorKind.Unreadable,
                null,
                "the daemon's registry could not be read"
            );
        }

        using var document = JsonDocument.Parse(json);
        var vaults = new Dictionary<string, VaultScoop>(StringComparer.Ordinal);
        var items = RolaJson.Items(document.RootElement, "vaults");
        if (items is not null)
        {
            foreach (var item in items.Value.EnumerateArray())
            {
                cancellationToken.ThrowIfCancellationRequested();

                var name = RolaJson.Text(item, "name");
                var path = RolaJson.Text(item, "path");
                if (!string.IsNullOrEmpty(name) && !string.IsNullOrEmpty(path))
                {
                    vaults[name] = new VaultScoop(path);
                }
            }
        }

        _vaults = vaults;
    }
}
