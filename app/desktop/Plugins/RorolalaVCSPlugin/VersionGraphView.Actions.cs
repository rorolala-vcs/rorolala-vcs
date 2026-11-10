using Avalonia.Controls;
using Avalonia.Input.Platform;
using Avalonia.Threading;
using FileSystemPlugin;
using RolaSharp;
using RorolalaDesktop.Contract;
using RorolalaDesktop.I18n;

using MenuItem = Avalonia.Controls.MenuItem;
using Separator = Avalonia.Controls.Separator;

namespace RorolalaVCSPlugin;

/// <summary>The menu half of the view: what a right-click offers, and the runs it begins.</summary>
internal sealed partial class VersionGraphView
{
    /// <summary>Offers what a right-click on the thing under the pointer can do.</summary>
    /// <remarks>
    /// A card offers what can be done with the file it stands for, copying its hashes, moving the
    /// record, joining an unrecorded variant, ending a merge. A pill, the message and the creator offer
    /// what that one thing says, copied: the whole of it, which is what a pill cannot show.
    /// </remarks>
    /// <param name="hover">What the pointer is on.</param>
    private void ShowMenu(Hover hover)
    {
        var menu = new ContextMenu { Placement = PlacementMode.Pointer };

        switch (hover.Part)
        {
            case HoverPart.EntryPill when hover.Chip is { Entry: { } entry } chip:
                Add(menu, "copy_uuid", Icons.ContentCopy, () => Copy(entry.Uuid));
                Add(menu, "copy_path", Icons.ContentCopy, () => Copy(entry.Path));

                if (entry.Version is { Length: > 0 } version)
                {
                    Add(menu, "copy_version_hash", Icons.ContentCopy, () => Copy(version));
                }

                Add(menu, "copy_variant_hash", Icons.ContentCopy, () => Copy(hover.Card!.Hash));

                // A local entry is the record a file is kept under, so a fork of it is a new identity
                // for the same record: the path goes on holding the version it holds, under a Uuid of
                // its own rather than the one it was following.
                if (
                    entry.Side == "local"
                    && entry.Path.Length > 0
                    && _root is { Length: > 0 } root
                )
                {
                    menu.Items.Add(new Separator());
                    Add(
                        menu,
                        "fork_uuid",
                        Icons.CallSplit,
                        () => BeginForkUuid(root, entry.Path)
                    );
                }

                // A file of the work may be made to follow another side's entry, which is a retrack
                // with the entry as its target: the record moves and the content follows it where the
                // two versions say so. The second item moves the record alone, leaving the file as
                // it stands, which is what a reader who has edited it usually wants.
                if (entry.Side != "local" && entry.Uuid.Length > 0)
                {
                    var file = SwitchFile(hover.Card!, entry.Path);

                    if (
                        file is not null
                        && !string.Equals(
                            LocalAt(hover.Card!, entry.Path)?.Uuid,
                            entry.Uuid,
                            StringComparison.OrdinalIgnoreCase
                        )
                    )
                    {
                        menu.Items.Add(new Separator());
                        Add(
                            menu,
                            "switch_uuid",
                            Icons.ArrowRightAlt,
                            () => AskSwitch(file, entry.Uuid)
                        );
                        Add(
                            menu,
                            "switch_uuid_keep",
                            Icons.ArrowRightAlt,
                            () => BeginSwitch(file, entry.Uuid, onlyIndex: true, force: false)
                        );
                    }
                }

                menu.Open(this);

                return;

            case HoverPart.FilePill when hover.Card?.Draft is { } draft:
                Add(menu, "copy_path", Icons.ContentCopy, () => Copy(draft.Path));
                menu.Open(this);

                return;

            case HoverPart.Message when hover.Card is { } said:
                Add(
                    menu,
                    "copy_message",
                    Icons.ContentCopy,
                    () => Copy(said.Message),
                    enabled: said.Message.Length > 0
                );
                menu.Open(this);

                return;

            case HoverPart.Creator when hover.Card is { } maker:
                Add(menu, "copy_creator_name", Icons.ContentCopy, () => Copy(maker.Creator));
                Add(
                    menu,
                    "copy_creator_hash",
                    Icons.ContentCopy,
                    () => Copy(maker.CreatorHash),
                    enabled: maker.CreatorHash.Length > 0
                );
                menu.Open(this);

                return;

            case HoverPart.HashPill when hover.Card is { } named:
                VariantMenu(menu, named);
                menu.Open(this);

                return;

            case HoverPart.Number when hover.Card is { } numbered && numbered.HasVersion:
                Add(menu, "copy_version_hash", Icons.ContentCopy, () => Copy(numbered.VersionHash!));
                Add(menu, "copy_variant_hash", Icons.ContentCopy, () => Copy(numbered.Hash));
                menu.Open(this);

                return;
        }

        if (hover.Card is { } card)
        {
            NodeMenu(menu, card);
            menu.Open(this);
        }
    }

