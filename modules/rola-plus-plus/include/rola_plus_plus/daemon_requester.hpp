#pragma once

#include <cstddef>
#include <cstdint>
#include <expected>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include <rola_plus_plus/gen/outcome.hpp>
#include <rola_plus_plus/gen/raw.hpp>
#include <rola_plus_plus/gen/release.hpp>

#include "account.hpp"
#include "detail.hpp"
#include "error.hpp"
#include "error_map.hpp"
#include "write.hpp"

namespace rola {

// One entry a batch write asks the Vault to create. `kind` is `tree` for a tree
// and anything else for a file, which is how the ABI reads it.
struct DaemonRemoteCreate {
    std::string path;
    std::string uuid;
    std::string version;
    std::string kind;
};

// One version a batch write asks the Vault to set.
struct DaemonRemoteSet {
    std::string uuid;
    std::string version;
};

// Which outcome an ownership request reached.
enum class DaemonOwnershipKind { owner, missing, held_by };

// What became of one ownership request. `owner` names the holder the entry now
// has, the other account that refused, or nobody.
struct DaemonOwnership {
    DaemonOwnershipKind kind = DaemonOwnershipKind::missing;
    std::string owner;
};

// Which outcome a write reached.
enum class DaemonEntryWriteKind { written, exists, taken, missing, refused, malformed };

// What became of one write.
struct DaemonEntryWrite {
    DaemonEntryWriteKind kind = DaemonEntryWriteKind::malformed;
};

// Which outcome a walk of a chain reached.
enum class DaemonChainKind { brought, not_a_version, not_a_variant, missing };

// What became of a chain the Vault was asked for, and the hash the walk stopped
// at when it names one.
struct DaemonChain {
    DaemonChainKind kind = DaemonChainKind::missing;
    std::string key;
};

// Which outcome a removal reached.
enum class DaemonRemoveKind { removed, missing, refused, malformed };

// What became of a removal, and the account that holds the entry when the
// removal was refused.
struct DaemonRemove {
    DaemonRemoveKind kind = DaemonRemoveKind::malformed;
    std::string owner;
};

// Which outcome a move reached.
enum class DaemonPathMove { moved, missing, taken, malformed };

namespace detail {

inline DaemonOwnershipKind ownership_kind_of(RolaDaemonOwnershipKind kind) {
    switch (kind) {
    case RolaDaemonOwnershipKind_Owner:
        return DaemonOwnershipKind::owner;
    case RolaDaemonOwnershipKind_Missing:
        return DaemonOwnershipKind::missing;
    case RolaDaemonOwnershipKind_HeldBy:
        return DaemonOwnershipKind::held_by;
    }

    return DaemonOwnershipKind::missing;
}

inline DaemonEntryWriteKind entry_write_kind_of(RolaDaemonEntryWriteKind kind) {
    switch (kind) {
    case RolaDaemonEntryWriteKind_Written:
        return DaemonEntryWriteKind::written;
    case RolaDaemonEntryWriteKind_Exists:
        return DaemonEntryWriteKind::exists;
    case RolaDaemonEntryWriteKind_Taken:
        return DaemonEntryWriteKind::taken;
    case RolaDaemonEntryWriteKind_Missing:
        return DaemonEntryWriteKind::missing;
    case RolaDaemonEntryWriteKind_Refused:
        return DaemonEntryWriteKind::refused;
    case RolaDaemonEntryWriteKind_Malformed:
        return DaemonEntryWriteKind::malformed;
    }

    return DaemonEntryWriteKind::malformed;
}

inline DaemonChainKind chain_kind_of(RolaDaemonChainKind kind) {
    switch (kind) {
    case RolaDaemonChainKind_Brought:
        return DaemonChainKind::brought;
    case RolaDaemonChainKind_NotAversion:
        return DaemonChainKind::not_a_version;
    case RolaDaemonChainKind_NotAvariant:
        return DaemonChainKind::not_a_variant;
    case RolaDaemonChainKind_Missing:
        return DaemonChainKind::missing;
    }

    return DaemonChainKind::missing;
}

inline DaemonRemoveKind remove_kind_of(RolaDaemonEntryRemoveKind kind) {
    switch (kind) {
    case RolaDaemonEntryRemoveKind_Removed:
        return DaemonRemoveKind::removed;
    case RolaDaemonEntryRemoveKind_Missing:
        return DaemonRemoveKind::missing;
    case RolaDaemonEntryRemoveKind_Refused:
        return DaemonRemoveKind::refused;
    case RolaDaemonEntryRemoveKind_Malformed:
        return DaemonRemoveKind::malformed;
    }

    return DaemonRemoveKind::malformed;
}

inline DaemonPathMove path_move_of(RolaDaemonPathMoveKind kind) {
    switch (kind) {
    case RolaDaemonPathMoveKind_Moved:
        return DaemonPathMove::moved;
    case RolaDaemonPathMoveKind_Missing:
        return DaemonPathMove::missing;
    case RolaDaemonPathMoveKind_Taken:
        return DaemonPathMove::taken;
    case RolaDaemonPathMoveKind_Malformed:
        return DaemonPathMove::malformed;
    }

    return DaemonPathMove::malformed;
}

// The outcomes are read out while the batch is alive, so what a caller holds has
// no handle behind it.
inline std::vector<DaemonOwnership> read_ownerships(const RolaDaemonOwnerships *handle) {
    std::vector<DaemonOwnership> out;
    if (handle == nullptr) {
        return out;
    }

    const auto many = raw::daemon_ownerships_count(handle);
    out.reserve(many);
    for (std::uintptr_t index = 0; index < many; ++index) {
        Owned<RolaDaemonOwnershipOutcome> one{raw::daemon_ownerships_at(handle, index)};
        if (!one) {
            continue;
        }
        out.push_back(DaemonOwnership{
            ownership_kind_of(raw::daemon_ownership_outcome_kind(one.get())),
            take(raw::daemon_ownership_outcome_owner(one.get())),
        });
    }
    return out;
}

inline std::vector<DaemonEntryWrite> read_entry_writes(const RolaDaemonEntryWrites *handle) {
    std::vector<DaemonEntryWrite> out;
    if (handle == nullptr) {
        return out;
    }

    const auto many = raw::daemon_entry_writes_count(handle);
    out.reserve(many);
    for (std::uintptr_t index = 0; index < many; ++index) {
        Owned<RolaDaemonEntryWriteOutcome> one{raw::daemon_entry_writes_at(handle, index)};
        if (!one) {
            continue;
        }
        out.push_back(DaemonEntryWrite{
            entry_write_kind_of(raw::daemon_entry_write_outcome_kind(one.get())),
        });
    }
    return out;
}

} // namespace detail

// The daemon actions a Workspace runs against a Vault. It mirrors the C# side:
// each method is one action, it fills the request struct that action reads, and
// it does not interpret what the Vault answers beyond the typed handles the
// header declares.
//
// Every call blocks until the action has run. The caller puts it on a thread if
// it must keep drawing.
class DaemonRequester {
public:
    DaemonRequester(const WorkspaceReading &workspace, const Account &account,
                    std::string target)
        : workspace_(workspace), account_(&account), target_(std::move(target)) {}

