using System.Text.Json;

namespace RorolalaVCSPlugin;

/// <summary>
/// A command's own account of a failure, as the command line writes one when it is asked for a structural answer.
/// </summary>
/// <remarks>
/// The shape is <c>{"name": ..., "reason": ...}</c>: the name is what the failure is, untranslated, and the reason
/// is what it says. It is not a result — nothing in it names what was asked for — so a caller that reads results
/// reads this one first.
/// <para>
/// A status is how a run's failure is usually known, and it is what the plugin reads: a command that failed says so
/// with its status and its error stream. This exists for the case where neither says it — a failure written as data
/// instead, which is what asking a run for <c>--json</c> makes of one — so that a run which answered with a failure
/// is not read as a run that answered with nothing.
/// </para>
/// </remarks>
/// <param name="Name">The name the failure is known by.</param>
/// <param name="Reason">What it says, in the command line's own words.</param>
internal sealed record FailureReport(string Name, string Reason)
{
    /// <summary>
    /// The status a run that answered with a failure is reported as.
    /// </summary>
    /// <remarks>
    /// Nothing branches on the number: a caller shows the reason when the status is not zero. It is negative so that
    /// it cannot be mistaken for a status the command line itself states, and told apart from
    /// <see cref="Sync.FailedToRun"/> because the command did run.
    /// </remarks>
    public const int Code = -2;

    /// <summary>What `output` says the failure was, or nothing when it does not say one.</summary>
    /// <remarks>
    /// The two fields together are what the shape is recognized by. Every result this plugin reads carries a member
    /// of its own — <c>entries</c>, <c>layouts</c>, <c>vaults</c>, a plan's <c>layout</c> — and no result carries both
    /// of these, so a document holding both is a failure and one holding either is not.
    /// </remarks>
    /// <param name="output">What the command wrote on its output stream.</param>
    /// <returns>The failure, or nothing when the output is not one.</returns>
    public static FailureReport? Read(string? output)
    {
        if (string.IsNullOrWhiteSpace(output))
        {
            return null;
        }

        try
        {
            using var document = JsonDocument.Parse(output);

            if (
                document.RootElement.ValueKind is not JsonValueKind.Object
                || Text(document.RootElement, "name") is not { Length: > 0 } name
                || Text(document.RootElement, "reason") is not { Length: > 0 } reason
            )
            {
                return null;
            }

            return new FailureReport(name, reason);
        }
        catch (JsonException)
        {
            // What is not JSON is not a failure written as data: a run that wrote something else is answered by
            // whatever else it said.
            return null;
        }
    }

    /// <summary>The string a member of an object holds, or nothing when it holds none.</summary>
    /// <param name="item">The object.</param>
    /// <param name="name">The member's name.</param>
    /// <returns>The text, or nothing.</returns>
    private static string? Text(JsonElement item, string name) =>
        item.ValueKind is JsonValueKind.Object
        && item.TryGetProperty(name, out var value)
        && value.ValueKind is JsonValueKind.String
            ? value.GetString()
            : null;
}
