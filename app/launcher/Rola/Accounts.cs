using System.Text.Json;

namespace RorolalaLauncher.Rola;

/// <summary>The accounts this machine holds, and which one the work acts as.</summary>
/// <param name="Current">The account the work acts as, or nothing when none has been chosen.</param>
/// <param name="Names">Every account any scope holds, in name order.</param>
internal sealed record AccountList(string? Current, IReadOnlyList<string> Names);

/// <summary>
/// The accounts this machine holds, as the command line names them.
/// </summary>
/// <remarks>
/// An account is a private key and the choice of which one the work acts as is Rorolala's, kept in its own
/// file for the user. None of it is read or written here: what an account is, is the command line's answer,
/// and this only asks for it and reads the answer back.
/// </remarks>
/// <param name="rola">The command line program.</param>
internal sealed class Accounts(RolaRunner rola)
{
    /// <summary>
    /// The names that cannot be an account, because <c>rola account</c> reads them as its own words.
    /// </summary>
    /// <remarks>
    /// A name is given to <c>rola account</c> as a word, and a word it knows as a subcommand is that
    /// subcommand rather than an account: one called <c>ls</c> would list the accounts instead of being
    /// chosen, and one called <c>current</c> would print the account in use. Refused here so that a reader
    /// is told before a key is made under a name that could never be chosen.
    /// </remarks>
    public static readonly IReadOnlyList<string> Forbidden = ["current", "ls"];

    /// <summary>Lists the accounts, as <c>rola account</c> lists them.</summary>
    /// <param name="directory">Where the run is made, which is the scopes a key is looked for in.</param>
    /// <returns>The accounts, or none when the command did not answer.</returns>
    public AccountList List(string directory)
    {
        var result = rola.Run(["--json", "account"], directory);

        if (!result.Done)
        {
            return new AccountList(null, []);
        }

        try
        {
            using var document = JsonDocument.Parse(result.Output);

            if (
                document.RootElement.ValueKind is not JsonValueKind.Object
                || !document.RootElement.TryGetProperty("names", out var names)
                || names.ValueKind is not JsonValueKind.Array
            )
            {
                return new AccountList(null, []);
            }

            var current =
                document.RootElement.TryGetProperty("current", out var held)
                && held.ValueKind is JsonValueKind.String
                    ? held.GetString()
                    : null;

            return new AccountList(
                current,
                [
                    .. names.EnumerateArray()
                        .Select(name => name.GetString())
                        .Where(name => name is { Length: > 0 })
                        .Select(name => name!),
                ]
            );
        }
        catch (JsonException)
        {
            return new AccountList(null, []);
        }
    }

    /// <summary>
    /// Makes a key pair for an account and makes it the one the work acts as.
    /// </summary>
    /// <remarks>
    /// The two commands are the whole of what an account is — <c>rola key generate --install</c> writes the
    /// pair into the user's key directory, and <c>rola account</c> records which one is in use — so what is
    /// reported is what the first of them that failed said.
    /// </remarks>
    /// <param name="name">What the pair is known by.</param>
    /// <param name="directory">Where the runs are made.</param>
    /// <returns>Whether it ended well, and what to say when it did not.</returns>
    public (bool Done, string Said) Create(string name, string directory)
    {
        var made = rola.Run(["key", "generate", name, "--install"], directory);

        if (!made.Done)
        {
            return (false, made.Reason);
        }

        var used = rola.Run(["account", name], directory);

        return used.Done ? (true, string.Empty) : (false, used.Reason);
    }

    /// <summary>
    /// The public half of an account's key pair, found the way the command line finds keys.
    /// </summary>
    /// <remarks>
    /// What is listed is the paths the command line ranks, so a name that resolves to a file here is one a
    /// run would resolve the same way. The name a path stands for is its stem: <c>alice.pem</c> and
    /// <c>alice.pub</c> are one pair.
    /// </remarks>
    /// <param name="name">The account.</param>
    /// <param name="directory">Where the run is made.</param>
    /// <returns>The public key's path, or nothing when there is no key by that name.</returns>
    public string? PublicKey(string name, string directory)
    {
        var result = rola.Run(["--json", "key"], directory);

        if (!result.Done)
        {
            return null;
        }

        try
        {
            using var document = JsonDocument.Parse(result.Output);

            if (
                document.RootElement.ValueKind is not JsonValueKind.Object
                || !document.RootElement.TryGetProperty("paths", out var paths)
                || paths.ValueKind is not JsonValueKind.Array
            )
            {
                return null;
            }

            return paths
                .EnumerateArray()
                .Select(path => path.GetString())
                .Where(path => path is { Length: > 0 })
                .FirstOrDefault(path =>
                    string.Equals(
                        Path.GetFileNameWithoutExtension(path),
                        name,
                        StringComparison.Ordinal
                    )
                );
        }
        catch (JsonException)
        {
            return null;
        }
    }
}
