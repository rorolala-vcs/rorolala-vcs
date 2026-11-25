#pragma once

#include <cstdint>
#include <string>
#include <variant>
#include <vector>

namespace rola {

// Which Vault a request reaches for.
enum class WriteTargetKind {
    tracked,
    named,
    address,
};

// The Vault a request reaches for. A request that names none reaches the Vault
// the Layout tracks, which is the zero value.
struct WriteTarget {
    WriteTargetKind kind = WriteTargetKind::tracked;
    std::string name;
    std::string address;
};

// The words a write calls its work while it does it. A field left empty keeps
// the native word, so a caller translates only what it shows.
struct WriteLabels {
    std::string reading;
    std::string writing;
    std::string fetching;
    std::string uploading;
    std::string downloading;
    std::string checking;
    std::string applying;
};

// The fields every write request carries. The Workspace is not here: a writer
// fills it with the root of the reading it was made over, so a request cannot
// name another Workspace.
struct WriteCommon {
    std::string account;
    WriteTarget target;
    bool offline = false;
    bool force = false;
    std::uintptr_t concurrency = 0;
    WriteLabels labels;
};

// What an alignment is to do with the one path a run named. The names follow
// the native variants; `confirm_delete` and `break_move` are spelled out
// because `delete` and `break` are keywords.
enum class AlignModeKind {
    confirm_delete,
    confirm_rename,
    break_move,
    move_to,
    restore_move,
    restore_modify,
    restore_staged,
    restore_delete,
    restore,
};

struct AlignMode {
    AlignModeKind kind = AlignModeKind::confirm_delete;

    // Where the path moved to, for `move_to`. It is read against the Workspace
    // root, and empty for every other kind.
    std::string move_to;
};

// Settles one path against the Layout it works from.
struct AlignRequest {
    WriteCommon common;
    std::string path;
    AlignMode mode;
    bool completely = false;
    bool include_ignored = false;
};

// Brings what a Vault holds and this Layout does not into it.
struct BringRequest {
    WriteCommon common;
    std::vector<std::string> refs;
    std::string to;
    bool by_name = false;
};

// Brings a variant in to be joined into a file this Layout names. The native
// side tells the two brings apart by `join`, which is filled from `variant`.
struct BringVariantRequest {
    WriteCommon common;
    std::string to;
    std::string variant;
    std::string file;
    bool tree_merge = false;
};

// Fetches the content of paths, directories, Uuids and hashes. An empty
// `layout` reads the one being worked in.
struct FetchRequest {
    WriteCommon common;
    std::string layout;
    std::vector<std::string> words;
    bool recursive = false;
};

// Records a batch of paths, or the changes under them, as versions.
struct TrackRequest {
    WriteCommon common;
    std::vector<std::string> files;
    bool replace = false;
    std::string policy;
    bool include_ignored = false;
};

// Moves a path's record to a version, or one version back when `record` is
// empty.
struct RetrackRequest {
    WriteCommon common;
    std::string file;
    std::string record;
    bool only_index = false;
    bool deny_jump = false;
    bool fork = false;
};

// Which way a sync run may move content.
enum class SyncDirection {
    up,
    down,
    both,
};

// Brings a Layout and the Vault it tracks together. The same request asks for
// a plan and carries it out.
struct SyncRequest {
    WriteCommon common;
    SyncDirection direction = SyncDirection::both;
    bool forced = false;
    bool storage = true;
    bool index = true;
    bool layout = true;
};

// Which way an entry's ownership moves.
enum class OwnershipKind {
    hold,
    giveup,
};

// Moves the ownership of the entries a run names.
struct OwnershipRequest {
    WriteCommon common;
    OwnershipKind kind = OwnershipKind::hold;
    std::vector<std::string> paths;
    bool allow_partial = false;
};

// What a collecting run sweeps and how long it keeps an unreferenced entry.
// A `grace` of zero is the native side's own choice.
struct CollectRequest {
    bool storage = true;
    bool index = true;
    std::uint64_t grace = 0;
};

// One change a Layout batch applies. The nine kinds share one shape because
// they travel as one tagged record each; the fields a kind does not read stay
// empty.
enum class LayoutOpKind {
    create_entry,
    update_entry,
    remove_entry,
    create_path,
    move_path,
    remove_path,
    soft_delete_path,
    change_ownership,
    change_uuid,
};

struct LayoutOp {
    LayoutOpKind kind = LayoutOpKind::create_entry;

    // The entry the change names, for the entry kinds.
    std::string uuid;
    // The path the change names, for the path kinds.
    std::string path;
    // The version an entry holds.
    std::string version;
    // The account that holds an entry, empty for none.
    std::string owner;
    // What an entry says about itself, empty for nothing.
    std::string description;
    // `tree` to make a tree entry, empty for a file.
    std::string entry_kind;
    // The path or Uuid a move starts at.
    std::string from;
    // The path or Uuid a move ends at.
    std::string to;
};

struct LayoutApplyRequest {
    std::vector<LayoutOp> ops;
};

// One object a caller asks the index to write. The four kinds carry different
// fields, so the request names which one it is rather than leaving three of
// them empty.
struct IndexVariantObject {
    std::string storage;
    std::string base_version;
    std::string join;
    std::string creator;
    std::string message;
};

struct IndexVersionObject {
    std::string variant;
};

struct IndexCreatorObject {
    std::string text;
};

struct IndexMessageObject {
    std::string text;
};

using IndexObjectRequest =
    std::variant<IndexVariantObject, IndexVersionObject, IndexCreatorObject,
                 IndexMessageObject>;

// Records a file's content as a variant and a version. An empty `join` merges
// nothing.
struct FileVersionRequest {
    std::string storage;
    std::string base_version;
    std::string join;
    std::string creator;
    std::string message;
};

// The words one subject's version carries, by the path a track plan named it.
struct TrackMessage {
    std::string path;
    std::string text;
};

} // namespace rola
