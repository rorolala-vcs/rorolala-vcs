namespace RolaSharp;

/// <summary>
/// What Rorolala keeps for the user rather than for a Vault or a Workspace.
/// </summary>
/// <remarks>
/// These belong to no Vault and no Workspace: they are the user's own, and they live under the
/// machine's local data directory. A machine that does not name one has nowhere to keep them, which
/// is why what is answered here can be empty.
/// </remarks>
public static class RolaUser
{
    /// <summary>The account the work acts as, or an empty string when none is named.</summary>
    /// <returns>The account name, as <c>rola account</c> writes it.</returns>
    public static string CurrentAccount() => NativeText.Taken(RorolalaBinding.rola_current_account());
}
