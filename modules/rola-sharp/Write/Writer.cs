namespace RolaSharp.Write;

/// <summary>A write over a scope a caller has already read.</summary>
/// <remarks>
/// A writer does not load anything itself: the scope it is handed is the thing that was read, and
/// the writer is the action taken against it. <see cref="NoCheck"/> is the writer's own switch, and
/// skips only the cheap check this side makes before calling the native write.
/// </remarks>
public abstract class Writer
{
    /// <summary>Whether a write skips the validation before it.</summary>
    public bool NoCheck { get; set; }
}