    DaemonRequester(const DaemonRequester &) = delete;
    DaemonRequester &operator=(const DaemonRequester &) = delete;
    ~DaemonRequester() = default;

    std::expected<void, Error> sync_all(std::string_view ask) {
        const std::string text{ask};
        RolaDaemonSyncAllRequest request{.ask = text.c_str()};

        auto answer = rola::take<raw::daemon_action_sync_all>(
            raw::daemon_action_sync_all(workspace(), native_account(), target_.c_str(), &request));
        if (!answer) {
            return std::unexpected{detail::from(answer.error().get())};
        }
        return {};
    }

    std::expected<void, Error> sync_index_all() {
        auto answer = rola::take<raw::daemon_action_sync_index_all>(
            raw::daemon_action_sync_index_all(workspace(), native_account(), target_.c_str()));
        if (!answer) {
            return std::unexpected{detail::from(answer.error().get())};
        }
        return {};
    }

    std::expected<std::optional<std::string>, Error> list_remote(std::string_view which) {
        const std::string text{which};
        RolaDaemonListRemoteRequest request{.which = text.c_str()};

        auto answer = rola::take<raw::daemon_action_list_remote>(
            raw::daemon_action_list_remote(workspace(), native_account(), target_.c_str(), &request));
        if (!answer) {
            return std::unexpected{detail::from(answer.error().get())};
        }
        return detail::presence(detail::take(answer->release()));
    }

    std::expected<std::optional<std::string>, Error> sync_hashes(std::string_view units) {
        const std::string text{units};
        RolaDaemonSyncHashesRequest request{.units = text.c_str()};

        auto answer = rola::take<raw::daemon_action_sync_hashes>(
            raw::daemon_action_sync_hashes(workspace(), native_account(), target_.c_str(), &request));
        if (!answer) {
            return std::unexpected{detail::from(answer.error().get())};
        }
        return detail::presence(detail::take(answer->release()));
    }

