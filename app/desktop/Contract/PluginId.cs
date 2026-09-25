using System.Text.RegularExpressions;

namespace RorolalaDesktop.Contract;

/// <summary>
/// The stable identity of a plugin: a dotted snake_case string, such as <c>rorolala.file_system</c>.
/// </summary>
/// <remarks>
/// The identity is permanent. It is written into <c>plugins.json</c> and <c>preference.json</c> as a
/// key, into dock layout as the head of every dock name id, and as the prefix of every translation
/// key the plugin states, so changing it would orphan a user's configuration and their layout. Two
/// identities name two plugins, and one is never used for two.
/// </remarks>
/// <param name="Value">The identity, as the grammar spells it.</param>
public readonly record struct PluginId(string Value)
{
    /// <summary>
    /// What a well-formed identity is: one or more snake_case segments, dot-separated, each opening
    /// with a letter.
    /// </summary>
    private static readonly Regex Grammar = new(
        "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)*$",
        RegexOptions.CultureInvariant
    );

    /// <summary>
    /// Whether a string is a well-formed identity.
    /// </summary>
    /// <param name="value">The string to read, which may be nothing.</param>
    /// <returns>Whether it could be an identity.</returns>
    public static bool IsWellFormed(string? value) => value is not null && Grammar.IsMatch(value);

    /// <summary>
    /// The prefix this plugin's translation keys carry, which is the identity with dots underscored.
    /// </summary>
    /// <remarks>
    /// The prefix is a writing convention that keeps two plugins' keys apart; it is not the merge
    /// mechanism, which is first-registration-wins (Section 11).
    /// </remarks>
    public string KeyPrefix => Value.Replace('.', '_');

    /// <summary>The identity as it is written.</summary>
    public override string ToString() => Value;
}
