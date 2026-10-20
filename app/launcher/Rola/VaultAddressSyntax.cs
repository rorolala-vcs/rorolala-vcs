using System.Net;
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
        var bare = whole.StartsWith(Scheme, StringComparison.Ordinal) ? whole[Scheme.Length..] : whole;
        var slash = bare.IndexOf('/', StringComparison.Ordinal);
        var authority = slash < 0 ? bare : bare[..slash];

        return Authority(authority);
    }

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

    /// <summary>What a bare port is written with.</summary>
    [GeneratedRegex("^[0-9]+$")]
    private static partial Regex Digits();
}