    /// <summary>Offers what can be done with a card's file.</summary>
    /// <param name="menu">The menu to add to.</param>
    /// <param name="card">The card.</param>
    private void NodeMenu(ContextMenu menu, GraphCard card)
    {
        if (card.Kind == GraphCardKind.Draft && card.Draft is { } draft)
        {
            if (DraftFile(draft) is { } file)
            {
                menu.Items.Add(DeleteChangesItem(file));
                menu.Items.Add(new Separator());
            }

            Add(menu, "copy_path", Icons.ContentCopy, () => Copy(draft.Path));

            return;
        }

        VariantMenu(menu, card);
    }

    /// <summary>Offers what a right-click on a variant or on the hash that names it can do.</summary>
    /// <remarks>
    /// A card and the variant hash pill on it name one variant, so they offer one menu: the items that
    /// copy what the variant says, the join and its two ends, and the retracks the chosen file may take
    /// to the version it produced.
    /// </remarks>
    /// <param name="menu">The menu to add to.</param>
    /// <param name="card">The variant.</param>
    private void VariantMenu(ContextMenu menu, GraphCard card)
    {
        Add(
            menu,
            "copy_variant_hash",
            Icons.ContentCopy,
            () => Copy(card.Hash),
            enabled: card.Hash.Length > 0
        );

        if (card.HasVersion)
        {
            Add(menu, "copy_version_hash", Icons.ContentCopy, () => Copy(card.VersionHash!));
        }

        Add(menu, "copy_content_hash", Icons.ContentCopy, () => Copy(card.Storage));

        if (card.Message.Length > 0)
        {
            Add(menu, "copy_message", Icons.ContentCopy, () => Copy(card.Message));
        }

        // A variant may be joined into the file the user is on. The press is on the variant being
        // merged in, so the node and the hash pill both offer it; without a chosen file, a card with an
        // unrecorded variant of its own is what the join can stand on.
        if (Subject() is { } selected)
        {
            if (PendingMerge(selected.Path) is null)
            {
                menu.Items.Add(new Separator());
                Add(
                    menu,
                    "join_selected",
                    Icons.Merge,
                    () => BeginJoin(selected.Path, card.Hash)
                );
            }
        }
        else if (Joinable(card))
        {
            menu.Items.Add(new Separator());
            Add(menu, "join_variant", Icons.Merge, () => BeginJoin(TargetFor(card)!, card.Hash));
        }

        // A merge waiting on a file is ended by taking its variant file away and then its record, which
        // is the two runs `align --delete` is built from.
        if (PendingOf(card) is { } pending)
        {
            var actual = VariantPath(pending.Root, VariantFiles.Place(pending.Merge));
            var recorded = VariantPath(pending.Root, pending.Merge.Path);

            if (actual is not null && recorded is not null)
            {
                menu.Items.Add(new Separator());
                menu.Items.Add(
                    CancelMergeItem(pending.Path, pending.Directory, actual, recorded, keep: false)
                );
                menu.Items.Add(
                    CancelMergeItem(pending.Path, pending.Directory, actual, recorded, keep: true)
                );
            }
        }

        // A retrack is about a version, and the file it is written to stands beside the card: a variant
        // only the Vault has may be gone to as well, which is a jump.
        if (card.HasVersion && TargetFor(card) is { } target)
        {
            menu.Items.Add(new Separator());
            menu.Items.Add(
                RetrackItem(
                    target,
                    card.VersionHash!,
                    VariantGraphDrawing.VersionLabel(card),
                    onlyIndex: false
                )
            );
            menu.Items.Add(
                RetrackItem(
                    target,
                    card.VersionHash!,
                    VariantGraphDrawing.VersionLabel(card),
                    onlyIndex: true
                )
            );
        }
    }

