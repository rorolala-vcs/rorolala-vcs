using System.Net;
using System.Net.Sockets;
using System.Text.RegularExpressions;

namespace RorolalaLauncher.Rola;

/// <summary>
/// What a Vault address is written as, read the way the protocol reads one.
/// </summary>
/// <remarks>
/// An address is read by the command line when it runs, and a window that is being edited has to say
/// whether what is typed is one before anything is run. This is that reading, stated again for a caller
/// that has no command to run yet — and it is deliberately the same reading, because two answers to "is
/// this an address" would be a window that accepts what the command then refuses.
/// <para>
/// What is accepted, and what each is taken to mean:
/// </para>
/// <list type="table">
/// <item><term><c>host</c></term><description>that host, the default port, no namespace</description></item>
/// <item><term><c>port</c></term><description><c>127.0.0.1</c>, that port, no namespace</description></item>
/// <item><term><c>host:port</c></term><description>that host and port, no namespace</description></item>
/// <item><term><c>rola://host:port/ns</c></term><description>all of it</description></item>
/// <item><term><c>host:port/ns</c></term><description>the bare authority, then the namespace</description></item>
/// <item><term><c>user@host:path</c></term><description>the Vault at <c>path</c> on that host, reached over SSH</description></item>
/// <item><term><c>user@host:port:path</c></term><description>the same, on that SSH port</description></item>
/// </list>
/// </remarks>
internal static partial class VaultAddressSyntax
{
    /// <summary>The scheme a full Vault link is written with.</summary>
    private const string Scheme = "rola://";

    /// <summary>
    /// Whether a written address names a daemon.
    /// </summary>
    /// <param name="text">What was written.</param>
    /// <returns>Whether the command line would read it as an address.</returns>
    public static bool Valid(string? text)
    {
        if (string.IsNullOrWhiteSpace(text))
        {
            return false;
        }

        var whole = text.Trim();

        // The scheme is a daemon link and nothing else: an SSH address is not written with one, so
        // anything carrying an account after it is not an address either reading accepts.
        if (whole.StartsWith(Scheme, StringComparison.Ordinal))
        {
            return Daemon(whole[Scheme.Length..]);
        }

        if (IsSession(whole))
        {
            return Session(whole);
        }

        return Daemon(whole);
    }

    /// <summary>
    /// Whether a bare address names a daemon: the first slash separates the authority from what is
    /// named under it.
    /// </summary>
    /// <param name="bare">What was written, without the scheme.</param>
    /// <returns>Whether it names a daemon.</returns>
    private static bool Daemon(string bare)
    {
        var slash = bare.IndexOf('/', StringComparison.Ordinal);
        var authority = slash < 0 ? bare : bare[..slash];

        return Authority(authority);
    }

    /// <summary>
    /// Whether what was written is an SSH address rather than anything else.
    /// </summary>
    /// <remarks>
    /// An account is what says so: no daemon address may carry an <c>@</c>, since a host is written
    /// without one, so one before the first <c>:</c> is the only thing a spelling has to have to be a
    /// session. The first <c>:</c> is what it is compared against rather than the first <c>/</c>.
    /// </remarks>
    /// <param name="text">What was written.</param>
    /// <returns>Whether it is written as a session.</returns>
    private static bool IsSession(string text)
    {
        var at = text.IndexOf('@', StringComparison.Ordinal);

        if (at < 0)
        {
            return false;
        }

        var colon = text.IndexOf(':', StringComparison.Ordinal);

        return colon < 0 || at < colon;
    }

    /// <summary>
    /// Whether an SSH address names a session: the account, the host, an optional port, and a path.
    /// </summary>
    /// <remarks>
    /// The path is the Vault's own and is not read here — it is the far side's to resolve — so what is
    /// checked is that it is a path rather than anything that would end the command it is handed to.
    /// </remarks>
    /// <param name="text">What was written.</param>
    /// <returns>Whether it names a session.</returns>
    private static bool Session(string text)
    {
        var at = text.IndexOf('@', StringComparison.Ordinal);

        if (!IsUser(text[..at]))
        {
            return false;
        }

        return SplitHost(text[(at + 1)..], out var after) && PortPath(after);
    }

    /// <summary>
    /// The host a session reaches, and what follows it.
    /// </summary>
    /// <remarks>
    /// A bracketed ipv6 address is read between the brackets, so the colons it carries are not taken
    /// for the separator that ends the host.
    /// </remarks>
    /// <param name="rest">What follows the account.</param>
    /// <param name="after">What follows the host's separating colon.</param>
    /// <returns>Whether a host was read.</returns>
    private static bool SplitHost(string rest, out string after)
    {
        after = string.Empty;

        if (rest.StartsWith('['))
        {
            var close = rest.IndexOf(']', StringComparison.Ordinal);

            if (close < 0
                || !IPAddress.TryParse(rest[1..close], out var address)
                || address.AddressFamily != AddressFamily.InterNetworkV6
                || close + 1 >= rest.Length
                || rest[close + 1] != ':')
            {
                return false;
            }

            after = rest[(close + 2)..];

            return true;
        }

        var colon = rest.IndexOf(':', StringComparison.Ordinal);

        if (colon < 0 || !Host(rest[..colon]))
        {
            return false;
        }

        after = rest[(colon + 1)..];

        return true;
    }

