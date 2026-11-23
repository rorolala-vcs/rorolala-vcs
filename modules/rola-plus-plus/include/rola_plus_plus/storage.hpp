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
#include "error_map.hpp"

namespace rola {

// One piece of content a store keeps, and how many bytes the chunk holds.
struct Chunk {
    std::string key;
    std::uint64_t length = 0;
};

// A chunked object: the chunks it is put back together from, in order.
class StorageManifest {
public:
    explicit StorageManifest(std::vector<Chunk> chunks) : chunks_(std::move(chunks)) {}

    [[nodiscard]] std::span<const Chunk> chunks() const noexcept { return chunks_; }

private:
    std::vector<Chunk> chunks_;
};

// The content-addressed store a Workspace or a Vault keeps. Two paths are
// carried: the place whose stores are read, and the store a write goes to. They
// are the same when the caller names one place; a caller that has resolved a
// binding passes both, which is what the C# scope does.
class StorageScope {
public:
    explicit StorageScope(std::filesystem::path root)
        : root_(std::filesystem::absolute(std::move(root)).lexically_normal()),
          place_root_(root_) {}

    StorageScope(std::filesystem::path root, std::filesystem::path place_root)
        : root_(std::filesystem::absolute(std::move(root)).lexically_normal()),
          place_root_(std::filesystem::absolute(std::move(place_root)).lexically_normal()) {}

    // The store a write goes to.
    [[nodiscard]] const std::filesystem::path &root() const noexcept { return root_; }

    // The place whose stores this scope reads.
    [[nodiscard]] const std::filesystem::path &place_root() const noexcept { return place_root_; }

private:
    std::filesystem::path root_;
    std::filesystem::path place_root_;
};

class StorageReading;

namespace detail {
struct StorageAccess;
}

// What a store holds at one moment. The two listings are read out whole; the
// chunks of one content key are read on demand through `manifest`, because a
// caller asks about one key at a time.
class StorageReading {
public:
    StorageReading(const StorageReading &) = default;
    StorageReading &operator=(const StorageReading &) = default;
    StorageReading(StorageReading &&) noexcept = default;
    StorageReading &operator=(StorageReading &&) noexcept = default;
    ~StorageReading() = default;

    [[nodiscard]] const std::filesystem::path &place_root() const noexcept { return place_root_; }

    // Every object key the store holds.
    [[nodiscard]] std::span<const std::string> keys() const noexcept { return keys_; }

    // Every content key the store keeps as a manifest.
    [[nodiscard]] std::span<const std::string> manifests() const noexcept { return manifests_; }

    // The chunks a chunked object is put back together from, or nothing when the
    // content is kept whole.
    [[nodiscard]] std::expected<std::optional<StorageManifest>, Error> manifest(
        std::string_view key) const;

private:
    friend struct detail::StorageAccess;

    StorageReading(std::filesystem::path place_root, std::vector<std::string> keys,
                   std::vector<std::string> manifests)
        : place_root_(std::move(place_root)), keys_(std::move(keys)),
          manifests_(std::move(manifests)) {}

    std::filesystem::path place_root_;
    std::vector<std::string> keys_;
    std::vector<std::string> manifests_;
};

namespace detail {

inline Error storage_error(const RolaStorageError *error_handle) { return from(error_handle); }

} // namespace detail

namespace detail {

struct StorageAccess {
    static StorageReading make(std::filesystem::path place_root, std::vector<std::string> keys,
                               std::vector<std::string> manifests) {
        return StorageReading{std::move(place_root), std::move(keys), std::move(manifests)};
    }
};

} // namespace detail

inline std::expected<std::optional<StorageManifest>, Error> StorageReading::manifest(
    std::string_view key) const {
    const auto directory = detail::utf8(place_root_);
    const std::string hash{key};

    auto answer =
        take<raw::storage_manifest>(raw::storage_manifest(directory.c_str(), hash.c_str()));
    if (!answer) {
        return std::unexpected{detail::storage_error(answer.error().get())};
    }

    Owned<RolaStorageManifest> handle{std::move(*answer)};
    if (!handle) {
        return std::optional<StorageManifest>{};
    }

    const auto many = raw::storage_manifest_chunk_count(handle.get());
    if (many == 0) {
        return std::optional<StorageManifest>{};
    }

    std::vector<Chunk> chunks;
    chunks.reserve(many);
    for (std::uintptr_t index = 0; index < many; ++index) {
        Owned<RolaStorageChunk> one{raw::storage_manifest_chunk_at(handle.get(), index)};
        if (!one) {
            continue;
        }
        chunks.push_back(Chunk{
            .key = detail::take(raw::storage_chunk_key(one.get())),
            .length = raw::storage_chunk_len(one.get()),
        });
    }

    return std::optional<StorageManifest>{StorageManifest{std::move(chunks)}};
}

// Read what a store holds. A place that is not there, or a store that does not
// read back, is the failure.
inline std::expected<StorageReading, Error> read(const StorageScope &scope) {
    const auto directory = detail::utf8(scope.place_root());

    auto objects = take<raw::storage_objects>(raw::storage_objects(directory.c_str()));
    if (!objects) {
        return std::unexpected{detail::storage_error(objects.error().get())};
    }

    auto manifests = take<raw::storage_manifests>(raw::storage_manifests(directory.c_str()));
    if (!manifests) {
        return std::unexpected{detail::storage_error(manifests.error().get())};
    }

    const auto strings_of = [](const RolaStorageKeys *keys) {
        std::vector<std::string> out;
        if (keys == nullptr) {
            return out;
        }
        const auto many = raw::storage_keys_count(keys);
        out.reserve(many);
        for (std::uintptr_t index = 0; index < many; ++index) {
            auto one = detail::take(raw::storage_keys_at(keys, index));
            if (!one.empty()) {
                out.push_back(std::move(one));
            }
        }
        return out;
    };

    Owned<RolaStorageKeys> objects_handle{std::move(*objects)};
    Owned<RolaStorageKeys> manifests_handle{std::move(*manifests)};

    return detail::StorageAccess::make(scope.place_root(), strings_of(objects_handle.get()),
                                       strings_of(manifests_handle.get()));
}

} // namespace rola
