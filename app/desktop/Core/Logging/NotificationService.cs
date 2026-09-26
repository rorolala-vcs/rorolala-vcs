using System.Security.Cryptography;
using System.Text;

namespace RorolalaDesktop.Logging;

/// <summary>
/// One place that decides whether a notification has been seen before.
/// </summary>
/// <remarks>
/// Both popups and the Log dock consult this, so there is one deduplication rule rather than two. A
/// notification is identified by a hash of its combined content — level, source, message, and the
/// values already formatted into it — so content that differs by even a little is a distinct
/// notification and is not suppressed. Distinctness is scoped to one session: a restart begins with
/// an empty table, and because a restart is also how plugins are reloaded, that is enough.
/// </remarks>
internal sealed class NotificationService
{
    /// <summary>How often each distinct notification has been seen.</summary>
    private readonly Dictionary<string, int> _counts = new(StringComparer.Ordinal);

    /// <summary>
    /// The identity of a notification: a hash of its combined content.
    /// </summary>
    /// <param name="level">The level it was said at.</param>
    /// <param name="source">The plugin or kernel part that said it.</param>
    /// <param name="message">What was said, with the values already formatted into it.</param>
    /// <returns>An opaque key naming this exact content.</returns>
    public static string Key(LogLevel level, string source, string message)
    {
        var content = string.Concat(level.ToString(), "\u0000", source, "\u0000", message);

        return Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(content)));
    }

    /// <summary>
    /// Counts a notification and says how many times its content has now been seen.
    /// </summary>
    /// <param name="key">The identity from <see cref="Key"/>.</param>
    /// <returns>The count after this call; <c>1</c> means this is the first time.</returns>
    public int Note(string key)
    {
        _counts.TryGetValue(key, out var count);
        count += 1;
        _counts[key] = count;

        return count;
    }
}
