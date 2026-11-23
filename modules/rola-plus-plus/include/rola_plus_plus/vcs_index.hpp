#pragma once

#include <cstdint>
#include <expected>
#include <filesystem>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <utility>
#include <variant>
#include <vector>

#include <rola_plus_plus/gen/raw.hpp>
#include <rola_plus_plus/gen/release.hpp>

#include "detail.hpp"
#include "error.hpp"
#include "error_map.hpp"

namespace rola {

// One version in a listing, with its traced number and the variant it points at.
struct VcsVersionItem {
    std::optional<std::uint64_t> number;
    std::string version;
    std::optional<std::string> variant;
};

// One variant in a listing, with the version it is based on.
struct VcsVariantItem {
    std::string variant;
    std::optional<std::string> version;
};

// Everything a VCS index holds, as its three listings name it.
struct VcsIndexListing {
    std::vector<VcsVersionItem> versions;
    std::vector<VcsVariantItem> variants;
    std::vector<std::string> string_hashes;
};

// A Creator: the name it is.
struct VcsIndexCreatorView {
    std::string text;
};

// A Message: the text it is.
struct VcsIndexMessageView {
    std::string text;
};

// A Variant: the hashes it carries.
struct VcsIndexVariantView {
    std::string hash;
    std::string base_version;
    std::string storage;
    std::optional<std::string> join;
};

// A Version: its key, the variant it points at, and the number traced for it.
struct VcsIndexVersionView {
    std::optional<std::uint64_t> number;
    std::string hash;
    std::string variant;
};

// One index object as the ABI reads it. The four kinds have different fields,
// so the value names which one it is rather than leaving three of them empty.
using VcsIndexObjectView =
    std::variant<VcsIndexCreatorView, VcsIndexMessageView, VcsIndexVariantView,
                 VcsIndexVersionView>;

// The VCS index a Workspace or a Vault keeps: versions, variants and text
// objects, read by key.
class VcsIndexScope {
public:
    explicit VcsIndexScope(std::filesystem::path root)
        : root_(std::filesystem::absolute(std::move(root)).lexically_normal()) {}

    [[nodiscard]] const std::filesystem::path &root() const noexcept { return root_; }

private:
    std::filesystem::path root_;
};

class VcsIndexReading;

namespace detail {
struct VcsIndexAccess;
}

// What an index holds at one moment. The listings are read out whole at read
// time; one object is read by its key when a caller asks, which is why the
// native index handle is kept for the reading's lifetime and the reading moves.
class VcsIndexReading {
public:
    VcsIndexReading(VcsIndexReading &&) noexcept = default;
    VcsIndexReading &operator=(VcsIndexReading &&) noexcept = default;
    VcsIndexReading(const VcsIndexReading &) = delete;
    VcsIndexReading &operator=(const VcsIndexReading &) = delete;
    ~VcsIndexReading() = default;

    [[nodiscard]] const std::filesystem::path &root() const noexcept { return root_; }

    // The root version's key, or an empty view when the index holds no root.
    [[nodiscard]] std::string_view root_version() const noexcept { return root_version_; }

    [[nodiscard]] const VcsIndexListing &listing() const noexcept { return listing_; }

    // One index object as the kind it turned out to be. A key the index does not
    // hold is a failure of `ErrorKind::not_found`.
    [[nodiscard]] std::expected<VcsIndexObjectView, Error> object_view(
        std::string_view hash) const;

    // One version's number, traced through the chain. Nothing when the chain
    // does not say, or when the object is not a version.
    [[nodiscard]] std::expected<std::optional<std::uint64_t>, Error> version_number(
        std::string_view hash) const;

private:
    friend struct detail::VcsIndexAccess;

    VcsIndexReading(std::filesystem::path root, Owned<RolaVCSIndex> index,
                    std::string root_version, VcsIndexListing listing)
        : root_(std::move(root)), index_(std::move(index)),
          root_version_(std::move(root_version)), listing_(std::move(listing)) {}

