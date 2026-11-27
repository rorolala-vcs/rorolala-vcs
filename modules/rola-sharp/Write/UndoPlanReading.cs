namespace RolaSharp.Write;

/// <summary>How one recorded operation stands against the tree.</summary>
public enum UndoStanding
{
    /// <summary>Every change the operation made still stands.</summary>
    Whole = 0,

    /// <summary>Some of them still stand.</summary>
    Part = 1,

    /// <summary>None of them does.</summary>
    None = 2,
}

/// <summary>What a run does with an operation that stands only in part.</summary>
public enum UndoPartial
{
    /// <summary>Leave the operation and the stack below it where they are.</summary>
    Stop = 0,

    /// <summary>Reverse what still stands and go on to the one before it.</summary>
    Continue = 1,
}

/// <summary>One recorded operation a run would undo.</summary>
/// <param name="Command">The command that made the changes.</param>
/// <param name="Standing">How the changes stand.</param>
public sealed record UndoGroupReading(string Command, UndoStanding Standing);

/// <summary>The operations a run would undo, newest first.</summary>
/// <param name="Groups">Each operation the run considered, newest first.</param>
public sealed record UndoPlanReading(IReadOnlyList<UndoGroupReading> Groups);