    /// <summary>Adds one item to a menu, drawn with a picture when the library has it.</summary>
    /// <param name="menu">The menu.</param>
    /// <param name="key">The i18n key its words come from.</param>
    /// <param name="icon">The picture's key.</param>
    /// <param name="action">What pressing it does.</param>
    /// <param name="enabled">Whether it may be pressed.</param>
    private void Add(ContextMenu menu, string key, string icon, Action action, bool enabled = true)
    {
        var item = new MenuItem
        {
            Header = RolaI18N.Get($"rorolala_vcs.version_graph.{key}"),
            IsEnabled = enabled,
        };

        if (_icons.Find(icon) is { } picture)
        {
            item.Icon = Icons.Inked(picture);
        }

        item.Click += (_, _) => action();
        menu.Items.Add(item);
    }

    /// <summary>The item that drops the file's unrecorded change.</summary>
    /// <param name="file">The file, as the program names it.</param>
    /// <returns>The item, ready to be added to a menu.</returns>
    private MenuItem DeleteChangesItem(string file)
    {
        var item = new MenuItem
        {
            Header = RolaI18N.Get("rorolala_vcs.version_graph.delete_changes"),
        };

        if (_icons.Find(Icons.Refresh) is { } picture)
        {
            item.Icon = Icons.Inked(picture);
        }

        item.Click += (_, _) => AskDeleteChanges(file);

        return item;
    }

    /// <summary>One of the two items that end a merge, which differ in what the file is left holding.</summary>
    /// <param name="file">The file the merge is waiting on, as the program names it.</param>
    /// <param name="directory">The directory the runs of the item are made in.</param>
    /// <param name="variant">Where the tree holds the variant file.</param>
    /// <param name="recorded">Where the record says the variant file lies.</param>
    /// <param name="keep">Whether the file's content is kept rather than put back.</param>
    /// <returns>The item, ready to be added to a menu.</returns>
    private MenuItem CancelMergeItem(
        string file,
        string directory,
        string variant,
        string recorded,
        bool keep
    )
    {
        var item = new MenuItem
        {
            Header = RolaI18N.Get(
                keep
                    ? "rorolala_vcs.version_graph.cancel_merge_keep"
                    : "rorolala_vcs.version_graph.cancel_merge"
            ),
        };

        if (_icons.Find(keep ? Icons.EditNote : Icons.CallSplit) is { } picture)
        {
            item.Icon = Icons.Inked(picture);
        }

        item.Click += (_, _) => AskCancelMerge(file, directory, variant, recorded, keep);

        return item;
    }

    /// <summary>One of the two retrack items, which differ in whether the file on disk follows the record.</summary>
    /// <param name="file">The file the record is of.</param>
    /// <param name="version">The version the record is to be moved to.</param>
    /// <param name="label">What the version is called in the question.</param>
    /// <param name="onlyIndex">Whether the file on disk is left alone.</param>
    /// <returns>The item, ready to be added to a menu.</returns>
    private MenuItem RetrackItem(string file, string version, string label, bool onlyIndex)
    {
        var item = new MenuItem
        {
            Header = RolaI18N.Get(
                onlyIndex
                    ? "rorolala_vcs.version_graph.retrack_only_index"
                    : "rorolala_vcs.version_graph.retrack"
            ),
        };

        if (_icons.Find(onlyIndex ? Icons.ArrowRightAlt : Icons.EditNote) is { } picture)
        {
            item.Icon = Icons.Inked(picture);
        }

        item.Click += (_, _) => AskRetrack(file, version, label, onlyIndex);

        return item;
    }

    /// <summary>Whether a card is a variant a join may still be made of.</summary>
    /// <param name="card">The card.</param>
    /// <returns>Whether the join item is offered.</returns>
    private bool Joinable(GraphCard card) =>
        card.Kind == GraphCardKind.Variant
        && !card.HasVersion
        && !card.Pending
        && card.Join is null
        && card.IncomingJoins == 0
        && card.Entries.Count == 0
        && TargetFor(card) is { } file
        && PendingMerge(file) is null;

    /// <summary>The local entry at the same path as an entry on another side, when the card has one.</summary>
    /// <param name="card">The card the two entries hang under.</param>
    /// <param name="path">The path the other side's entry is at.</param>
    /// <returns>The local entry, or nothing when this card's local entry is at another path.</returns>
    private static VariantGraphEntry? LocalAt(GraphCard card, string path)
    {
        var wanted = VariantGraphDrawing.Normalize(path);

        return card.Entries.FirstOrDefault(entry =>
            entry.Side == "local"
            && entry.Path.Length > 0
            && string.Equals(
                VariantGraphDrawing.Normalize(entry.Path),
                wanted,
                StringComparison.Ordinal
            )
        );
    }

