#pragma once

#include <cstdint>
#include <expected>
#include <filesystem>
#include <optional>
#include <string>
#include <string_view>
#include <utility>

#include <rola_plus_plus/gen/constants.hpp>
#include <rola_plus_plus/gen/raw.hpp>
#include <rola_plus_plus/gen/release.hpp>

#include "detail.hpp"
#include "error.hpp"
#include "layout_writer.hpp"
#include "storage.hpp"
#include "vault.hpp"
#include "write.hpp"
#include "write_request.hpp"
#include "write_result.hpp"

namespace rola {

// Writes over a Vault a caller has read. The vault exports name the Vault's
// root the way the workspace ones name a Workspace's, and the one-shot forms
// carry the flags they need as arguments rather than as a request.
class VaultWriter {
public:
    explicit VaultWriter(const VaultReading &reading)
        : reading_(&reading), root_path_(reading.root()), root_(detail::utf8(reading.root())) {}

    // The truth Layout the Vault keeps.
    [[nodiscard]] LayoutWriter layout() const {
        return LayoutWriter{root_path_, vault_layout_name()};
    }

    [[nodiscard]] StorageScope storage() const { return StorageScope{root_path_}; }

    [[nodiscard]] std::expected<StorageBind, Error> bind_storage(
        std::filesystem::path storage) const {
        const auto at = detail::utf8(storage);
        auto answer = take<raw::vault_storage_bind>(
            raw::vault_storage_bind(root_.c_str(), at.c_str()));
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

    // Puts a path on one version and writes what the entry says about it.
    [[nodiscard]] std::expected<Restored, Error> align(
        std::string_view path, std::string_view version, std::string_view owner = {},
        std::string_view description = {}, std::string_view preference = {}) const {
        const std::string at{path};
        const std::string named{version};
        const std::string held{owner};
        const std::string words{description};
        const std::string preferred{preference};

        auto answer = take<raw::write_align_vault>(raw::write_align_vault(
            root_.c_str(), at.c_str(), named.c_str(), held.c_str(), words.c_str(),
            preferred.c_str()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteRestore> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, at, "the align answered nothing"}};
        }
        return Restored{.id = detail::take(raw::write_restore_id(handle.get())),
                        .version = detail::take(raw::write_restore_version(handle.get()))};
    }

    // Moves a path's record to a version in the Vault.
    [[nodiscard]] std::expected<Retracked, Error> retrack_file(
        std::string_view path, std::string_view target = {}, bool fork = false,
        bool only_index = false, bool force = false, bool deny_jump = false) const {
        const std::string at{path};
        const std::string record{target};

        auto answer = take<raw::write_retrack_vault_file>(raw::write_retrack_vault_file(
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

    // Puts a version's content back at a path in the Vault.
    [[nodiscard]] std::expected<Restored, Error> restore(std::string_view path,
                                                         std::string_view version) const {
        const std::string at{path};
        const std::string named{version};

        auto answer = take<raw::write_checkout_restore_vault>(
            raw::write_checkout_restore_vault(root_.c_str(), at.c_str(), named.c_str()));
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

    // Records a file's content and names it in the Vault's Layout.
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

        auto answer = take<raw::write_track_vault_file>(raw::write_track_vault_file(
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

    // Lays the Vault's store and index out afresh.
    [[nodiscard]] std::expected<Packed, Error> pack(bool storage = true,
                                                    bool index = true) const {
        auto answer = take<raw::write_pack_vault>(
            raw::write_pack_vault(root_.c_str(), storage, index));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWritePack> handle{std::move(*answer)};
        return Packed{.changed = handle ? raw::write_pack_changed(handle.get()) : false};
    }

    // Locks the Vault for as long as the guard lives. Nothing when the Vault is
    // locked already.
    [[nodiscard]] std::optional<LockGuard> lock() const {
        detail::VaultLock guard{raw::vault_lock_acquire(detail::WriterAccess::vault(*reading_))};
        if (!guard) {
            return std::nullopt;
        }
        return LockGuard{std::move(guard)};
    }

    [[nodiscard]] const VaultReading &reading() const noexcept { return *reading_; }

private:
    const VaultReading *reading_;
    std::filesystem::path root_path_;
    std::string root_;
};

} // namespace rola
