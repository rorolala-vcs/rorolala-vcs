namespace RolaSharp;

/// <summary>What binding a store came to.</summary>
/// <param name="BoundWorkspace">Whether a Workspace was bound.</param>
/// <param name="BoundVault">Whether a Vault was bound.</param>
/// <param name="ReverseRecorded">Whether the store recorded the binding back.</param>
/// <param name="ReadOnly">Whether the store can only be read.</param>
/// <param name="Warnings">What the caller should be told, one line each.</param>
public sealed record StorageBound(
    bool BoundWorkspace,
    bool BoundVault,
    bool ReverseRecorded,
    bool ReadOnly,
    IReadOnlyList<string> Warnings
)
{
    /// <summary>Reads the answer the native side hands over.</summary>
    /// <param name="answer">The binding handle, which this takes ownership of.</param>
    /// <returns>What the binding came to.</returns>
    internal static StorageBound From(nint answer)
    {
        var count = RorolalaBinding.rola_write_storage_bind_warning_count(answer);
        var warnings = new List<string>((int)count);
        for (nuint index = 0; index < count; index++)
        {
            var line = NativeText.Taken(
                RorolalaBinding.rola_write_storage_bind_warning_at(answer, index)
            );
            if (line.Length > 0)
            {
                warnings.Add(line);
            }
        }

        return new StorageBound(
            RorolalaBinding.rola_write_storage_bind_bound_workspace(answer),
            RorolalaBinding.rola_write_storage_bind_bound_vault(answer),
            RorolalaBinding.rola_write_storage_bind_reverse_recorded(answer),
            RorolalaBinding.rola_write_storage_bind_read_only(answer),
            warnings
        );
    }
}