    /// <summary>The file whose record another side's entry is to become.</summary>
    /// <remarks>
    /// The file the user is on is what a switch is about, it is the one whose entry is being changed,
    /// and the entry pressed may be at another path altogether. A card's own local file is the answer
    /// when no file is chosen, which is what makes the item reachable from the graph alone.
    /// </remarks>
    /// <param name="card">The card the other side's entry hangs under.</param>
    /// <param name="path">The path the other side's entry is at.</param>
    /// <returns>The file, as the program names it, or nothing when neither a choice nor the card names one.</returns>
    private string? SwitchFile(GraphCard card, string path)
    {
        if (Subject() is { } chosen)
        {
            return chosen.Path;
        }

        if (LocalAt(card, path) is { } local && _root is { Length: > 0 } root)
        {
            return Path.Combine(root, local.Path);
        }

        return null;
    }

    /// <summary>Puts the switching of an edited file's record to the user, since content may be lost.</summary>
    /// <remarks>
    /// A file with unrecorded changes is refused by a retrack that would write over it: what is asked is
    /// whether the content is to be restored to the entry's version anyway, or the record is to move
    /// with the file left as it stands. The third answer does nothing.
    /// </remarks>
    /// <param name="file">The file whose record is to change.</param>
    /// <param name="uuid">The entry the record is to become.</param>
    private void AskSwitch(string file, string uuid)
    {
        if (!Edited(file))
        {
            BeginSwitch(file, uuid, onlyIndex: false, force: false);

            return;
        }

        _dialogs.Choose(
            new Choice(
                RolaI18N.Get("rorolala_vcs.version_graph.switch_uuid_confirm_title"),
                RolaI18N.Get(
                    "rorolala_vcs.version_graph.switch_uuid_confirm_message",
                    Path.GetFileName(file)
                ),
                [
                    new DialogChoice(
                        RolaI18N.Get("rorolala_vcs.version_graph.switch_uuid_restore"),
                        () => BeginSwitch(file, uuid, onlyIndex: false, force: true)
                    ),
                    new DialogChoice(
                        RolaI18N.Get("rorolala_vcs.version_graph.switch_uuid_keep_choice"),
                        () => BeginSwitch(file, uuid, onlyIndex: true, force: false)
                    ),
                    new DialogChoice(
                        RolaI18N.Get("rorolala_vcs.version_graph.switch_uuid_cancel"),
                        () => { }
                    ),
                ]
            )
        );
    }

    /// <summary>Whether a file holds a change that a content switch would write over.</summary>
    /// <remarks>
    /// A merge waiting on the file is one too: a retrack that switches content refuses it as well, and
    /// the same agreement is what goes past both.
    /// </remarks>
    /// <param name="file">The file, as the program names it.</param>
    /// <returns>Whether unrecorded work stands in the way.</returns>
    private bool Edited(string file)
    {
        if (
            Path.GetDirectoryName(file) is not { Length: > 0 } directory
            || _statuses.Reading(directory) is not { } status
        )
        {
            return false;
        }

        try
        {
            return status.StateOf(file) is PathState.Modified or PathState.Wrong or PathState.Merging;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Warn($"how `{file}` stands could not be read: {error.Message}");

            return false;
        }
    }

    /// <summary>Forks the entry a path records into a new Uuid, keeping what it holds.</summary>
    /// <param name="directory">The directory the run is made in.</param>
    /// <param name="path">The path in the Layout being worked in.</param>
    private void BeginForkUuid(string directory, string path)
    {
        var run = _hub.Begin("rorolala_vcs.progress.fork_uuid");

        _ = Task.Run(() =>
        {
            var (done, said) = LayoutPathNewUuidCommand.Run(directory, path);

            Dispatcher.UIThread.Post(() =>
            {
                _hub.End(run);
                _files.Touch();

                if (!done)
                {
                    FailureNotice.Show(
                        _dialogs,
                        _icons,
                        RolaI18N.Get("rorolala_vcs.version_graph.fork_uuid_failed"),
                        said
                    );

                    return;
                }

                Refresh();
            });
        });
    }

