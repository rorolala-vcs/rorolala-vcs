namespace RolaSharp;

/// <summary>One operation a Workspace recorded.</summary>
/// <param name="Number">Where it sits in the list, counting from the newest.</param>
/// <param name="Command">The command that made the changes.</param>
/// <param name="Time">When the run ended, in UTC.</param>
/// <param name="Changes">What the run did, in the order it did it.</param>
public sealed record RecordedOperation(
    ulong Number,
    string Command,
    string Time,
    IReadOnlyList<RecordedChange> Changes
);

/// <summary>One change a recorded operation made.</summary>
/// <remarks>
/// The journal states an effect in the layout's own terms, which says more than a reader of the log
/// needs: an entry that gained a record, a path that was bound and a file that was written are three
/// ways of saying something appeared, moved or went. What crosses is the paths and one word for which
/// of those it was, so the reading needs nothing of the journal's shape.
/// </remarks>
/// <param name="Kind">What kind of thing changed: <c>entry</c>, <c>path</c>, <c>uuid</c>, <c>file</c> or <c>file_moved</c>.</param>
/// <param name="From">The path before the change, or the path the change is about.</param>
/// <param name="To">The path after the change, when the change moved something.</param>
/// <param name="State">
/// What became of it: <c>added</c>, <c>removed</c>, <c>changed</c>, <c>bound</c>, <c>unbound</c>,
/// <c>moved</c> or <c>reidentified</c>.
/// </param>
public sealed record RecordedChange(string Kind, string From, string To, string State);
