namespace RolaSharp;

using System.Runtime.InteropServices;

/// <summary>Reads the operations a Workspace recorded through the C ABI.</summary>
/// <remarks>
/// The list is copied out of the native side and every handle released before this answers, so what
/// a scope keeps is managed memory and nothing else.
/// </remarks>
internal static class Histories
{
    /// <summary>Reads the operation log of a Workspace.</summary>
    /// <param name="root">The root of the Workspace.</param>
    /// <returns>The operations newest first, and how many files could not be read.</returns>
    /// <exception cref="RolaReadError">The Workspace has no operation log this side can read.</exception>
    public static (IReadOnlyList<RecordedOperation> Operations, int Unreadable) Read(string root)
    {
        var workspace = Marshal.StringToCoTaskMemUTF8(root);
        try
        {
            var answer = RorolalaBinding.rola_history_operations(workspace);
            if (answer == 0)
            {
                throw new RolaReadError(
                    RolaReadErrorKind.Unavailable,
                    root,
                    "the Workspace has no operation log to read"
                );
            }

            try
            {
                return (OperationsIn(answer), Count(answer));
            }
            finally
            {
                RorolalaBinding.free_rola_history_operations(answer);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(workspace);
        }
    }

    /// <summary>The operations the answer holds.</summary>
    private static List<RecordedOperation> OperationsIn(nint answer)
    {
        var operations = new List<RecordedOperation>();
        var count = RorolalaBinding.rola_history_operations_count(answer);
        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_history_operations_at(answer, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                operations.Add(
                    new RecordedOperation(
                        RorolalaBinding.rola_history_operation_number(item),
                        NativeText.Taken(RorolalaBinding.rola_history_operation_command(item)),
                        NativeText.Taken(RorolalaBinding.rola_history_operation_time(item)),
                        ChangesIn(item)
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_history_operation(item);
            }
        }

        return operations;
    }

    /// <summary>What one operation did, in the order it did it.</summary>
    private static List<RecordedChange> ChangesIn(nint operation)
    {
        var changes = new List<RecordedChange>();
        var count = RorolalaBinding.rola_history_operation_changes(operation);
        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_history_operation_change(operation, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                changes.Add(
                    new RecordedChange(
                        NativeText.Taken(RorolalaBinding.rola_history_change_kind(item)),
                        NativeText.Taken(RorolalaBinding.rola_history_change_from(item)),
                        NativeText.Taken(RorolalaBinding.rola_history_change_to(item)),
                        NativeText.Taken(RorolalaBinding.rola_history_change_state(item))
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_history_change(item);
            }
        }

        return changes;
    }

    /// <summary>How many files could not be read, as a count this side can hold.</summary>
    private static int Count(nint answer)
    {
        var unreadable = RorolalaBinding.rola_history_operations_unreadable_count(answer);

        return unreadable > int.MaxValue ? int.MaxValue : (int)unreadable;
    }
}