    std::expected<std::optional<std::string>, Error> list_remote_index(std::string_view which) {
        const std::string text{which};
        RolaDaemonListRemoteIndexRequest request{.which = text.c_str()};

        auto answer = rola::take<raw::daemon_action_list_remote_index>(
            raw::daemon_action_list_remote_index(workspace(), native_account(), target_.c_str(),
                                                 &request));
        if (!answer) {
            return std::unexpected{detail::from(answer.error().get())};
        }
        return detail::presence(detail::take(answer->release()));
    }

    // The object owns the native index object, which the index accessors read.
    std::expected<Owned<RolaVCSIndexObject>, Error> read_remote_index(std::string_view hash) {
        const std::string text{hash};
        RolaDaemonReadRemoteIndexRequest request{.hash = text.c_str()};

        auto answer = rola::take<raw::daemon_action_read_remote_index>(
            raw::daemon_action_read_remote_index(workspace(), native_account(), target_.c_str(),
                                                 &request));
        if (!answer) {
            return std::unexpected{detail::from(answer.error().get())};
        }
        return std::move(*answer);
    }

    std::expected<void, Error> fetch_layout(std::string_view name) {
        const std::string text{name};
        RolaDaemonFetchLayoutRequest request{.name = text.c_str()};

        auto answer = rola::take<raw::daemon_action_fetch_layout>(
            raw::daemon_action_fetch_layout(workspace(), native_account(), target_.c_str(), &request));
        if (!answer) {
            return std::unexpected{detail::from(answer.error().get())};
        }
        return {};
    }

    std::expected<std::vector<DaemonOwnership>, Error> request_ownership(
        std::string_view name, std::span<const std::string_view> uuids) {
        return ownership(name, uuids, false);
    }

    std::expected<std::vector<DaemonOwnership>, Error> giveup_ownership(
        std::string_view name, std::span<const std::string_view> uuids) {
        return ownership(name, uuids, true);
    }

    std::expected<void, Error> fetch_ignore_rules(std::string_view name) {
        const std::string text{name};
        RolaDaemonFetchIgnoreRulesRequest request{.name = text.c_str()};

        auto answer = rola::take<raw::daemon_action_fetch_ignore_rules>(
            raw::daemon_action_fetch_ignore_rules(workspace(), native_account(), target_.c_str(),
                                                  &request));
        if (!answer) {
            return std::unexpected{detail::from(answer.error().get())};
        }
        return {};
    }

    std::expected<DaemonChain, Error> bring_remote_index(std::string_view hash) {
        const std::string text{hash};
        RolaDaemonBringRemoteIndexRequest request{.hash = text.c_str()};

        auto answer = rola::take<raw::daemon_action_bring_remote_index>(
            raw::daemon_action_bring_remote_index(workspace(), native_account(), target_.c_str(),
                                                  &request));
        if (!answer) {
            return std::unexpected{detail::from(answer.error().get())};
        }

        Owned<RolaDaemonChainOutcome> outcome{std::move(*answer)};
        return DaemonChain{
            detail::chain_kind_of(raw::daemon_chain_outcome_kind(outcome.get())),
            detail::take(raw::daemon_chain_outcome_key(outcome.get())),
        };
    }

    std::expected<DaemonRemove, Error> remove_remote_path(std::string_view path) {
        const std::string text{path};
        RolaDaemonRemoveRemotePathRequest request{.path = text.c_str()};

        auto answer = rola::take<raw::daemon_action_remove_remote_path>(
            raw::daemon_action_remove_remote_path(workspace(), native_account(), target_.c_str(),
                                                  &request));
        if (!answer) {
            return std::unexpected{detail::from(answer.error().get())};
        }

        Owned<RolaDaemonEntryRemoveOutcome> outcome{std::move(*answer)};
        return DaemonRemove{
            detail::remove_kind_of(raw::daemon_entry_remove_outcome_kind(outcome.get())),
            detail::take(raw::daemon_entry_remove_outcome_owner(outcome.get())),
        };
    }

    std::expected<DaemonPathMove, Error> move_remote_path(std::string_view from,
                                                          std::string_view to) {
        const std::string one{from};
        const std::string two{to};

        auto answer = rola::take<raw::daemon_action_move_remote_path>(
            raw::daemon_action_move_remote_path(workspace(), native_account(), target_.c_str(),
                                                one.c_str(), two.c_str()));
        if (!answer) {
            return std::unexpected{detail::from(answer.error().get())};
        }

        Owned<RolaDaemonPathMoveOutcome> outcome{std::move(*answer)};
        return detail::path_move_of(raw::daemon_path_move_outcome_kind(outcome.get()));
    }

