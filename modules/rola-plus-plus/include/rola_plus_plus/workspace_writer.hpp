#pragma once

#include <cstdint>
#include <expected>
#include <filesystem>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include <rola_plus_plus/gen/raw.hpp>
#include <rola_plus_plus/gen/release.hpp>

#include "detail.hpp"
#include "error.hpp"
#include "storage.hpp"
#include "write.hpp"
#include "write_request.hpp"
#include "write_result.hpp"
#include "workspace.hpp"

namespace rola {

namespace detail {

inline TrackRefusal read_track_refusal(const RolaWriteTrackRefusal *handle) {
    TrackRefusal out;
    out.kind = take(raw::write_track_refusal_kind(handle));
    out.path = take(raw::write_track_refusal_path(handle));
    out.name = take(raw::write_track_refusal_name(handle));
    out.file_kind = take(raw::write_track_refusal_file_kind(handle));
    out.cause = take(raw::write_track_refusal_cause(handle));
    const auto many = raw::write_track_refusal_paths_count(handle);
    out.paths.reserve(many);
    for (std::uintptr_t index = 0; index < many; ++index) {
        out.paths.push_back(take(raw::write_track_refusal_paths_at(handle, index)));
    }
    return out;
}

inline TrackAction read_track_action(const std::string &text) noexcept {
    if (text == "added") {
        return TrackAction::added;
    }
    if (text == "updated") {
        return TrackAction::updated;
    }
    if (text == "replaced") {
        return TrackAction::replaced;
    }
    return TrackAction::unknown;
}

inline SyncEntry read_sync_entry(const RolaWriteSyncEntry *handle) {
    return SyncEntry{
        .uuid = take(raw::write_sync_entry_uuid(handle)),
        .local_path = presence(take(raw::write_sync_entry_local_path(handle))),
        .remote_path = presence(take(raw::write_sync_entry_remote_path(handle))),
        .planned_path = presence(take(raw::write_sync_entry_planned_path(handle))),
        .local_version = presence(take(raw::write_sync_entry_local_version(handle))),
        .remote_version = presence(take(raw::write_sync_entry_remote_version(handle))),
        .owner = presence(take(raw::write_sync_entry_owner(handle))),
        .deprecated = raw::write_sync_entry_deprecated(handle),
        .item_kind = presence(take(raw::write_sync_entry_item_kind(handle))),
        .modified = raw::write_sync_entry_modified(handle),
        .kind = take(raw::write_sync_entry_kind(handle)),
    };
}

inline OwnershipRefusal read_ownership_refusal(const RolaWriteOwnershipRefusal *handle) {
    return OwnershipRefusal{
        .kind = take(raw::write_ownership_refusal_kind(handle)),
        .path = take(raw::write_ownership_refusal_path(handle)),
        .owner = take(raw::write_ownership_refusal_owner(handle)),
        .cause = take(raw::write_ownership_refusal_cause(handle)),
    };
}

inline std::expected<AlignResult, Error> read_align(const RolaWriteAlign *handle) {
    const auto kind = take(raw::write_align_kind(handle));
    if (kind == "paths") {
        AlignResult out;
        out.shape = AlignShape::paths;
        const auto many = raw::write_align_paths_count(handle);
        out.paths.reserve(many);
        for (std::uintptr_t index = 0; index < many; ++index) {
            Owned<RolaWriteAlignEntry> one{raw::write_align_paths_at(handle, index)};
            if (!one) {
                continue;
            }
            out.paths.push_back(AlignChange{
                .did = take(raw::write_align_entry_did(one.get())),
                .what = take(raw::write_align_entry_what(one.get())),
            });
        }
        return out;
    }

    if (kind == "tree" || kind == "tree_file") {
        AlignResult out;
        out.shape = kind == "tree" ? AlignShape::tree : AlignShape::tree_file;
        out.path = presence(take(raw::write_align_path(handle)));
        out.staged = raw::write_align_staged(handle);
        return out;
    }

    if (kind == "refused") {
        Owned<RolaWriteAlignRefusal> refusal{raw::write_align_refusal(handle)};
        if (!refusal) {
            return std::unexpected{
                Error{ErrorKind::internal, {}, "the alignment refused without a refusal"}};
        }

        const auto how = take(raw::write_align_refusal_kind(refusal.get()));
        std::string subject = take(raw::write_align_refusal_path(refusal.get()));
        if (subject.empty()) {
            subject = take(raw::write_align_refusal_from(refusal.get()));
        }
        if (subject.empty()) {
            subject = take(raw::write_align_refusal_to(refusal.get()));
        }
        const auto cause = take(raw::write_align_refusal_cause(refusal.get()));
        const auto move = take(raw::write_align_refusal_move_kind(refusal.get()));

        std::string reason = how;
        if (!cause.empty()) {
            reason += ": " + cause;
        }
        if (!move.empty()) {
            reason += " (" + move + ")";
        }
        return std::unexpected{Error{ErrorKind::precondition, std::move(subject),
                                     std::move(reason)}};
    }

    return std::unexpected{
        Error{ErrorKind::internal, {}, "the alignment answered no kind this build knows"}};
}

} // namespace detail

// Writes over a Workspace a caller has read. The writer names the root of the
// reading it holds; every call reaches the native side through that root.
class WorkspaceWriter {
public:
    explicit WorkspaceWriter(const WorkspaceReading &reading)
        : reading_(&reading), root_path_(reading.root()), root_(detail::utf8(reading.root())) {}