    /// <summary>Switches the entry a file records to the one another side names.</summary>
    /// <param name="file">The file whose record is to change.</param>
    /// <param name="uuid">The entry the record is to become.</param>
    /// <param name="onlyIndex">Whether the record moves and the file on disk is left alone.</param>
    /// <param name="force">Whether unrecorded changes are to be discarded.</param>
    private void BeginSwitch(string file, string uuid, bool onlyIndex, bool force)
    {
        if (Path.GetDirectoryName(file) is not { Length: > 0 } directory)
        {
            return;
        }

        var run = _hub.Begin("rorolala_vcs.progress.switch_uuid");

        _ = Task.Run(() =>
        {
            var (done, said) = RetrackCommand.Run(directory, file, uuid, onlyIndex, force);

            Dispatcher.UIThread.Post(() =>
            {
                _hub.End(run);
                _files.Touch();

                if (!done)
                {
                    FailureNotice.Show(
                        _dialogs,
                        _icons,
                        RolaI18N.Get("rorolala_vcs.version_graph.switch_uuid_failed"),
                        said
                    );

                    return;
                }

                Refresh();

                // The entry read for the switch may have come from the Vault's Layout, and a fetch is
                // what keeps the copy read from here in step with it.
                if (!_sync.Running)
                {
                    _fetch.Fetch(directory, Refresh);
                }
            });
        });
    }

    /// <summary>The file a card stands for, as the program names it.</summary>
    /// <param name="card">The card.</param>
    /// <returns>The file, or nothing when neither the card nor the choice names one.</returns>
    private string? TargetFor(GraphCard card)
    {
        if (
            _root is { Length: > 0 } root
            && card.Entries.FirstOrDefault(entry =>
                entry.Side == "local" && !entry.Deprecated
            ) is { } entry
        )
        {
            return Path.Combine(root, entry.Path);
        }

        return _target;
    }

    /// <summary>The file a draft belongs to, as the program names it.</summary>
    /// <param name="draft">The draft.</param>
    /// <returns>The file, or nothing when the Workspace's root is not known.</returns>
    private string? DraftFile(DraftInfo draft) =>
        _root is { Length: > 0 } root ? Path.Combine(root, draft.Path) : null;

    /// <summary>The merge in progress the card stands for, with the file it waits on.</summary>
    /// <param name="card">The card.</param>
    /// <returns>The merge, the file and where the reading is rooted, or nothing.</returns>
    private (string Path, string Directory, Merge Merge, string Root)? PendingOf(GraphCard card)
    {
        if (
            !card.Pending
            || _root is not { Length: > 0 } root
            || card.PendingPath is not { Length: > 0 } place
            || Path.Combine(root, place) is not { Length: > 0 } file
            || Path.GetDirectoryName(file) is not { Length: > 0 } directory
            || _statuses.Reading(directory) is not { } status
        )
        {
            return null;
        }

        var merge = status.Merging.FirstOrDefault(one =>
            StatusReadings.Names(status.Root, one.Target, file)
        );

        return merge is null ? null : (file, directory, merge, root);
    }

    /// <summary>The merge in progress waiting on a file, when one is.</summary>
    /// <param name="file">The file, as the program names it.</param>
    /// <returns>The merge, or nothing.</returns>
    private Merge? PendingMerge(string file)
    {
        if (
            Path.GetDirectoryName(file) is not { Length: > 0 } directory
            || _statuses.Reading(directory) is not { } status
        )
        {
            return null;
        }

        return status.Merging.FirstOrDefault(one =>
            StatusReadings.Names(status.Root, one.Target, file)
        );
    }

    /// <summary>Puts the dropping of an edit to the user, since nothing else holds its content.</summary>
    /// <param name="file">The file, as the program names it.</param>
    private void AskDeleteChanges(string file) =>
        _dialogs.Show(
            new Dialog(
                RolaI18N.Get("rorolala_vcs.version_graph.delete_changes_confirm_title"),
                RolaI18N.Get(
                    "rorolala_vcs.version_graph.delete_changes_confirm_message",
                    Path.GetFileName(file)
                ),
                () => BeginDeleteChanges(file)
            )
        );

