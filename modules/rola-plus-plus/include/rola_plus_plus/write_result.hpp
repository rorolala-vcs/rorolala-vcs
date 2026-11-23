#pragma once

#include <cstdint>
#include <optional>
#include <string>
#include <vector>

namespace rola {

// One path an alignment acted on.
struct AlignChange {
    std::string did;
    std::string what;
};

// Which shape an alignment answered with. A refusal is not one of them: it is
// reported as a `ErrorKind::precondition` error, the way the C# side throws.
enum class AlignShape {
    paths,
    tree,
    tree_file,
    unknown,
};

struct AlignResult {
    AlignShape shape = AlignShape::unknown;
    std::vector<AlignChange> paths;
    std::optional<std::string> path;
    bool staged = false;
};

// One entry a bring put in.
struct BringEntry {
    std::string uuid;
    std::optional<std::string> remote_path;
    std::string local_path;
};

struct BringResult {
    std::vector<BringEntry> entries;
    std::vector<std::string> failed;
};

// What a variant merge came to.
struct BringVariantResult {
    std::string target;
    std::string variant;
    std::string path;
};

// One target whose content was fetched.
struct FetchEntry {
    std::string target;
    std::string path;
    std::string version;
};

// How many objects one Vault handed over.
struct FetchSource {
    std::string vault;
    std::uint64_t count = 0;
};

struct FetchResult {
    std::vector<FetchEntry> entries;
    std::vector<FetchSource> brought;
};

// One subject a track plan asks a message for.
struct TrackSubject {
    bool is_tree = false;
    std::string key;
    std::string path;
    std::string digest;
};

// Why a track run refused.
struct TrackRefusal {
    std::string kind;
    std::string path;
    std::vector<std::string> paths;
    std::string name;
    std::string file_kind;
    std::string cause;
};

struct TrackPlan {
    std::vector<TrackSubject> subjects;
    std::uint64_t unchanged = 0;
    std::vector<std::string> prefill;
    std::optional<TrackRefusal> refused;

    // True only for a plan `plan_track` made. `apply_track` refuses a plan
    // built by hand, since the native side rereads its subjects and digests
    // and a hand-built one has none to reread.
    bool agreed = false;
};

enum class TrackAction {
    added,
    updated,
    replaced,
    unknown,
};

// One subject a track run recorded.
struct TrackItem {
    std::string path;
    TrackAction action = TrackAction::unknown;
};

struct TrackResult {
    std::vector<TrackItem> items;
    std::vector<TrackRefusal> refused;
};

// Whether a retrack moved one path's record or a tree's.
enum class RetrackKind {
    path,
    tree,
    unknown,
};

// One Vault that handed content over.
struct RetrackTook {
    std::string vault;
    std::uint64_t objects = 0;
};

struct RetrackResult {
    RetrackKind kind = RetrackKind::unknown;
    std::string path;
    std::string version;
    std::optional<std::uint64_t> number;
    bool changed = false;
    bool switched = false;
    std::optional<std::string> from;
    std::optional<std::string> to;
    bool forked = false;
    std::vector<RetrackTook> took;
};

// One entry a sync plan speaks about.
struct SyncEntry {
    std::string uuid;
    std::optional<std::string> local_path;
    std::optional<std::string> remote_path;
    std::optional<std::string> planned_path;
    std::optional<std::string> local_version;
    std::optional<std::string> remote_version;
    std::optional<std::string> owner;
    bool deprecated = false;
    std::optional<std::string> item_kind;
    bool modified = false;
    std::string kind;
};

struct SyncPlan {
    std::string layout;
    std::string vault;
    bool forced = false;
    bool layout_read = true;
    bool index = true;
    bool storage = true;
    std::uint64_t up_to_date = 0;
    std::vector<SyncEntry> entries;

    // True only for a plan `plan_sync` made, for the reason a track plan keeps
    // the same flag.
    bool agreed = false;
};

// One Vault that handed something over while a sync ran.
struct SyncTook {
    std::string vault;
    std::uint64_t objects = 0;
};

struct SyncReport {
    std::string layout;
    std::string vault;
    std::uint64_t applied = 0;
    std::uint64_t received = 0;
    std::uint64_t up_to_date = 0;
    std::vector<std::string> failed;
    bool layout_read = true;
    std::uint64_t index_objects = 0;
    std::uint64_t storage_objects = 0;
    std::vector<SyncTook> took;
};

// One entry an ownership plan means to move.
struct OwnershipWanted {
    std::string path;
    std::string uuid;
};

// Why an entry's ownership may not be moved.
struct OwnershipRefusal {
    std::string kind;
    std::string path;
    std::string owner;
    std::string cause;
};

struct OwnershipPlan {
    std::vector<OwnershipWanted> wanted;
    std::vector<OwnershipRefusal> refused;
    std::uint64_t skipped = 0;
};

// One entry whose ownership the Vault agreed to move.
struct OwnershipEntry {
    std::string path;
    std::string uuid;
    std::optional<std::string> owner;
};

struct OwnershipResult {
    std::vector<OwnershipEntry> entries;
    std::vector<OwnershipRefusal> failed;
};

// One side of a collecting run, as a plan or a run reports it.
struct CollectSide {
    std::uint64_t removed = 0;
    std::uint64_t bytes = 0;
    std::uint64_t kept = 0;
    bool changed = false;
};

struct CollectPlan {
    std::optional<CollectSide> index;
    std::optional<CollectSide> storage;
};

struct CollectReport {
    std::optional<CollectSide> index;
    std::optional<CollectSide> storage;
};

// What packing a store or an index changed.
struct Packed {
    bool changed = false;
};
// What a tree cache rebuild found.
struct CacheReport {
    std::uint64_t paths = 0;
    std::uint64_t bytes = 0;
};

// What a store answered when content went in.
struct Stored {
    std::string key;
    bool chunked = false;
};

// What binding a store to a Workspace or a Vault came to.
struct StorageBind {
    bool bound_workspace = false;
    bool bound_vault = false;
    bool reverse_recorded = false;
    bool read_only = false;
    std::vector<std::string> warnings;
};

// What one path's alignment or restore wrote.
struct Restored {
    std::string id;
    std::string version;
};

// What retracking one path wrote.
struct Retracked {
    std::string id;
    std::string version;
};

// What recording one file's content wrote.
struct TrackedFile {
    std::string id;
    std::string storage;
    std::string variant;
    std::string version;
};

// The variant and version a file's content was recorded as.
struct FileVersionResult {
    std::string variant;
    std::string version;
};

// What writing a tree's description recorded.
struct TreeDescriptionResult {
    std::uint64_t bytes = 0;
};

// One path a staging changed.
struct TreeAddChange {
    std::string path;
    std::string kind;
};

struct TreeAddResult {
    std::vector<TreeAddChange> changes;
};

// What making a tree did.
struct TreeInitResult {
    std::string path;
    std::uint64_t collapsed = 0;
    bool is_root = false;
};

// What a reset did.
struct TreeResetResult {
    bool whole = false;
    std::vector<std::string> paths;
};

// What a tree removal staged.
struct TreeRemoveResult {
    std::vector<std::string> paths;
    bool cached = false;
};

} // namespace rola
