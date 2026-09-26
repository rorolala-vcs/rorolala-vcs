using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace RorolalaFSAgent;

/// <summary>The one answer the agent prints: the results of a run, as JSON.</summary>
internal static class Json
{
    private static readonly JsonSerializerOptions Options = new()
    {
        // Paths are read by whoever asked for the run, and a path is not HTML: escaping the
        // characters a way of writing prose needs would only make what is printed harder to read.
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
        // `note` is written only where there is one, so a done or a skipped item carries the four
        // fields the example has rather than a fifth that says nothing.
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
    };

    /// <summary>The run's results as one line of JSON.</summary>
    /// <param name="results">One result per item, in order.</param>
    public static string Serialize(IReadOnlyList<Result> results) =>
        JsonSerializer.Serialize(new { results }, Options);
}
