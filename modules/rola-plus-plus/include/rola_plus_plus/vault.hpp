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
#include "storage.hpp"
#include "tree.hpp"
#include "vcs_index.hpp"
#include "variant_graph.hpp"
#include "workspace.hpp"

namespace rola {

// A Vault's configuration: what it says about itself. The stores are the ones
// the Vault works on, in the order it reaches for them, so the record shares the
// `StorageBinding` a Workspace's configuration uses.
struct VaultConfig {
    std::string name;
    std::string description;
    std::vector<std::string> admins;
    std::uint16_t max_concurrency = 0;
    std::vector<StorageBinding> storages;
};

class VaultScope;

class VaultReading;

namespace detail {
struct VaultAccess;
struct WriterAccess;
}

// A Vault: the directory a server-side Vault keeps, and everything under it. It
// holds the root and nothing else, so it copies and compares as a path does.
// Finding the root is the only check this side makes.
class VaultScope {
public:
    explicit VaultScope(std::filesystem::path any_directory_inside)
        : root_(std::filesystem::absolute(std::move(any_directory_inside)).lexically_normal()) {}

    // Look upward from a path for the directory that makes a Vault.
    [[nodiscard]] static std::expected<VaultScope, Error> open(std::filesystem::path start);

    [[nodiscard]] const std::filesystem::path &root() const noexcept { return root_; }

    // The truth Layout the Vault keeps.
    [[nodiscard]] LayoutScope layout() const {
        return LayoutScope{root_, vault_layout_name()};
    }

    [[nodiscard]] StorageScope storage() const { return StorageScope{root_}; }

    [[nodiscard]] VcsIndexScope vcs_index() const { return VcsIndexScope{root_}; }

    [[nodiscard]] TreeScope tree() const { return TreeScope{root_}; }

    // A tree at a path under the Vault, refused when it leaves the root.
    [[nodiscard]] std::expected<TreeScope, Error> tree_at(std::string_view path) const;

    [[nodiscard]] VariantGraphScope variant_graph(VariantGraphRequest request = {}) const {
        return VariantGraphScope{root_, std::move(request)};
    }

private:
    std::filesystem::path root_;
};

// The native Vault a write holds, with the configuration read beside it. Only a
// read makes one, and it moves because the handle does.
class VaultReading {
public:
    VaultReading(VaultReading &&) noexcept = default;
    VaultReading &operator=(VaultReading &&) noexcept = default;
    VaultReading(const VaultReading &) = delete;
    VaultReading &operator=(const VaultReading &) = delete;
    ~VaultReading() = default;

    // The root the native handle was located at. A writer names its calls after
    // it, so the value is kept rather than asked of the handle again.
    [[nodiscard]] const std::filesystem::path &root() const noexcept { return root_; }

    // Whether the Vault is locked by a run.
    [[nodiscard]] bool locked() const noexcept {
        return raw::vault_is_locking(vault_.get());
    }

    [[nodiscard]] const VaultConfig &config() const noexcept { return config_; }

private:
    friend struct detail::VaultAccess;
    friend struct detail::WriterAccess;

    VaultReading(Owned<RolaVault> vault, VaultConfig config, std::filesystem::path root)
        : vault_(std::move(vault)), config_(std::move(config)), root_(std::move(root)) {}

    Owned<RolaVault> vault_;
    VaultConfig config_;
    std::filesystem::path root_;
};

namespace detail {

// The configuration file a Vault reads to describe itself, and the stores it
// names. The loop over storage bindings repeats the one `WorkspaceReading`
// performs because the two configurations are read from different native calls.
inline VaultConfig read_vault_config(const std::string &directory) {
    VaultConfig config;

    Owned<RolaVaultConfig> handle{raw::vault_config_read(directory.c_str())};
    if (handle) {
        config.name = take(raw::vault_config_name(handle.get()));
        config.description = take(raw::vault_config_description(handle.get()));
        config.max_concurrency = raw::vault_config_max_concurrency(handle.get());

        const auto many = raw::vault_config_admin_count(handle.get());
        config.admins.reserve(many);
        for (std::uintptr_t index = 0; index < many; ++index) {
            config.admins.push_back(take(raw::vault_config_admin_at(handle.get(), index)));
        }
    }

    Owned<RolaStorageBindings> storages{raw::vault_storages_new(directory.c_str())};
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

struct VaultAccess {
    static VaultReading make(Owned<RolaVault> vault, VaultConfig config,
                             std::filesystem::path root) {
        return VaultReading{std::move(vault), std::move(config), std::move(root)};
    }
};

} // namespace detail

inline std::expected<VaultScope, Error> VaultScope::open(std::filesystem::path start) {
    std::error_code error_code;
    auto at = std::filesystem::absolute(std::move(start), error_code).lexically_normal();
    if (error_code) {
        return std::unexpected{Error{ErrorKind::invalid_argument, detail::utf8(at),
                                     error_code.message()}};
    }

    for (;;) {
        std::error_code check;
        const auto config = at / vault_config_path();
        if (std::filesystem::is_regular_file(config, check) && !check) {
            return VaultScope{at};
        }

        const auto parent = at.parent_path();
        if (parent == at || parent.empty()) {
            break;
        }
        at = parent;
    }

    return std::unexpected{Error{ErrorKind::no_vault, detail::utf8(at),
                                 "no Vault holds the directory"}};
}

inline std::expected<TreeScope, Error> VaultScope::tree_at(std::string_view path) const {
    const auto at = detail::subtree(root_, path);
    if (!at) {
        return std::unexpected{
            Error{ErrorKind::invalid_argument, std::string{path}, "the path leaves the Vault"}};
    }
    return TreeScope{*at};
}

// Read the native Vault a scope names, and its configuration with it.
inline std::expected<VaultReading, Error> read(const VaultScope &scope) {
    const auto path = detail::utf8(scope.root());
    Owned<RolaVault> vault{raw::vault_locate(path.c_str())};
    if (!vault) {
        return std::unexpected{
            Error{ErrorKind::no_vault, path, "no Vault holds the directory"}};
    }
    return detail::VaultAccess::make(std::move(vault), detail::read_vault_config(path),
                                     scope.root());
}

} // namespace rola
