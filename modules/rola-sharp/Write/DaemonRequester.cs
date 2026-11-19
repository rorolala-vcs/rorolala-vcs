using System.Runtime.InteropServices;

using RolaSharp;

namespace RolaSharp.Write;

/// <summary>The daemon actions a Workspace runs against a Vault.</summary>
/// <remarks>
/// Each method is one action and its input is the JSON string the daemon's action reads. The
/// requester does not parse or validate that document: the action's own reader is the one place that
/// knows its shape, and a second copy here would be free to drift.
/// </remarks>
public sealed class DaemonRequester
{
    private readonly WorkspaceScope _workspace;
    private readonly AccountScope _account;
    private readonly string _target;

    /// <summary>Makes a requester for one Vault.</summary>
    /// <param name="workspace">The Workspace the exchange is run from, which must have been read.</param>
    /// <param name="account">The account the exchange signs as.</param>
    /// <param name="target">The Vault's address or name, as the action takes it.</param>
    public DaemonRequester(WorkspaceScope workspace, AccountScope account, string target)
    {
        _workspace = workspace ?? throw new ArgumentNullException(nameof(workspace));
        _account = account ?? throw new ArgumentNullException(nameof(account));
        _target = target ?? throw new ArgumentNullException(nameof(target));
    }

    /// <summary>Runs the handshake action.</summary>
    /// <param name="name">The name the action takes.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? Handshake(string name) => Call(name, RorolalaBinding.do_action_handshake);

    /// <summary>Runs the sync-all action.</summary>
    /// <param name="input">The action's request.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? SyncAll(string input) => Call(input, RorolalaBinding.do_action_sync_all);

    /// <summary>Runs the sync-index-all action.</summary>
    /// <param name="input">The action's request.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? SyncIndexAll(string input) =>
        Call(input, RorolalaBinding.do_action_sync_index_all);

    /// <summary>Runs the list-remote action.</summary>
    /// <param name="input">The action's request.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? ListRemote(string input) =>
        Call(input, RorolalaBinding.do_action_list_remote);

    /// <summary>Runs the sync-hashes action.</summary>
    /// <param name="input">The action's request.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? SyncHashes(string input) =>
        Call(input, RorolalaBinding.do_action_sync_hashes);

    /// <summary>Runs the list-remote-index action.</summary>
    /// <param name="input">The action's request.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? ListRemoteIndex(string input) =>
        Call(input, RorolalaBinding.do_action_list_remote_index);

    /// <summary>Runs the read-remote-index action.</summary>
    /// <param name="input">The action's request.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? ReadRemoteIndex(string input) =>
        Call(input, RorolalaBinding.do_action_read_remote_index);

    /// <summary>Runs the fetch-layout action.</summary>
    /// <param name="input">The action's request.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? FetchLayout(string input) =>
        Call(input, RorolalaBinding.do_action_fetch_layout);

    /// <summary>Runs the request-ownership action.</summary>
    /// <param name="input">The action's request.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? RequestOwnership(string input) =>
        Call(input, RorolalaBinding.do_action_request_ownership);

    /// <summary>Runs the giveup-ownership action.</summary>
    /// <param name="input">The action's request.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? GiveupOwnership(string input) =>
        Call(input, RorolalaBinding.do_action_giveup_ownership);

    /// <summary>Runs the fetch-ignore-rules action.</summary>
    /// <param name="input">The action's request.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? FetchIgnoreRules(string input) =>
        Call(input, RorolalaBinding.do_action_fetch_ignore_rules);

    /// <summary>Runs the remove-remote-path action.</summary>
    /// <param name="input">The action's request.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? RemoveRemotePath(string input) =>
        Call(input, RorolalaBinding.do_action_remove_remote_path);

    /// <summary>Runs the write-remote-entries action.</summary>
    /// <param name="input">The action's request.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? WriteRemoteEntries(string input) =>
        Call(input, RorolalaBinding.do_action_write_remote_entries);

    /// <summary>Runs the bring-remote-index action.</summary>
    /// <param name="input">The action's request.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? BringRemoteIndex(string input) =>
        Call(input, RorolalaBinding.do_action_bring_remote_index);

    /// <summary>Runs the move-remote-path action.</summary>
    /// <param name="from">The path it is at.</param>
    /// <param name="to">The path it is to be at.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? MoveRemotePath(string from, string to) =>
        Call(from, to, RorolalaBinding.do_action_move_remote_path);

    /// <summary>Runs the create-remote-entry action.</summary>
    /// <param name="path">The path to bind.</param>
    /// <param name="uuid">The entry's Uuid.</param>
    /// <param name="version">The version it holds.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? CreateRemoteEntry(string path, string uuid, string version) =>
        Call(path, uuid, version, RorolalaBinding.do_action_create_remote_entry);

    /// <summary>Runs the set-remote-version action.</summary>
    /// <param name="uuid">The entry's Uuid.</param>
    /// <param name="version">The version it is to hold.</param>
    /// <returns>What the Vault answered, or nothing.</returns>
    public string? SetRemoteVersion(string uuid, string version) =>
        Call(uuid, version, RorolalaBinding.do_action_set_remote_version);

    private string? Call(
        string input,
        Func<nint, nint, nint, nint, RorolalaResult> action
    )
    {
        var target = Utf8(_target);
        var text = Utf8(input);
        try
        {
            return Answer(action(_workspace.Handle, _account.Handle, target, text));
        }
        finally
        {
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(text);
        }
    }

    private string? Call(
        string first,
        string second,
        Func<nint, nint, nint, nint, nint, RorolalaResult> action
    )
    {
        var target = Utf8(_target);
        var one = Utf8(first);
        var two = Utf8(second);
        try
        {
            return Answer(action(_workspace.Handle, _account.Handle, target, one, two));
        }
        finally
        {
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(one);
            Marshal.FreeCoTaskMem(two);
        }
    }

    private string? Call(
        string first,
        string second,
        string third,
        Func<nint, nint, nint, nint, nint, nint, RorolalaResult> action
    )
    {
        var target = Utf8(_target);
        var one = Utf8(first);
        var two = Utf8(second);
        var three = Utf8(third);
        try
        {
            return Answer(
                action(_workspace.Handle, _account.Handle, target, one, two, three)
            );
        }
        finally
        {
            Marshal.FreeCoTaskMem(target);
            Marshal.FreeCoTaskMem(one);
            Marshal.FreeCoTaskMem(two);
            Marshal.FreeCoTaskMem(three);
        }
    }

    private static string? Answer(RorolalaResult answer)
    {
        if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
        {
            throw WriteResults.Action(answer.payload);
        }

        var text = NativeText.Taken(answer.payload);

        return text.Length == 0 ? null : text;
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}