    /// <summary>Puts the ending of a merge to the user, with what it leaves the file holding.</summary>
    /// <param name="file">The file the merge is waiting on, as the program names it.</param>
    /// <param name="directory">The directory the runs are made in.</param>
    /// <param name="variant">Where the tree holds the variant file.</param>
    /// <param name="recorded">Where the record says the variant file lies.</param>
    /// <param name="keep">Whether the file's content is kept rather than put back.</param>
    private void AskCancelMerge(
        string file,
        string directory,
        string variant,
        string recorded,
        bool keep
    ) =>
        _dialogs.Show(
            new Dialog(
                RolaI18N.Get("rorolala_vcs.version_graph.cancel_merge_confirm_title"),
                RolaI18N.Get(
                    keep
                        ? "rorolala_vcs.version_graph.cancel_merge_keep_confirm_message"
                        : "rorolala_vcs.version_graph.cancel_merge_confirm_message",
                    Path.GetFileName(file)
                ),
                () => BeginCancelMerge(file, directory, variant, recorded, keep)
            )
        );

    /// <summary>Puts the retrack to the user, asking first only when there is work on disk to lose.</summary>
    /// <param name="file">The file the record is of.</param>
    /// <param name="version">The version the record is to be moved to.</param>
    /// <param name="label">What the version is called in the question.</param>
    /// <param name="onlyIndex">Whether the file on disk is left alone.</param>
    private void AskRetrack(string file, string version, string label, bool onlyIndex)
    {
        if (Path.GetDirectoryName(file) is not { Length: > 0 } directory)
        {
            return;
        }

        if (onlyIndex || !Changed(file))
        {
            BeginRetrack(directory, file, version, onlyIndex, force: false);

            return;
        }

        _dialogs.Show(
            new Dialog(
                RolaI18N.Get("rorolala_vcs.version_graph.retrack_confirm_title"),
                RolaI18N.Get(
                    "rorolala_vcs.version_graph.retrack_confirm_message",
                    Path.GetFileName(file),
                    label
                ),
                () => BeginRetrack(directory, file, version, onlyIndex: false, force: true)
            )
        );
    }

    /// <summary>Whether the working file holds a change that was never recorded.</summary>
    /// <remarks>
    /// A reading that cannot answer is answered as a change, so that the one thing not known is a
    /// question asked rather than a write made.
    /// </remarks>
    /// <param name="file">The file, as the program names it.</param>
    /// <returns>Whether the file's content is more than the record says.</returns>
    private bool Changed(string file)
    {
        if (
            Path.GetDirectoryName(file) is not { Length: > 0 } directory
            || _statuses.Reading(directory) is not { } status
        )
        {
            return true;
        }

        try
        {
            return status.StateOf(file) is PathState.Modified or PathState.Wrong;
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Warn($"how `{file}` stands could not be read: {error.Message}");

            return true;
        }
    }

    /// <summary>The path a place in a merge record names, when it lies inside the work.</summary>
    /// <param name="root">Where the Workspace is rooted.</param>
    /// <param name="place">The place, relative to the Workspace's root.</param>
    /// <returns>The absolute path, or nothing when the place is not inside the Workspace.</returns>
    private static string? VariantPath(string root, string place)
    {
        var work = Path.GetFullPath(root);
        var disk = Path.GetFullPath(Path.Combine(work, place));
        var relative = Path.GetRelativePath(work, disk);

        return relative == ".."
            || relative.StartsWith(".." + Path.DirectorySeparatorChar, StringComparison.Ordinal)
            || Path.IsPathRooted(relative)
            ? null
            : disk;
    }

    /// <summary>Runs the restore that drops a file's unrecorded change.</summary>
    /// <param name="file">The file, as the program names it.</param>
    private void BeginDeleteChanges(string file)
    {
        if (Path.GetDirectoryName(file) is not { Length: > 0 } directory)
        {
            return;
        }

        var run = _hub.Begin("rorolala_vcs.progress.restore");

        _ = Task.Run(() =>
        {
            var (done, said) = AlignCommand.Restore(directory, file);

            Dispatcher.UIThread.Post(() =>
            {
                _hub.End(run);
                _files.Touch();

                if (!done)
                {
                    FailureNotice.Show(
                        _dialogs,
                        _icons,
                        RolaI18N.Get("rorolala_vcs.version_graph.delete_changes_failed"),
                        said
                    );

                    return;
                }

                Refresh();
            });
        });
    }

