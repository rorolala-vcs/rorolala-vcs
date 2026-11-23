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

// One entry of a Layout: the path it is at and what it records. Every field is
// a value, so the entry outlives the native handle it was read from.
struct LayoutEntry {
    std::string path;
    std::string uuid;
    std::string version;
    std::string owner;
    std::string description;
    std::string preference;
    bool deprecated = false;
};

// A Layout a place holds. The same descriptor names the local Layout (no name)
// and a truth Layout or a fetched copy (a name), which is why it holds both the
// root and the name rather than being three types.
class LayoutScope {
public:
    explicit LayoutScope(std::filesystem::path root,
                         std::optional<std::string> name = std::nullopt)
        : root_(std::filesystem::absolute(std::move(root)).lexically_normal()),
          name_(std::move(name)) {}

    [[nodiscard]] const std::filesystem::path &root() const noexcept { return root_; }

    // Nothing for the Layout being worked in; otherwise the name the native side
    // reads it by, such as `truth` or `truth@<vault>`.
    [[nodiscard]] const std::optional<std::string> &name() const noexcept { return name_; }

private:
    std::filesystem::path root_;
    std::optional<std::string> name_;
};

class LayoutReading;

namespace detail {
struct LayoutAccess;
}

// What a Layout holds at one moment, read out whole so the value has no handle
// behind it.
class LayoutReading {
public:
    LayoutReading(const LayoutReading &) = default;
    LayoutReading &operator=(const LayoutReading &) = default;
    LayoutReading(LayoutReading &&) noexcept = default;
    LayoutReading &operator=(LayoutReading &&) noexcept = default;
    ~LayoutReading() = default;

    [[nodiscard]] std::span<const LayoutEntry> entries() const noexcept { return entries_; }

    // The entry at a path, when the Layout names one. The path is compared as
    // the Layout writes it, since that is the form the entries hold.
    [[nodiscard]] std::optional<LayoutEntry> find(std::string_view path) const {
        for (const auto &one : entries_) {
            if (one.path == path) {
                return one;
            }
        }
        return std::nullopt;
    }

private:
    friend struct detail::LayoutAccess;

    explicit LayoutReading(Owned<RolaLayoutEntries> entries);

    std::vector<LayoutEntry> entries_;
};

namespace detail {

inline Error layout_error(const RolaLayoutError *error_handle) { return from(error_handle); }

} // namespace detail

inline LayoutReading::LayoutReading(Owned<RolaLayoutEntries> entries) {
    if (!entries) {
        return;
    }

    const auto many = raw::layout_entries_count(entries.get());
    entries_.reserve(many);
    for (std::uintptr_t index = 0; index < many; ++index) {
        Owned<RolaLayoutEntry> entry{raw::layout_entries_at(entries.get(), index)};
        if (!entry) {
            continue;
        }

        auto uuid = detail::take(raw::layout_entry_uuid(entry.get()));
        if (uuid.empty()) {
            continue;
        }

        entries_.push_back(LayoutEntry{
            .path = detail::take(raw::layout_entry_path(entry.get())),
            .uuid = std::move(uuid),
            .version = detail::take(raw::layout_entry_version(entry.get())),
            .owner = detail::take(raw::layout_entry_owner(entry.get())),
            .description = detail::take(raw::layout_entry_description(entry.get())),
            .preference = detail::take(raw::layout_entry_preference(entry.get())),
            .deprecated = raw::layout_entry_deprecated(entry.get()),
        });
    }
}

namespace detail {

struct LayoutAccess {
    static LayoutReading make(Owned<RolaLayoutEntries> entries) {
        return LayoutReading{std::move(entries)};
    }
};

} // namespace detail

// Read what a Layout holds. A place that is not there, no Layout under the name,
// or a Layout that does not read back is the failure.
inline std::expected<LayoutReading, Error> read(const LayoutScope &scope) {
    const auto directory = detail::utf8(scope.root());
    const char *name = scope.name() ? scope.name()->c_str() : nullptr;

    auto answer = take<raw::layout_entries>(raw::layout_entries(directory.c_str(), name));
    if (!answer) {
        return std::unexpected{detail::layout_error(answer.error().get())};
    }

    return detail::LayoutAccess::make(std::move(*answer));
}

} // namespace rola
