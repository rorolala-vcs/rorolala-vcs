#pragma once

#include <cstdint>
#include <expected>
#include <filesystem>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include <rola_plus_plus/gen/constants.hpp>
#include <rola_plus_plus/gen/raw.hpp>
#include <rola_plus_plus/gen/release.hpp>

#include "detail.hpp"
#include "error.hpp"
#include "layout.hpp"
#include "status.hpp"
#include "storage.hpp"
#include "tree.hpp"
#include "variant_graph.hpp"
#include "vcs_index.hpp"

namespace rola {

// One Vault a Workspace knows, as its configuration names it.
struct VaultBinding {
    std::string name;
    std::string address;
    bool is_default = false;
};

// One store a place binds, as its configuration names it.
struct StorageBinding {
    std::string path;
    std::filesystem::path root;
    bool is_own = false;
    bool present = false;
    bool writable = false;
    std::optional<std::string> reason;
};

// What a collecting run is told, as a Workspace's configuration says it.
struct GcConfig {
    std::optional<std::uint64_t> grace;
};

// A Workspace's configuration: the Vaults it knows, what a collecting run
// keeps and the stores it works on.
struct WorkspaceConfig {
    std::vector<VaultBinding> vaults;
    std::optional<std::string> default_vault;
    std::optional<GcConfig> gc;
    std::vector<StorageBinding> storages;
};

class WorkspaceScope;

class WorkspaceReading;

namespace detail {
struct WorkspaceAccess;
struct WriterAccess;
}

// A Workspace: the directory a run works in, and everything under it. It holds
// the root and nothing else, so it copies and compares as a path does. Finding
// the root is the only check this side makes.
class WorkspaceScope {
public:
    explicit WorkspaceScope(std::filesystem::path any_directory_inside)
        : root_(std::filesystem::absolute(std::move(any_directory_inside)).lexically_normal()) {}

    // Look upward from a path for the directory that makes a Workspace.
    [[nodiscard]] static std::expected<WorkspaceScope, Error> open(
        std::filesystem::path start);

    [[nodiscard]] const std::filesystem::path &root() const noexcept { return root_; }

    [[nodiscard]] StatusScope status() const { return StatusScope{root_}; }

    // The Layout the Workspace works in.
    [[nodiscard]] LayoutScope layout() const { return LayoutScope{root_}; }

    // The copy of a Vault's truth Layout a fetch left here.
    [[nodiscard]] std::expected<LayoutScope, Error> truth_layout(std::string_view vault) const;

    // The store the Workspace reads from. The store a write goes to is chosen
    // from the configuration a reading holds, so a writer names it there.
    [[nodiscard]] StorageScope storage() const { return StorageScope{root_}; }

    [[nodiscard]] VcsIndexScope vcs_index() const { return VcsIndexScope{root_}; }

    [[nodiscard]] TreeScope tree() const { return TreeScope{root_}; }

    // A tree at a path under the Workspace, refused when it leaves the root.
    [[nodiscard]] std::expected<TreeScope, Error> tree_at(std::string_view path) const;

    [[nodiscard]] VariantGraphScope variant_graph(VariantGraphRequest request = {}) const {
        return VariantGraphScope{root_, std::move(request)};
    }

private:
    std::filesystem::path root_;
};

// The native Workspace a write holds, with the configuration read beside it.
// Only a read makes one, and it moves because the handle does.
class WorkspaceReading {
public:
    WorkspaceReading(WorkspaceReading &&) noexcept = default;
    WorkspaceReading &operator=(WorkspaceReading &&) noexcept = default;
    WorkspaceReading(const WorkspaceReading &) = delete;
    WorkspaceReading &operator=(const WorkspaceReading &) = delete;
    ~WorkspaceReading() = default;

    // The root the native handle was located at. A writer names its calls after
    // it, so the value is kept rather than asked of the handle again.
    [[nodiscard]] const std::filesystem::path &root() const noexcept { return root_; }

    // Whether the Workspace is locked by a run.
    [[nodiscard]] bool locked() const noexcept {
        return raw::workspace_is_locking(workspace_.get());
    }

    [[nodiscard]] const WorkspaceConfig &config() const noexcept { return config_; }

private:
    friend struct detail::WorkspaceAccess;
    friend struct detail::WriterAccess;

    WorkspaceReading(Owned<RolaWorkspace> workspace, WorkspaceConfig config,
                     std::filesystem::path root)
        : workspace_(std::move(workspace)), config_(std::move(config)),
          root_(std::move(root)) {}