    std::expected<DaemonEntryWrite, Error> create_remote_entry(std::string_view path,
                                                               std::string_view uuid,
                                                               std::string_view version) {
        const std::string one{path};
        const std::string two{uuid};
        const std::string three{version};

        auto answer = rola::take<raw::daemon_action_create_remote_entry>(
            raw::daemon_action_create_remote_entry(workspace(), native_account(), target_.c_str(),
                                                   one.c_str(), two.c_str(), three.c_str()));
        if (!answer) {
            return std::unexpected{detail::from(answer.error().get())};
        }

        Owned<RolaDaemonEntryWriteOutcome> outcome{std::move(*answer)};
        return DaemonEntryWrite{
            detail::entry_write_kind_of(raw::daemon_entry_write_outcome_kind(outcome.get())),
        };
    }

    std::expected<DaemonEntryWrite, Error> set_remote_version(std::string_view uuid,
                                                              std::string_view version) {
        const std::string one{uuid};
        const std::string two{version};

        auto answer = rola::take<raw::daemon_action_set_remote_version>(
            raw::daemon_action_set_remote_version(workspace(), native_account(), target_.c_str(),
                                                  one.c_str(), two.c_str()));
        if (!answer) {
            return std::unexpected{detail::from(answer.error().get())};
        }

        Owned<RolaDaemonEntryWriteOutcome> outcome{std::move(*answer)};
        return DaemonEntryWrite{
            detail::entry_write_kind_of(raw::daemon_entry_write_outcome_kind(outcome.get())),
        };
    }

    // The creates and sets share one backing store so every `const char *` the
    // ABI reads points into it for the whole call.
    std::expected<std::vector<DaemonEntryWrite>, Error> write_remote_entries(
        std::span<const DaemonRemoteCreate> creates, std::span<const DaemonRemoteSet> sets) {
        std::vector<std::string> fields;
        fields.reserve((creates.size() * 4) + (sets.size() * 2));

        for (const auto &one : creates) {
            fields.push_back(one.path);
            fields.push_back(one.uuid);
            fields.push_back(one.version);
            fields.push_back(one.kind);
        }
        for (const auto &one : sets) {
            fields.push_back(one.uuid);
            fields.push_back(one.version);
        }

        std::vector<RolaDaemonRemoteCreate> create_records;
        create_records.reserve(creates.size());
        for (std::size_t index = 0; index < creates.size(); ++index) {
            const auto base = index * 4;
            create_records.push_back(RolaDaemonRemoteCreate{
                fields[base].c_str(),
                fields[base + 1].c_str(),
                fields[base + 2].c_str(),
                fields[base + 3].c_str(),
            });
        }

        std::vector<RolaDaemonRemoteSet> set_records;
        set_records.reserve(sets.size());
        const auto offset = creates.size() * 4;
        for (std::size_t index = 0; index < sets.size(); ++index) {
            const auto base = offset + (index * 2);
            set_records.push_back(RolaDaemonRemoteSet{
                fields[base].c_str(),
                fields[base + 1].c_str(),
            });
        }

        auto answer = rola::take<raw::daemon_action_write_remote_entries>(
            raw::daemon_action_write_remote_entries(
                workspace(), native_account(), target_.c_str(),
                create_records.empty() ? nullptr : create_records.data(), create_records.size(),
                set_records.empty() ? nullptr : set_records.data(), set_records.size()));
        if (!answer) {
            return std::unexpected{detail::from(answer.error().get())};
        }

        Owned<RolaDaemonEntryWrites> outcome{std::move(*answer)};
        return detail::read_entry_writes(outcome.get());
    }

private:
    [[nodiscard]] const RolaWorkspace *workspace() const {
        return detail::WriterAccess::workspace(workspace_);
    }

    [[nodiscard]] const RolaAccount *native_account() const {
        return detail::AccountAccess::handle(*account_);
    }

    std::expected<std::vector<DaemonOwnership>, Error> ownership(
        std::string_view name, std::span<const std::string_view> uuids, bool giveup) {
        const std::string text{name};
        detail::ViewSpan span{uuids};
        RolaDaemonRequestOwnershipRequest request{.name = text.c_str(), .uuids = span.span()};
        RolaDaemonGiveupOwnershipRequest giveup_request{.name = text.c_str(),
                                                        .uuids = span.span()};

        auto answer = giveup
                          ? rola::take<raw::daemon_action_giveup_ownership>(
                                raw::daemon_action_giveup_ownership(
                                    workspace(), native_account(), target_.c_str(), &giveup_request))
                          : rola::take<raw::daemon_action_request_ownership>(
                                raw::daemon_action_request_ownership(
                                    workspace(), native_account(), target_.c_str(), &request));
        if (!answer) {
            return std::unexpected{detail::from(answer.error().get())};
        }

        Owned<RolaDaemonOwnerships> outcome{std::move(*answer)};
        return detail::read_ownerships(outcome.get());
    }

    const WorkspaceReading &workspace_;
    const Account *account_;
    std::string target_;
};

} // namespace rola
