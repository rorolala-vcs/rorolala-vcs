using System.Runtime.InteropServices;

namespace RolaSharp;

/// <summary>A Layout: the record a Workspace or a Vault works from.</summary>
public class LayoutScope : Scope
{
    private readonly string _root;
    private readonly string? _name;
    private IReadOnlyList<LayoutEntry> _entries = [];

    /// <summary>Makes a scope over a Layout a place holds.</summary>
    /// <param name="root">A directory inside the Workspace or the Vault.</param>
    /// <param name="name">
    /// The Layout's name as the command line names it, or nothing for the one being worked in.
    /// </param>
    public LayoutScope(string root, string? name = null)
    {
        _root = Path.GetFullPath(root);
        _name = name;
    }

    /// <summary>The directory the Layout's place is rooted in.</summary>
    public DirectoryInfo Root => new(_root);

    /// <summary>The name the Layout is read by, or nothing for the one being worked in.</summary>
    public string? Name => _name;

    /// <summary>What the Layout holds, in path order and then Uuid order.</summary>
    public IReadOnlyList<LayoutEntry> Entries
    {
        get
        {
            if (!Loaded)
            {
                throw new InvalidOperationException("call Read first");
            }

            return _entries;
        }
    }

    /// <summary>The entry at a path, when the Layout names one.</summary>
    /// <param name="path">The path to look for.</param>
    /// <returns>The entry, or nothing.</returns>
    public LayoutEntry? Find(string path)
    {
        ArgumentNullException.ThrowIfNull(path);

        return Entries.FirstOrDefault(entry => entry.Path == path);
    }

    /// <inheritdoc/>
    protected override void IsValid()
    {
        if (!Directory.Exists(_root))
        {
            throw new RolaReadError(
                RolaReadErrorKind.NoWorkspace,
                _root,
                "the place holding the Layout is not there"
            );
        }
    }

    /// <inheritdoc/>
    protected override void Load(CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();

        var directory = Utf8(_root);
        var name = _name is null ? 0 : Utf8(_name);
        try
        {
            var answer = RorolalaBinding.rola_layout_entries(directory, name);
            if (answer.tag is RorolalaResultTag.RorolalaResult_Err)
            {
                throw ReadResults.Layout(answer.payload);
            }

            var handle = answer.payload;
            if (handle == 0)
            {
                _entries = [];
                return;
            }

            try
            {
                _entries = Read(handle);
            }
            finally
            {
                RorolalaBinding.free_rola_layout_entries(handle);
            }
        }
        finally
        {
            Marshal.FreeCoTaskMem(directory);
            if (name != 0)
            {
                Marshal.FreeCoTaskMem(name);
            }
        }
    }

    private static IReadOnlyList<LayoutEntry> Read(nint handle)
    {
        var count = RorolalaBinding.rola_layout_entries_count(handle);
        var entries = new List<LayoutEntry>();

        for (nuint index = 0; index < count; index++)
        {
            var item = RorolalaBinding.rola_layout_entries_at(handle, index);
            if (item == 0)
            {
                continue;
            }

            try
            {
                var uuid = NativeText.Taken(RorolalaBinding.rola_layout_entry_uuid(item));
                if (string.IsNullOrEmpty(uuid))
                {
                    continue;
                }

                entries.Add(
                    new LayoutEntry(
                        NativeText.Taken(RorolalaBinding.rola_layout_entry_path(item)),
                        uuid,
                        NativeText.Taken(RorolalaBinding.rola_layout_entry_version(item)),
                        NativeText.Taken(RorolalaBinding.rola_layout_entry_owner(item)),
                        NativeText.Taken(RorolalaBinding.rola_layout_entry_description(item)),
                        NativeText.Taken(RorolalaBinding.rola_layout_entry_preference(item)),
                        RorolalaBinding.rola_layout_entry_deprecated(item)
                    )
                );
            }
            finally
            {
                RorolalaBinding.free_rola_layout_entry(item);
            }
        }

        return entries;
    }

    private static nint Utf8(string text) => Marshal.StringToCoTaskMemUTF8(text);
}

/// <summary>The Layout a Workspace works in.</summary>
public sealed class LocalLayoutScope : LayoutScope
{
    /// <summary>Makes a scope over a Workspace's one Layout.</summary>
    /// <param name="root">A directory inside the Workspace.</param>
    public LocalLayoutScope(string root)
        : base(root) { }
}

/// <summary>A Vault's truth Layout, or the copy a fetch left beside a Workspace.</summary>
public sealed class TruthLayoutScope : LayoutScope
{
    /// <summary>Makes a scope over a truth Layout.</summary>
    /// <param name="root">A directory inside the Vault, or the Workspace holding a copy.</param>
    /// <param name="vault">The Vault whose copy is read, or nothing for the Vault itself.</param>
    public TruthLayoutScope(string root, string? vault = null)
        : base(root, vault is null ? "truth" : $"truth@{vault}") { }
}
