#pragma once

#include <cstdint>
#include <filesystem>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include <rola_plus_plus/gen/raw.hpp>
#include <rola_plus_plus/gen/release.hpp>

namespace rola::detail {

// The native side takes and answers paths as UTF-8, which is not what
// std::filesystem::path::string() holds on Windows.
inline std::string utf8(const std::filesystem::path &path) {
    const auto text = path.u8string();
    return std::string{reinterpret_cast<const char *>(text.data()), text.size()};
}

// The same bytes with forward slashes, which is how a reading names a path.
inline std::string generic_utf8(const std::filesystem::path &path) {
    const auto text = path.generic_u8string();
    return std::string{reinterpret_cast<const char *>(text.data()), text.size()};
}

// A path built from the UTF-8 a reading or a caller holds.
inline std::filesystem::path from_utf8(std::string_view text) {
    return std::filesystem::path{
        std::u8string{reinterpret_cast<const char8_t *>(text.data()), text.size()}};
}

// An owned C string the native side handed over, kept as a std::string. A null
// is released as correctly as any other value and read as empty.
inline std::string take(char *text) {
    OwnedString value{text};
    return value ? std::string{value.get()} : std::string{};
}

// A C string that belongs to something else, copied out before that owner is
// given back. Reading it through `take` would release a buffer the owner frees.
inline std::string copied(const char *text) {
    return text != nullptr ? std::string{text} : std::string{};
}

// An empty string is how the native side says a field carries nothing, which is
// what a caller reads as no value at all.
inline std::optional<std::string> presence(std::string text) {
    if (text.empty()) {
        return std::nullopt;
    }
    return text;
}

// A hash as the index writes it. The native side sometimes answers the hex
// without its algorithm prefix, and a caller comparing keys needs one shape.
inline std::string key(std::string text) {
    constexpr std::string_view prefix{"blake3:"};
    if (text.empty() || text.starts_with(prefix)) {
        return text;
    }
    return std::string{prefix} + text;
}

// One `_count` and `_at` pair, filled into a vector the reading owns. Reading
// them whole is why a reading outlives the native handle it was made from.
template <class Handle, class Count, class At>
std::vector<std::string> strings(const Handle *handle, Count count, At at) {
    const auto many = count(handle);
    std::vector<std::string> out;
    out.reserve(many);
    for (std::uintptr_t index = 0; index < many; ++index) {
        out.push_back(take(at(handle, index)));
    }
    return out;
}

// The path a reading names it by, when the caller's path lies under the root.
// Both sides are made absolute, so a caller's path is compared with the root
// the reading was made from rather than with the current directory.
inline std::optional<std::string> named(const std::filesystem::path &root,
                                        std::string_view path) {
    std::error_code error;
    const auto at = std::filesystem::absolute(from_utf8(path), error).lexically_normal();
    if (error) {
        return std::nullopt;
    }

    const auto relative = at.lexically_relative(root);
    if (relative.empty() || relative == at || relative == std::filesystem::path{"."}) {
        return std::nullopt;
    }
    if (*relative.begin() == std::filesystem::path{".."}) {
        return std::nullopt;
    }

    return generic_utf8(relative);
}

// A path under a place, refusing one that leaves it. The relative path is the
// caller's, which a scope handed out of a place must not let escape the root.
inline std::optional<std::filesystem::path> subtree(const std::filesystem::path &root,
                                                    std::string_view relative) {
    std::error_code error;
    auto at = std::filesystem::absolute(root / from_utf8(relative), error).lexically_normal();
    if (error) {
        return std::nullopt;
    }

    const auto under = at.lexically_relative(root);
    if (under.empty() || under == at) {
        return std::nullopt;
    }
    if (*under.begin() == std::filesystem::path{".."}) {
        return std::nullopt;
    }

    return at;
}

} // namespace rola::detail
