#pragma once

#include <expected>
#include <filesystem>
#include <string>
#include <string_view>
#include <utility>

#include <rola_plus_plus/gen/raw.hpp>
#include <rola_plus_plus/gen/release.hpp>

#include "detail.hpp"
#include "error.hpp"
#include "storage.hpp"
#include "write.hpp"
#include "write_result.hpp"

namespace rola {

// Puts content into a store a caller has read.
//
// The store exports that take one file (`rola_write_storage_put_file`,
// `rola_storage_extract`) and the one that creates a store have no request
// form, so this writer exposes them directly rather than through a request
// object; step 04 keeps them because the C# writer has them.
class StorageWriter {
public:
    explicit StorageWriter(std::filesystem::path root)
        : root_path_(std::filesystem::absolute(std::move(root)).lexically_normal()),
          root_(detail::utf8(root_path_)) {}

    // Makes a directory a store.
    [[nodiscard]] static std::expected<void, Error> create(std::filesystem::path root) {
        const auto at = detail::utf8(std::filesystem::absolute(std::move(root)).lexically_normal());
        auto answer = take<raw::storage_create>(raw::storage_create(at.c_str()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        return {};
    }

    // Lays the store out afresh.
    [[nodiscard]] std::expected<Packed, Error> pack() const {
        auto answer = take<raw::write_pack_storage>(raw::write_pack_storage(root_.c_str()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWritePack> handle{std::move(*answer)};
        return Packed{.changed = handle ? raw::write_pack_changed(handle.get()) : false};
    }

    // Puts a file's content into the store. An empty `policy` leaves the cut to
    // the store.
    [[nodiscard]] std::expected<Stored, Error> put(std::filesystem::path file,
                                                   std::string_view policy = {}) const {
        const auto path = detail::utf8(file);
        const std::string cut{policy};

        auto answer = take<raw::write_storage_put_file>(
            raw::write_storage_put_file(root_.c_str(), path.c_str(), cut.c_str()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }

        Owned<RolaWriteStoragePut> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, path, "the store answered nothing"}};
        }
        return Stored{.key = detail::take(raw::write_storage_put_key(handle.get())),
                      .chunked = raw::write_storage_put_chunked(handle.get())};
    }

    // Takes the content stored under a key out to a destination.
    [[nodiscard]] std::expected<void, Error> extract(std::string_view key,
                                                     std::filesystem::path destination) const {
        const std::string hash{key};
        const auto path = detail::utf8(destination);

        auto answer = take<raw::storage_extract>(
            raw::storage_extract(root_.c_str(), hash.c_str(), path.c_str()));
        if (!answer) {
            return std::unexpected{detail::storage_error(answer.error().get())};
        }
        return {};
    }

    [[nodiscard]] const std::filesystem::path &root() const noexcept { return root_path_; }

private:
    std::filesystem::path root_path_;
    std::string root_;
};

} // namespace rola