    // The store the Workspace keeps.
    [[nodiscard]] StorageScope storage() const { return StorageScope{root_path_}; }

    [[nodiscard]] std::expected<StorageBind, Error> bind_storage(
        std::filesystem::path storage) const {
        const auto at = detail::utf8(storage);
        auto answer = take<raw::workspace_storage_bind>(
            raw::workspace_storage_bind(root_.c_str(), at.c_str()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }

        Owned<RolaWriteStorageBind> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, at, "the binding answered nothing"}};
        }

        StorageBind out;
        out.bound_workspace = raw::write_storage_bind_bound_workspace(handle.get());
        out.bound_vault = raw::write_storage_bind_bound_vault(handle.get());
        out.reverse_recorded = raw::write_storage_bind_reverse_recorded(handle.get());
        out.read_only = raw::write_storage_bind_read_only(handle.get());
        const auto many = raw::write_storage_bind_warning_count(handle.get());
        out.warnings.reserve(many);
        for (std::uintptr_t index = 0; index < many; ++index) {
            out.warnings.push_back(
                detail::take(raw::write_storage_bind_warning_at(handle.get(), index)));
        }
        return out;
    }

    // Reads the whole tree and writes down what each path holds.
    [[nodiscard]] std::expected<CacheReport, Error> rebuild_tree_cache() const {
        auto answer = take<raw::write_workspace_rebuild_cache>(
            raw::write_workspace_rebuild_cache(root_.c_str()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteWorkspaceCache> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, root_, "the rebuild answered nothing"}};
        }
        return CacheReport{.paths = raw::write_workspace_cache_paths(handle.get()),
                            .bytes = raw::write_workspace_cache_bytes(handle.get())};
    }

    [[nodiscard]] std::expected<AlignResult, Error> align(
        const AlignRequest &request, Progress *watch = nullptr,
        Cancellation *stop = nullptr) const {
        auto handles = detail::WriteHandles::make(watch, stop);
        if (!handles) {
            return std::unexpected{detail::control_error()};
        }

        RolaWriteAlignRequest native{
            .common = detail::native_common(root_, request.common),
            .path = request.path.c_str(),
            .mode = detail::native_align_mode(request.mode),
            .completely = request.completely,
            .include_ignored = request.include_ignored,
        };

        auto answer = take<raw::write_align>(
            raw::write_align(&native, handles->progress, handles->cancellation));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteAlign> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, request.path, "the alignment answered nothing"}};
        }
        return detail::read_align(handle.get());
    }

    [[nodiscard]] std::expected<BringResult, Error> bring(
        const BringRequest &request, Progress *watch = nullptr,
        Cancellation *stop = nullptr) const {
        auto handles = detail::WriteHandles::make(watch, stop);
        if (!handles) {
            return std::unexpected{detail::control_error()};
        }

        detail::StringSpan refs{request.refs};
        RolaWriteBringRequest native{
            .common = detail::native_common(root_, request.common),
            .refs = refs.span(),
            .to = request.to.c_str(),
            .by_name = request.by_name,
            .join = "",
            .variant = "",
            .file = "",
            .tree_merge = false,
        };

        auto answer = take<raw::write_bring>(
            raw::write_bring(&native, handles->progress, handles->cancellation));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteBring> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{Error{ErrorKind::internal, {}, "the bring answered nothing"}};
        }

        BringResult out;
        const auto entries = raw::write_bring_entry_count(handle.get());
        out.entries.reserve(entries);
        for (std::uintptr_t index = 0; index < entries; ++index) {
            Owned<RolaWriteBringEntry> one{raw::write_bring_entry_at(handle.get(), index)};
            if (!one) {
                continue;
            }
            out.entries.push_back(BringEntry{
                .uuid = detail::take(raw::write_bring_entry_uuid(one.get())),
                .remote_path =
                    detail::presence(detail::take(raw::write_bring_entry_remote_path(one.get()))),
                .local_path = detail::take(raw::write_bring_entry_local_path(one.get())),
            });
        }

        const auto failed = raw::write_bring_failed_count(handle.get());
        out.failed.reserve(failed);
        for (std::uintptr_t index = 0; index < failed; ++index) {
            out.failed.push_back(detail::take(raw::write_bring_failed_at(handle.get(), index)));
        }
        return out;
    }

    [[nodiscard]] std::expected<BringVariantResult, Error> bring_variant(
        const BringVariantRequest &request, Progress *watch = nullptr,
        Cancellation *stop = nullptr) const {
        auto handles = detail::WriteHandles::make(watch, stop);
        if (!handles) {
            return std::unexpected{detail::control_error()};
        }

        RolaWriteBringRequest native{
            .common = detail::native_common(root_, request.common),
            .refs = RolaStrSpan{},
            .to = request.to.c_str(),
            .by_name = false,
            .join = request.variant.c_str(),
            .variant = request.variant.c_str(),
            .file = request.file.c_str(),
            .tree_merge = request.tree_merge,
        };

        auto answer = take<raw::write_bring>(
            raw::write_bring(&native, handles->progress, handles->cancellation));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteBring> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, request.variant, "the merge answered nothing"}};
        }
        return BringVariantResult{
            .target = detail::take(raw::write_bring_target(handle.get())),
            .variant = detail::take(raw::write_bring_variant(handle.get())),
            .path = detail::take(raw::write_bring_path(handle.get())),
        };
    }

    // Fetches the content a request names. The plan has no C# counterpart and
    // is what `rola fetch --with` calls.
    [[nodiscard]] std::expected<FetchResult, Error> fetch_with(
        const FetchRequest &request, Progress *watch = nullptr,
        Cancellation *stop = nullptr) const {
        auto handles = detail::WriteHandles::make(watch, stop);
        if (!handles) {
            return std::unexpected{detail::control_error()};
        }

        detail::StringSpan words{request.words};
        RolaWriteFetchRequest native{
            .common = detail::native_common(root_, request.common),
            .layout = request.layout.c_str(),
            .words = words.span(),
            .recursive = request.recursive,
        };

        auto answer = take<raw::write_fetch>(
            raw::write_fetch(&native, handles->progress, handles->cancellation));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteFetch> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{Error{ErrorKind::internal, {}, "the fetch answered nothing"}};
        }

        FetchResult out;
        const auto entries = raw::write_fetch_entry_count(handle.get());
        out.entries.reserve(entries);
        for (std::uintptr_t index = 0; index < entries; ++index) {
            Owned<RolaWriteFetchEntry> one{raw::write_fetch_entry_at(handle.get(), index)};
            if (!one) {
                continue;
            }
            out.entries.push_back(FetchEntry{
                .target = detail::take(raw::write_fetch_entry_target(one.get())),
                .path = detail::take(raw::write_fetch_entry_path(one.get())),
                .version = detail::take(raw::write_fetch_entry_version(one.get())),
            });
        }

        const auto brought = raw::write_fetch_brought_count(handle.get());
        out.brought.reserve(brought);
        for (std::uintptr_t index = 0; index < brought; ++index) {
            Owned<RolaWriteFetchSource> one{raw::write_fetch_brought_at(handle.get(), index)};
            if (!one) {
                continue;
            }
            out.brought.push_back(FetchSource{
                .vault = detail::take(raw::write_fetch_source_vault(one.get())),
                .count = raw::write_fetch_source_count(one.get()),
            });
        }
        return out;
    }

    [[nodiscard]] std::expected<TrackPlan, Error> plan_track(
        const TrackRequest &request, Progress *watch = nullptr,
        Cancellation *stop = nullptr) const {
        auto handles = detail::WriteHandles::make(watch, stop);
        if (!handles) {
            return std::unexpected{detail::control_error()};
        }

        detail::StringSpan files{request.files};
        RolaWriteTrackRequest native{
            .common = detail::native_common(root_, request.common),
            .files = files.span(),
            .replace = request.replace,
            .policy = request.policy.c_str(),
            .include_ignored = request.include_ignored,
        };

        auto answer = take<raw::write_track_plan>(
            raw::write_track_plan(&native, handles->progress, handles->cancellation));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteTrackPlan> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, {}, "the track plan answered nothing"}};
        }

        TrackPlan out;
        const auto subjects = raw::write_track_plan_subject_count(handle.get());
        out.subjects.reserve(subjects);
        for (std::uintptr_t index = 0; index < subjects; ++index) {
            Owned<RolaWriteTrackSubject> one{
                raw::write_track_plan_subject_at(handle.get(), index)};
            if (!one) {
                continue;
            }
            out.subjects.push_back(TrackSubject{
                .is_tree = raw::write_track_subject_tree(one.get()),
                .key = detail::take(raw::write_track_subject_key(one.get())),
                .path = detail::take(raw::write_track_subject_path(one.get())),
                .digest = detail::take(raw::write_track_subject_digest(one.get())),
            });
        }

        out.unchanged = raw::write_track_plan_unchanged(handle.get());

        const auto prefill = raw::write_track_plan_prefill_count(handle.get());
        out.prefill.reserve(prefill);
        for (std::uintptr_t index = 0; index < prefill; ++index) {
            out.prefill.push_back(
                detail::take(raw::write_track_plan_prefill_at(handle.get(), index)));
        }

        Owned<RolaWriteTrackRefusal> refusal{raw::write_track_plan_refused(handle.get())};
        if (refusal) {
            out.refused = detail::read_track_refusal(refusal.get());
        }
        out.agreed = true;
        return out;
    }

    [[nodiscard]] std::expected<TrackResult, Error> apply_track(
        const TrackRequest &request, const TrackPlan &plan,
        std::span<const TrackMessage> messages, Progress *watch = nullptr,
        Cancellation *stop = nullptr) const {
        if (!plan.agreed) {
            return std::unexpected{Error{ErrorKind::precondition, {},
                                         "the plan was not made by plan_track"}};
        }

        auto handles = detail::WriteHandles::make(watch, stop);
        if (!handles) {
            return std::unexpected{detail::control_error()};
        }

        detail::StringSpan files{request.files};
        RolaWriteTrackRequest native{
            .common = detail::native_common(root_, request.common),
            .files = files.span(),
            .replace = request.replace,
            .policy = request.policy.c_str(),
            .include_ignored = request.include_ignored,
        };

        std::vector<RolaWriteTrackAgreed> agreed;
        agreed.reserve(plan.subjects.size());
        for (const auto &subject : plan.subjects) {
            agreed.push_back(RolaWriteTrackAgreed{
                .path = subject.path.c_str(),
                .digest = subject.digest.c_str(),
            });
        }

        std::vector<RolaWriteTrackMessage> words;
        words.reserve(messages.size());
        for (const auto &message : messages) {
            words.push_back(RolaWriteTrackMessage{
                .path = message.path.c_str(),
                .text = message.text.c_str(),
            });
        }

        auto answer = take<raw::write_track_apply>(raw::write_track_apply(
            &native, agreed.empty() ? nullptr : agreed.data(), agreed.size(),
            words.empty() ? nullptr : words.data(), words.size(), handles->progress,
            handles->cancellation));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteTrackApply> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{Error{ErrorKind::internal, {}, "the track answered nothing"}};
        }

        TrackResult out;
        const auto items = raw::write_track_apply_item_count(handle.get());
        out.items.reserve(items);
        for (std::uintptr_t index = 0; index < items; ++index) {
            Owned<RolaWriteTrackItem> one{raw::write_track_apply_item_at(handle.get(), index)};
            if (!one) {
                continue;
            }
            auto action = detail::take(raw::write_track_item_action(one.get()));
            out.items.push_back(TrackItem{
                .path = detail::take(raw::write_track_item_path(one.get())),
                .action = detail::read_track_action(action),
            });
        }

        const auto refused = raw::write_track_apply_refused_count(handle.get());
        out.refused.reserve(refused);
        for (std::uintptr_t index = 0; index < refused; ++index) {
            Owned<RolaWriteTrackRefusal> one{
                raw::write_track_apply_refused_at(handle.get(), index)};
            if (!one) {
                continue;
            }
            out.refused.push_back(detail::read_track_refusal(one.get()));
        }
        return out;
    }

    [[nodiscard]] std::expected<RetrackResult, Error> retrack(
        const RetrackRequest &request, Progress *watch = nullptr,
        Cancellation *stop = nullptr) const {
        auto handles = detail::WriteHandles::make(watch, stop);
        if (!handles) {
            return std::unexpected{detail::control_error()};
        }

        RolaWriteRetrackRequest native{
            .common = detail::native_common(root_, request.common),
            .file = request.file.c_str(),
            .record = request.record.c_str(),
            .only_index = request.only_index,
            .deny_jump = request.deny_jump,
            .fork = request.fork,
        };

        auto answer = take<raw::write_retrack>(
            raw::write_retrack(&native, handles->progress, handles->cancellation));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteRetrack> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, request.file, "the retrack answered nothing"}};
        }

        const auto kind = detail::take(raw::write_retrack_kind(handle.get()));
        RetrackResult out;
        out.path = detail::take(raw::write_retrack_path(handle.get()));
        out.version = detail::take(raw::write_retrack_version(handle.get()));
        out.number = raw::write_retrack_number_present(handle.get())
                         ? std::optional<std::uint64_t>{raw::write_retrack_number(handle.get())}
                         : std::nullopt;
        out.changed = raw::write_retrack_changed(handle.get());
        out.switched = raw::write_retrack_switched(handle.get());
        out.from = detail::presence(detail::take(raw::write_retrack_from(handle.get())));
        out.to = detail::presence(detail::take(raw::write_retrack_to(handle.get())));
        out.forked = raw::write_retrack_forked(handle.get());
        out.kind = kind == "path"    ? RetrackKind::path
                   : kind == "tree" ? RetrackKind::tree
                                    : RetrackKind::unknown;

        const auto took = raw::write_retrack_took_count(handle.get());
        out.took.reserve(took);
        for (std::uintptr_t index = 0; index < took; ++index) {
            Owned<RolaWriteRetrackTook> one{raw::write_retrack_took_at(handle.get(), index)};
            if (!one) {
                continue;
            }
            out.took.push_back(RetrackTook{
                .vault = detail::take(raw::write_retrack_took_vault(one.get())),
                .objects = raw::write_retrack_took_objects(one.get()),
            });
        }
        return out;
    }

    [[nodiscard]] std::expected<SyncPlan, Error> plan_sync(
        const SyncRequest &request, Progress *watch = nullptr,
        Cancellation *stop = nullptr) const {
        auto handles = detail::WriteHandles::make(watch, stop);
        if (!handles) {
            return std::unexpected{detail::control_error()};
        }

        RolaWriteSyncRequest native{
            .common = detail::native_common(root_, request.common),
            .direction = detail::native_direction(request.direction),
            .forced = request.forced,
            .storage = request.storage,
            .index = request.index,
            .layout = request.layout,
        };

        auto answer = take<raw::write_sync_plan>(
            raw::write_sync_plan(&native, handles->progress, handles->cancellation));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteSyncPlan> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, {}, "the sync plan answered nothing"}};
        }

        SyncPlan out;
        out.layout = detail::take(raw::write_sync_plan_layout(handle.get()));
        out.vault = detail::take(raw::write_sync_plan_vault(handle.get()));
        out.forced = raw::write_sync_plan_forced(handle.get());
        out.layout_read = raw::write_sync_plan_layout_read(handle.get());
        out.index = raw::write_sync_plan_index(handle.get());
        out.storage = raw::write_sync_plan_storage(handle.get());
        out.up_to_date = raw::write_sync_plan_up_to_date(handle.get());

        const auto entries = raw::write_sync_plan_entry_count(handle.get());
        out.entries.reserve(entries);
        for (std::uintptr_t index = 0; index < entries; ++index) {
            Owned<RolaWriteSyncEntry> one{raw::write_sync_plan_entry_at(handle.get(), index)};
            if (!one) {
                continue;
            }
            out.entries.push_back(detail::read_sync_entry(one.get()));
        }
        out.agreed = true;
        return out;
    }

    [[nodiscard]] std::expected<SyncReport, Error> apply_sync(
        const SyncRequest &request, const SyncPlan &plan, Progress *watch = nullptr,
        Cancellation *stop = nullptr) const {
        if (!plan.agreed) {
            return std::unexpected{Error{ErrorKind::precondition, {},
                                         "the plan was not made by plan_sync"}};
        }

        auto handles = detail::WriteHandles::make(watch, stop);
        if (!handles) {
            return std::unexpected{detail::control_error()};
        }

        RolaWriteSyncRequest native{
            .common = detail::native_common(root_, request.common),
            .direction = detail::native_direction(request.direction),
            .forced = request.forced,
            .storage = request.storage,
            .index = request.index,
            .layout = request.layout,
        };

        std::vector<RolaWriteSyncAgreed> agreed;
        agreed.reserve(plan.entries.size());
        for (const auto &entry : plan.entries) {
            agreed.push_back(RolaWriteSyncAgreed{
                .uuid = entry.uuid.c_str(),
                .local_path = entry.local_path ? entry.local_path->c_str() : "",
                .remote_path = entry.remote_path ? entry.remote_path->c_str() : "",
                .planned_path = entry.planned_path ? entry.planned_path->c_str() : "",
                .local_version = entry.local_version ? entry.local_version->c_str() : "",
                .remote_version = entry.remote_version ? entry.remote_version->c_str() : "",
                .kind = entry.kind.c_str(),
            });
        }

        auto answer = take<raw::write_sync_apply>(raw::write_sync_apply(
            &native, agreed.empty() ? nullptr : agreed.data(), agreed.size(),
            handles->progress, handles->cancellation));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteSyncApply> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{Error{ErrorKind::internal, {}, "the sync answered nothing"}};
        }

        SyncReport out;
        out.layout = detail::take(raw::write_sync_apply_layout(handle.get()));
        out.vault = detail::take(raw::write_sync_apply_vault(handle.get()));
        out.applied = raw::write_sync_apply_applied(handle.get());
        out.received = raw::write_sync_apply_received(handle.get());
        out.up_to_date = raw::write_sync_apply_up_to_date(handle.get());
        out.layout_read = raw::write_sync_apply_layout_read(handle.get());
        out.index_objects = raw::write_sync_apply_index_objects(handle.get());
        out.storage_objects = raw::write_sync_apply_storage_objects(handle.get());

        const auto failed = raw::write_sync_apply_failed_count(handle.get());
        out.failed.reserve(failed);
        for (std::uintptr_t index = 0; index < failed; ++index) {
            out.failed.push_back(
                detail::take(raw::write_sync_apply_failed_at(handle.get(), index)));
        }
        return out;
    }

    [[nodiscard]] std::expected<OwnershipPlan, Error> plan_ownership(
        const OwnershipRequest &request, Progress *watch = nullptr,
        Cancellation *stop = nullptr) const {
        auto handles = detail::WriteHandles::make(watch, stop);
        if (!handles) {
            return std::unexpected{detail::control_error()};
        }

        detail::StringSpan paths{request.paths};
        RolaWriteOwnershipRequest native{
            .common = detail::native_common(root_, request.common),
            .kind = detail::native_ownership(request.kind),
            .paths = paths.span(),
            .allow_partial = request.allow_partial,
        };

        auto answer = take<raw::write_ownership_plan>(
            raw::write_ownership_plan(&native, handles->progress, handles->cancellation));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteOwnershipPlan> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, {}, "the ownership plan answered nothing"}};
        }

        OwnershipPlan out;
        const auto wanted = raw::write_ownership_plan_wanted_count(handle.get());
        out.wanted.reserve(wanted);
        for (std::uintptr_t index = 0; index < wanted; ++index) {
            Owned<RolaWriteOwnershipWanted> one{
                raw::write_ownership_plan_wanted_at(handle.get(), index)};
            if (!one) {
                continue;
            }
            out.wanted.push_back(OwnershipWanted{
                .path = detail::take(raw::write_ownership_wanted_path(one.get())),
                .uuid = detail::take(raw::write_ownership_wanted_uuid(one.get())),
            });
        }

        const auto refused = raw::write_ownership_plan_refused_count(handle.get());
        out.refused.reserve(refused);
        for (std::uintptr_t index = 0; index < refused; ++index) {
            Owned<RolaWriteOwnershipRefusal> one{
                raw::write_ownership_plan_refused_at(handle.get(), index)};
            if (!one) {
                continue;
            }
            out.refused.push_back(detail::read_ownership_refusal(one.get()));
        }
        out.skipped = raw::write_ownership_plan_skipped(handle.get());
        return out;
    }

    // Moves the ownership a request names and a plan agreed to. A null plan
    // asks the native side to make the plan again and apply it.
    [[nodiscard]] std::expected<OwnershipResult, Error> apply_ownership(
        const OwnershipRequest &request, const OwnershipPlan *plan = nullptr,
        Progress *watch = nullptr, Cancellation *stop = nullptr) const {
        auto handles = detail::WriteHandles::make(watch, stop);
        if (!handles) {
            return std::unexpected{detail::control_error()};
        }

        detail::StringSpan paths{request.paths};
        RolaWriteOwnershipRequest native{
            .common = detail::native_common(root_, request.common),
            .kind = detail::native_ownership(request.kind),
            .paths = paths.span(),
            .allow_partial = request.allow_partial,
        };

        std::vector<RolaWriteOwnershipAgreed> agreed;
        if (plan != nullptr) {
            agreed.reserve(plan->wanted.size());
            for (const auto &wanted : plan->wanted) {
                agreed.push_back(RolaWriteOwnershipAgreed{
                    .path = wanted.path.c_str(),
                    .uuid = wanted.uuid.c_str(),
                });
            }
        }

        auto answer = take<raw::write_ownership_apply>(raw::write_ownership_apply(
            &native, agreed.empty() ? nullptr : agreed.data(), agreed.size(),
            handles->progress, handles->cancellation));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteOwnershipApply> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, {}, "the ownership move answered nothing"}};
        }

        OwnershipResult out;
        const auto entries = raw::write_ownership_apply_entry_count(handle.get());
        out.entries.reserve(entries);
        for (std::uintptr_t index = 0; index < entries; ++index) {
            Owned<RolaWriteOwnershipEntry> one{
                raw::write_ownership_apply_entry_at(handle.get(), index)};
            if (!one) {
                continue;
            }
            out.entries.push_back(OwnershipEntry{
                .path = detail::take(raw::write_ownership_entry_path(one.get())),
                .uuid = detail::take(raw::write_ownership_entry_uuid(one.get())),
                .owner = detail::presence(
                    detail::take(raw::write_ownership_entry_owner(one.get()))),
            });
        }

        const auto failed = raw::write_ownership_apply_failed_count(handle.get());
        out.failed.reserve(failed);
        for (std::uintptr_t index = 0; index < failed; ++index) {
            Owned<RolaWriteOwnershipRefusal> one{
                raw::write_ownership_apply_failed_at(handle.get(), index)};
            if (!one) {
                continue;
            }
            out.failed.push_back(detail::read_ownership_refusal(one.get()));
        }
        return out;
    }

    [[nodiscard]] std::expected<CollectPlan, Error> plan_collect(
        const CollectRequest &request) const {
        auto answer = take<raw::write_collect_plan>(
            raw::write_collect_plan(root_.c_str(), request.storage, request.index,
                                    request.grace));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteCollect> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{Error{ErrorKind::internal, {}, "the collect answered nothing"}};
        }

        Owned<RolaWriteCollectSide> index_side{raw::write_collect_index(handle.get())};
        Owned<RolaWriteCollectSide> storage_side{raw::write_collect_storage(handle.get())};
        return CollectPlan{
            .index = detail::read_collect_side(index_side.get()),
            .storage = detail::read_collect_side(storage_side.get()),
        };
    }

    [[nodiscard]] std::expected<CollectReport, Error> collect(
        const CollectRequest &request) const {
        auto answer = take<raw::write_collect_apply>(
            raw::write_collect_apply(root_.c_str(), request.storage, request.index,
                                     request.grace));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteCollect> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{Error{ErrorKind::internal, {}, "the collect answered nothing"}};
        }

        Owned<RolaWriteCollectSide> index_side{raw::write_collect_index(handle.get())};
        Owned<RolaWriteCollectSide> storage_side{raw::write_collect_storage(handle.get())};
        return CollectReport{
            .index = detail::read_collect_side(index_side.get()),
            .storage = detail::read_collect_side(storage_side.get()),
        };
    }

    // Lays the Workspace's store and index out afresh.
    [[nodiscard]] std::expected<Packed, Error> pack(bool storage = true,
                                                    bool index = true) const {
        auto answer =
            take<raw::write_pack>(raw::write_pack(root_.c_str(), storage, index));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWritePack> handle{std::move(*answer)};
        return Packed{.changed = handle ? raw::write_pack_changed(handle.get()) : false};
    }

    // Makes the Workspace's Layout track a Vault.
    [[nodiscard]] std::expected<void, Error> set_track(std::string_view vault) const {
        const std::string name{vault};
        auto answer = take<raw::write_workspace_set_track>(
            raw::write_workspace_set_track(root_.c_str(), name.c_str()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        return {};
    }

    // Makes the Workspace's Layout track no Vault.
    [[nodiscard]] std::expected<void, Error> clear_track() const {
        auto answer = take<raw::write_workspace_clear_track>(
            raw::write_workspace_clear_track(root_.c_str()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        return {};
    }

    // Forgets the reading of a path the run took out.
    [[nodiscard]] std::expected<void, Error> forget(std::string_view path) const {
        const std::string at{path};
        auto answer = take<raw::write_checkout_forget>(
            raw::write_checkout_forget(root_.c_str(), at.c_str()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        return {};
    }

    // Writes down that a path now holds what the Layout names.
    [[nodiscard]] std::expected<void, Error> remember(std::string_view path) const {
        const std::string at{path};
        auto answer = take<raw::write_checkout_remember>(
            raw::write_checkout_remember(root_.c_str(), at.c_str()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        return {};
    }

    // Puts a version's content back at a path and records that the two agree.
    [[nodiscard]] std::expected<Restored, Error> restore(std::string_view path,
                                                         std::string_view version) const {
        const std::string at{path};
        const std::string named{version};
        auto answer = take<raw::write_checkout_restore>(
            raw::write_checkout_restore(root_.c_str(), at.c_str(), named.c_str()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteRestore> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, at, "the restore answered nothing"}};
        }
        return Restored{.id = detail::take(raw::write_restore_id(handle.get())),
                        .version = detail::take(raw::write_restore_version(handle.get()))};
    }

    // Records a file's content as a version and names it in the Layout. This is
    // the single-file export the plan keeps beside the request form.
    [[nodiscard]] std::expected<TrackedFile, Error> track_file(
        std::string_view path, std::string_view creator, std::string_view message,
        std::string_view policy = {}, std::string_view owner = {},
        std::string_view description = {}) const {
        const std::string at{path};
        const std::string author{creator};
        const std::string words{message};
        const std::string cut{policy};
        const std::string held{owner};
        const std::string note{description};

        auto answer = take<raw::write_track_file>(raw::write_track_file(
            root_.c_str(), at.c_str(), author.c_str(), words.c_str(), cut.c_str(),
            held.c_str(), note.c_str()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteTrackedFile> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{Error{ErrorKind::internal, at, "the track answered nothing"}};
        }
        return TrackedFile{
            .id = detail::take(raw::write_tracked_file_id(handle.get())),
            .storage = detail::take(raw::write_tracked_file_storage(handle.get())),
            .variant = detail::take(raw::write_tracked_file_variant(handle.get())),
            .version = detail::take(raw::write_tracked_file_version(handle.get())),
        };
    }

    // Moves a path's record to a version. This is the single-file export the
    // plan keeps beside the request form.
    [[nodiscard]] std::expected<Retracked, Error> retrack_file(
        std::string_view path, std::string_view target = {}, bool fork = false,
        bool only_index = false, bool force = false, bool deny_jump = false) const {
        const std::string at{path};
        const std::string record{target};

        auto answer = take<raw::write_retrack_file>(raw::write_retrack_file(
            root_.c_str(), at.c_str(), record.c_str(), fork, only_index, force, deny_jump));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteRestore> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, at, "the retrack answered nothing"}};
        }
        return Retracked{.id = detail::take(raw::write_restore_id(handle.get())),
                         .version = detail::take(raw::write_restore_version(handle.get()))};
    }

    // Locks the Workspace for as long as the guard lives. Nothing when the
    // Workspace is locked already, which is what the C# guard answers.
    [[nodiscard]] std::optional<LockGuard> lock() const {
        detail::WorkspaceLock guard{
            raw::workspace_lock_acquire(detail::WriterAccess::workspace(*reading_))};
        if (!guard) {
            return std::nullopt;
        }
        return LockGuard{std::move(guard)};
    }

    [[nodiscard]] const WorkspaceReading &reading() const noexcept { return *reading_; }

private:
    const WorkspaceReading *reading_;
    std::filesystem::path root_path_;
    std::string root_;
};

} // namespace rola