    std::filesystem::path root_;
    Owned<RolaVCSIndex> index_;
    std::string root_version_;
    VcsIndexListing listing_;
};

namespace detail {

inline Error vcs_index_error(const RolaVCSIndexReadingError *error_handle) {
    return from(error_handle);
}

inline std::expected<VcsIndexObjectView, Error> object_view_of(
    const RolaVCSIndexObjectView &view) {
    switch (view.tag) {
    case RolaVCSIndexObjectView_Creator:
        return VcsIndexObjectView{
            std::in_place_type<VcsIndexCreatorView>,
            VcsIndexCreatorView{.text = take(view.payload.Creator.text)}};
    case RolaVCSIndexObjectView_Message:
        return VcsIndexObjectView{
            std::in_place_type<VcsIndexMessageView>,
            VcsIndexMessageView{.text = take(view.payload.Message.text)}};
    case RolaVCSIndexObjectView_Variant:
        return VcsIndexObjectView{
            std::in_place_type<VcsIndexVariantView>,
            VcsIndexVariantView{
                .hash = key(take(view.payload.Variant.hash)),
                .base_version = key(take(view.payload.Variant.base_version)),
                .storage = key(take(view.payload.Variant.storage)),
                .join = presence(key(take(view.payload.Variant.join))),
            }};
    case RolaVCSIndexObjectView_Version:
        return VcsIndexObjectView{
            std::in_place_type<VcsIndexVersionView>,
            VcsIndexVersionView{
                .number = view.payload.Version.number_present
                              ? std::optional<std::uint64_t>{view.payload.Version.number}
                              : std::nullopt,
                .hash = key(take(view.payload.Version.hash)),
                .variant = key(take(view.payload.Version.variant)),
            }};
    }

    return std::unexpected{
        Error{ErrorKind::unreadable, {}, "the index object is of a kind this build does not read"}};
}

inline VcsIndexListing listing_of(const RolaVCSIndexListing *handle) {
    VcsIndexListing listing;

    const auto versions = raw::vcs_index_listing_version_count(handle);
    listing.versions.reserve(versions);
    for (std::uintptr_t index = 0; index < versions; ++index) {
        Owned<RolaVCSIndexVersionItem> one{raw::vcs_index_listing_version_at(handle, index)};
        if (!one) {
            continue;
        }
        auto version = key(take(raw::vcs_index_version_item_version(one.get())));
        if (version.empty()) {
            continue;
        }
        listing.versions.push_back(VcsVersionItem{
            .number = raw::vcs_index_version_item_version_num_present(one.get())
                          ? std::optional<std::uint64_t>{
                                raw::vcs_index_version_item_version_num(one.get())}
                          : std::nullopt,
            .version = std::move(version),
            .variant = presence(key(take(raw::vcs_index_version_item_variant(one.get())))),
        });
    }

    const auto variants = raw::vcs_index_listing_variant_count(handle);
    listing.variants.reserve(variants);
    for (std::uintptr_t index = 0; index < variants; ++index) {
        Owned<RolaVCSIndexVariantItem> one{raw::vcs_index_listing_variant_at(handle, index)};
        if (!one) {
            continue;
        }
        auto variant = key(take(raw::vcs_index_variant_item_variant(one.get())));
        if (variant.empty()) {
            continue;
        }
        listing.variants.push_back(VcsVariantItem{
            .variant = std::move(variant),
            .version = presence(key(take(raw::vcs_index_variant_item_version(one.get())))),
        });
    }

    const auto strings = raw::vcs_index_listing_string_count(handle);
    listing.string_hashes.reserve(strings);
    for (std::uintptr_t index = 0; index < strings; ++index) {
        auto one = key(take(raw::vcs_index_listing_string_at(handle, index)));
        if (!one.empty()) {
            listing.string_hashes.push_back(std::move(one));
        }
    }

    return listing;
}

struct VcsIndexAccess {
    static VcsIndexReading make(std::filesystem::path root, Owned<RolaVCSIndex> index,
                                std::string root_version, VcsIndexListing listing) {
        return VcsIndexReading{std::move(root), std::move(index), std::move(root_version),
                                 std::move(listing)};
    }
};

} // namespace detail

inline std::expected<VcsIndexObjectView, Error> VcsIndexReading::object_view(
    std::string_view hash) const {
    const std::string wanted{hash};
    auto answer = take<raw::vcs_index_object_view>(
        raw::vcs_index_object_view(index_.get(), wanted.c_str()));
    if (!answer) {
        return std::unexpected{detail::vcs_index_error(answer.error().get())};
    }

    Owned<RolaVCSIndexObjectView> view{std::move(*answer)};
    if (!view) {
        return std::unexpected{
            Error{ErrorKind::not_found, wanted, "the index holds no such object"}};
    }

    return detail::object_view_of(*view);
}

inline std::expected<std::optional<std::uint64_t>, Error> VcsIndexReading::version_number(
    std::string_view hash) const {
    auto view = object_view(hash);
    if (!view) {
        return std::unexpected{view.error()};
    }

    const auto *version = std::get_if<VcsIndexVersionView>(&*view);
    if (version == nullptr) {
        return std::optional<std::uint64_t>{};
    }
    return version->number;
}

// Read the index a place keeps: its root version and its three listings. A place
// that is not there, or an index that does not read back, is the failure.
inline std::expected<VcsIndexReading, Error> read(const VcsIndexScope &scope) {
    const auto path = detail::utf8(scope.root());

    Owned<RolaVCSIndex> index{raw::vcs_index_locate(path.c_str())};
    if (!index) {
        return std::unexpected{Error{ErrorKind::no_index, path, "no VCS index is there"}};
    }

    auto root_version = detail::take(raw::vcs_index_root_version(index.get()));
    if (root_version == "null") {
        root_version.clear();
    }

    Owned<RolaVCSIndexListing> listing{raw::vcs_index_list(index.get())};
    if (!listing) {
        return std::unexpected{Error{ErrorKind::unreadable, path, "the index could not be listed"}};
    }

    return detail::VcsIndexAccess::make(scope.root(), std::move(index), std::move(root_version),
                                        detail::listing_of(listing.get()));
}

} // namespace rola