    Owned<RolaWorkspace> workspace_;
    WorkspaceConfig config_;
    std::filesystem::path root_;
};

namespace detail {

// The configuration file and the stores it names, read out while the native
// handles are alive and kept as values.
inline WorkspaceConfig read_workspace_config(const std::string &directory) {
    WorkspaceConfig config;

    Owned<RolaWorkspaceConfig> handle{raw::workspace_config_read(directory.c_str())};
    if (handle) {
        const auto many = raw::workspace_config_vault_count(handle.get());
        config.vaults.reserve(many);
        for (std::uintptr_t index = 0; index < many; ++index) {
            Owned<RolaWorkspaceVault> vault{raw::workspace_config_vault_at(handle.get(), index)};
            if (!vault) {
                continue;
            }
            config.vaults.push_back(VaultBinding{
                .name = take(raw::workspace_vault_name(vault.get())),
                .address = take(raw::workspace_vault_address(vault.get())),
                .is_default = raw::workspace_vault_is_default(vault.get()),
            });
        }

        auto default_vault = take(raw::workspace_config_default_vault(handle.get()));
        if (!default_vault.empty()) {
            config.default_vault = std::move(default_vault);
        }

        if (raw::workspace_config_gc_present(handle.get())) {
            const auto grace = raw::workspace_config_gc_grace(handle.get());
            GcConfig gc;
            if (grace >= 0) {
                gc.grace = static_cast<std::uint64_t>(grace);
            }
            config.gc = std::move(gc);
        }
    }

    Owned<RolaStorageBindings> storages{raw::workspace_storages_new(directory.c_str())};
    if (storages) {
        const auto many = raw::storage_bindings_count(storages.get());
        config.storages.reserve(many);
        for (std::uintptr_t index = 0; index < many; ++index) {
            Owned<RolaStorageBinding> binding{raw::storage_bindings_at(storages.get(), index)};
            if (!binding) {
                continue;
            }

            StorageBinding one{
                .path = take(raw::storage_binding_path(binding.get())),
                .root = from_utf8(take(raw::storage_binding_root(binding.get()))),
                .is_own = raw::storage_binding_own(binding.get()),
                .present = raw::storage_binding_present(binding.get()),
                .writable = raw::storage_binding_writable(binding.get()),
                .reason = std::nullopt,
            };

            auto reason = take(raw::storage_binding_reason(binding.get()));
            if (!reason.empty()) {
                one.reason = std::move(reason);
            }
            config.storages.push_back(std::move(one));
        }
    }

    return config;
}

struct WorkspaceAccess {
    static WorkspaceReading make(Owned<RolaWorkspace> workspace, WorkspaceConfig config,
                                 std::filesystem::path root) {
        return WorkspaceReading{std::move(workspace), std::move(config), std::move(root)};
    }
};

} // namespace detail

inline std::expected<WorkspaceScope, Error> WorkspaceScope::open(
    std::filesystem::path start) {
    std::error_code error_code;
    auto at = std::filesystem::absolute(std::move(start), error_code).lexically_normal();
    if (error_code) {
        return std::unexpected{Error{ErrorKind::invalid_argument, detail::utf8(at),
                                     error_code.message()}};
    }

    for (;;) {
        std::error_code check;
        const auto data = at / workspace_data_dir();
        if (std::filesystem::is_directory(data, check) && !check) {
            return WorkspaceScope{at};
        }

        const auto parent = at.parent_path();
        if (parent == at || parent.empty()) {
            break;
        }
        at = parent;
    }

    return std::unexpected{Error{ErrorKind::no_workspace, detail::utf8(at),
                                 "no Workspace holds the directory"}};
}

inline std::expected<LayoutScope, Error> WorkspaceScope::truth_layout(
    std::string_view vault) const {
    const bool one_component = !vault.empty() && vault != "." && vault != ".."
                               && vault.front() != ' ' && vault.back() != ' '
                               && vault.find_first_of("/\\:") == std::string_view::npos;
    if (!one_component) {
        return std::unexpected{Error{ErrorKind::invalid_argument, std::string{vault},
                                     "the Vault name is not one path component"}};
    }

    return LayoutScope{root_, vault_layout_name() + "@" + std::string{vault}};
}

inline std::expected<TreeScope, Error> WorkspaceScope::tree_at(std::string_view path) const {
    const auto at = detail::subtree(root_, path);
    if (!at) {
        return std::unexpected{Error{ErrorKind::invalid_argument, std::string{path},
                                     "the path leaves the Workspace"}};
    }
    return TreeScope{*at};
}

// Read the native Workspace a scope names, and its configuration with it.
inline std::expected<WorkspaceReading, Error> read(const WorkspaceScope &scope) {
    const auto path = detail::utf8(scope.root());
    Owned<RolaWorkspace> workspace{raw::workspace_locate(path.c_str())};
    if (!workspace) {
        return std::unexpected{Error{ErrorKind::no_workspace, path,
                                     "no Workspace holds the directory"}};
    }
    return detail::WorkspaceAccess::make(std::move(workspace),
                                         detail::read_workspace_config(path), scope.root());
}

} // namespace rola