    /// <summary>Ends the merge waiting on a file, putting the file back or leaving what it holds.</summary>
    /// <param name="file">The file the merge is waiting on, as the program names it.</param>
    /// <param name="directory">The directory the runs are made in.</param>
    /// <param name="variant">Where the tree holds the variant file.</param>
    /// <param name="recorded">Where the record says the variant file lies.</param>
    /// <param name="keep">Whether the file's content is kept rather than put back.</param>
    private void BeginCancelMerge(
        string file,
        string directory,
        string variant,
        string recorded,
        bool keep
    )
    {
        // A file with nothing unrecorded has nothing to put back: the merge is ended just the same by
        // the run before it.
        var restore = !keep && _statuses.Reading(directory) is { } status
            && status.StateOf(file) is PathState.Modified or PathState.Wrong;
        var run = _hub.Begin("rorolala_vcs.progress.unmerge");

        _ = Task.Run(() =>
        {
            var (done, said) = AlignCommand.Unmerge(directory, variant, recorded);

            if (done && restore)
            {
                (done, said) = AlignCommand.Restore(directory, file);
            }

            Dispatcher.UIThread.Post(() =>
            {
                _hub.End(run);
                _files.Touch();

                if (!done)
                {
                    FailureNotice.Show(
                        _dialogs,
                        _icons,
                        RolaI18N.Get("rorolala_vcs.version_graph.cancel_merge_failed"),
                        said
                    );

                    return;
                }

                Refresh();
            });
        });
    }

    /// <summary>Turns an unrecorded variant into a merge waiting on a file.</summary>
    /// <param name="file">The file the variant is to be joined into.</param>
    /// <param name="variant">The variant.</param>
    private void BeginJoin(string file, string variant)
    {
        if (Path.GetDirectoryName(file) is not { Length: > 0 } directory)
        {
            return;
        }

        var run = _hub.Begin("rorolala_vcs.progress.join");

        _ = Task.Run(() =>
        {
            var (done, said) = JoinCommand.Run(directory, file, variant);

            Dispatcher.UIThread.Post(() =>
            {
                _hub.End(run);
                _files.Touch();

                if (!done)
                {
                    FailureNotice.Show(
                        _dialogs,
                        _icons,
                        RolaI18N.Get("rorolala_vcs.version_graph.join_failed"),
                        said
                    );

                    return;
                }

                Refresh();
            });
        });
    }

    /// <summary>Runs a retrack, and reads the work again once it is over.</summary>
    /// <param name="directory">The directory the run is made in.</param>
    /// <param name="file">The file the record is of.</param>
    /// <param name="version">The version the record is to be moved to.</param>
    /// <param name="onlyIndex">Whether the file on disk is left alone.</param>
    /// <param name="force">Whether unrecorded changes are to be discarded.</param>
    private void BeginRetrack(
        string directory,
        string file,
        string version,
        bool onlyIndex,
        bool force
    )
    {
        var run = _hub.Begin("rorolala_vcs.progress.retrack");

        _ = Task.Run(() =>
        {
            var (done, said) = RetrackCommand.Run(directory, file, version, onlyIndex, force);

            Dispatcher.UIThread.Post(() =>
            {
                _hub.End(run);
                _files.Touch();

                if (!done)
                {
                    FailureNotice.Show(
                        _dialogs,
                        _icons,
                        RolaI18N.Get("rorolala_vcs.version_graph.retrack_failed"),
                        said
                    );

                    return;
                }

                Refresh();

                // A sync writes the copy a fetch brings and is followed by one of its own, so a fetch
                // made beside it would be two exchanges for one file.
                if (!_sync.Running)
                {
                    _fetch.Fetch(directory, Refresh);
                }
            });
        });
    }

    /// <summary>Puts a text on the clipboard.</summary>
    /// <param name="said">The text to copy.</param>
    private void Copy(string said)
    {
        if (TopLevel.GetTopLevel(this)?.Clipboard is { } clipboard)
        {
            _ = Store(clipboard, said);
        }
    }

    /// <summary>Writes a text to a clipboard, saying so when the system refuses it.</summary>
    /// <param name="clipboard">The clipboard to write to.</param>
    /// <param name="said">The text to write.</param>
    private async Task Store(IClipboard clipboard, string said)
    {
        try
        {
            await clipboard.SetTextAsync(said);
        }
        catch (Exception error) when (error is not OutOfMemoryException)
        {
            _log.Warn($"the text could not be copied: {error.Message}");
        }
    }
}