    /// <summary>
    /// Whether what follows the host is a path, and a port when one was written.
    /// </summary>
    /// <remarks>
    /// Two colons after the host are refused rather than read as one of the two readings: nothing says
    /// which of them is the port, and guessing is what a person writing a path with a colon in it would
    /// be met with.
    /// </remarks>
    /// <param name="rest">What follows the host's separating colon.</param>
    /// <returns>Whether a port, when named, is one, and a path is named.</returns>
    private static bool PortPath(string rest)
    {
        var colon = rest.IndexOf(':', StringComparison.Ordinal);

        if (colon < 0)
        {
            return IsPath(rest);
        }

        var port = rest[..colon];
        var path = rest[(colon + 1)..];

        if (path.Contains(':', StringComparison.Ordinal) || !Digits().IsMatch(port))
        {
            return false;
        }

        // Nothing listens on port 0 in SSH's sense of the word, and a port is written to say which one.
        return ushort.TryParse(port, out var number) && number > 0 && IsPath(path);
    }

    /// <summary>
    /// Whether what was written is an account a session may be started as.
    /// </summary>
    /// <remarks>
    /// The letters, digits, dots, underscores and hyphens a user name is made of, and not a hyphen to
    /// start with: what is written here is taken as the value of an option rather than parsed again.
    /// </remarks>
    /// <param name="user">What was written before the <c>@</c>.</param>
    /// <returns>Whether it is an account.</returns>
    private static bool IsUser(string user) =>
        user.Length > 0 && !user.StartsWith('-') && UserCharacters().IsMatch(user);

    /// <summary>
    /// Whether what was written is a path a session may be asked to serve.
    /// </summary>
    /// <remarks>
    /// It is handed to the far side as an argument to a command, so it cannot carry what would end that
    /// command — a colon, and the newlines — and it cannot start with a hyphen, which the far side's
    /// program would read as a flag.
    /// </remarks>
    /// <param name="path">What was written after the host, or after the port.</param>
    /// <returns>Whether it is a path.</returns>
    private static bool IsPath(string path) =>
        path.Length > 0 && !path.StartsWith('-') && path.IndexOfAny([':', '\0', '\n', '\r']) < 0;

    /// <summary>
    /// Whether an authority names a host and a port, or a host, or a port.
    /// </summary>
    /// <remarks>
    /// A bracketed ipv6 address is read only with the port it is written beside, which is where the
    /// brackets are load-bearing: on its own it is the address the link is not written with.
    /// </remarks>
    /// <param name="authority">What stands before the first slash.</param>
    /// <returns>Whether it names a daemon.</returns>
    private static bool Authority(string authority)
    {
        if (authority.Length == 0)
        {
            return false;
        }

        // A bracketed ipv6 address is read only with the port it is written beside: on its own it is not
        // the address the link is written with, and the brackets are what says whether a port follows.
        if (authority.StartsWith('['))
        {
            return authority.Contains("]:", StringComparison.Ordinal) && IPEndPoint.TryParse(authority, out _);
        }

        // A number that is not a port is not a host either: what a bare number means is a port. It is read
        // before anything else, because a reader of addresses that is asked about `99999` will answer that
        // it is an ipv4 address written in one number, and it is not one this program asks for.
        if (Digits().IsMatch(authority))
        {
            return ushort.TryParse(authority, out _);
        }

        // A host and a port, or an ip on its own, which are the two spellings no name is read as.
        if (IPEndPoint.TryParse(authority, out _) || IPAddress.TryParse(authority, out _))
        {
            return true;
        }

        var colon = authority.LastIndexOf(':');

        if (colon >= 0)
        {
            return ushort.TryParse(authority[(colon + 1)..], out _) && Host(authority[..colon]);
        }

        return Host(authority);
    }

    /// <summary>
    /// Whether a name is a host a daemon could be dialled at.
    /// </summary>
    /// <remarks>
    /// The letters, digits, hyphens, dots and underscores a name or an ip is made of, and nothing else: a
    /// name is kept as it was written rather than resolved, so what is checked is what was said.
    /// </remarks>
    /// <param name="name">What was written.</param>
    /// <returns>Whether it is a host.</returns>
    private static bool Host(string name) => name.Length > 0 && HostCharacters().IsMatch(name);

    /// <summary>Every character a host is written with.</summary>
    [GeneratedRegex("^[A-Za-z0-9._-]+$")]
    private static partial Regex HostCharacters();

    /// <summary>Every character an account is written with.</summary>
    [GeneratedRegex("^[A-Za-z0-9._-]+$")]
    private static partial Regex UserCharacters();

    /// <summary>What a bare port is written with.</summary>
    [GeneratedRegex("^[0-9]+$")]
    private static partial Regex Digits();
}
