namespace RolaSharp;

/// <summary>One operation a Workspace recorded.</summary>
/// <param name="Number">Where it sits in the list, counting from the newest.</param>
/// <param name="Command">The command that made the changes.</param>
/// <param name="Time">When the run ended, in UTC.</param>
/// <param name="Changes">How many changes it holds.</param>
public sealed record RecordedOperation(ulong Number, string Command, string Time, ulong Changes);
