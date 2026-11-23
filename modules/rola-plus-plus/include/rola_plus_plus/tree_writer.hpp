#pragma once

#include <cstdint>
#include <expected>
#include <filesystem>
#include <span>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include <rola_plus_plus/gen/raw.hpp>
#include <rola_plus_plus/gen/release.hpp>

#include "detail.hpp"
#include "error.hpp"
#include "write.hpp"
#include "write_result.hpp"

namespace rola {

// Writes the work a tree stages. It names the Workspace root the native tree
// exports take, and each call names the tree under it.
class TreeWriter {
public:
    explicit TreeWriter(std::filesystem::path workspace_root)
        : root_path_(std::filesystem::absolute(std::move(workspace_root)).lexically_normal()),
          root_(detail::utf8(root_path_)) {}

    // Writes the description the tree's next version carries, or clears it with
    // an empty text.
    [[nodiscard]] std::expected<TreeDescriptionResult, Error> write_description(
        std::string_view text, std::string_view target = {}) const {
        const std::string words{text};
        const std::string at{target};

        auto answer = take<raw::write_tree_description>(
            raw::write_tree_description(root_.c_str(), at.c_str(), words.c_str()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteTreeDescription> handle{std::move(*answer)};
        return TreeDescriptionResult{
            .bytes = handle ? raw::write_tree_description_bytes(handle.get()) : 0};
    }

    // Makes a directory a tree.
    [[nodiscard]] std::expected<TreeInitResult, Error> init(std::string_view target = {},
                                                              bool force = false) const {
        const std::string at{target};

        auto answer = take<raw::write_tree_init>(
            raw::write_tree_init(root_.c_str(), at.c_str(), force));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteTreeInit> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, at, "the tree was made without an answer"}};
        }
        return TreeInitResult{
            .path = detail::take(raw::write_tree_init_path(handle.get())),
            .collapsed = raw::write_tree_init_collapsed(handle.get()),
            .is_root = raw::write_tree_init_is_root(handle.get()),
        };
    }

    // Resets the staged paths, or the whole staging area when none are named.
    [[nodiscard]] std::expected<TreeResetResult, Error> reset(
        std::span<const std::string_view> paths, std::string_view target = {}) const {
        const std::string at{target};
        detail::ViewSpan list{paths};

        auto answer = take<raw::write_tree_reset>(
            raw::write_tree_reset(root_.c_str(), at.c_str(), list.span()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteTreeReset> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, at, "the reset answered nothing"}};
        }

        TreeResetResult out;
        out.whole = raw::write_tree_reset_whole(handle.get());
        const auto many = raw::write_tree_reset_path_count(handle.get());
        out.paths.reserve(many);
        for (std::uintptr_t index = 0; index < many; ++index) {
            out.paths.push_back(
                detail::take(raw::write_tree_reset_path_at(handle.get(), index)));
        }
        return out;
    }

    // Stages work into a tree.
    [[nodiscard]] std::expected<TreeAddResult, Error> add(
        std::span<const std::string_view> paths, std::string_view target = {},
        bool include_ignored = false) const {
        const std::string at{target};
        detail::ViewSpan list{paths};

        auto answer = take<raw::write_tree_add>(
            raw::write_tree_add(root_.c_str(), at.c_str(), list.span(), include_ignored));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteTreeAdd> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, at, "the staging answered nothing"}};
        }

        TreeAddResult out;
        const auto many = raw::write_tree_add_change_count(handle.get());
        out.changes.reserve(many);
        for (std::uintptr_t index = 0; index < many; ++index) {
            Owned<RolaWriteTreeAddChange> one{
                raw::write_tree_add_change_at(handle.get(), index)};
            if (!one) {
                continue;
            }
            out.changes.push_back(TreeAddChange{
                .path = detail::take(raw::write_tree_add_change_path(one.get())),
                .kind = detail::take(raw::write_tree_add_change_kind(one.get())),
            });
        }
        return out;
    }

    // Stages the removal of paths from a tree.
    [[nodiscard]] std::expected<TreeRemoveResult, Error> remove(
        std::span<const std::string_view> paths, std::string_view target = {},
        bool cached = false, bool recursive = false, bool force = false) const {
        const std::string at{target};
        detail::ViewSpan list{paths};

        auto answer = take<raw::write_tree_remove_paths>(
            raw::write_tree_remove_paths(root_.c_str(), at.c_str(), list.span(), cached,
                                         recursive, force));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteTreeRemove> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, at, "the removal answered nothing"}};
        }

        TreeRemoveResult out;
        out.cached = raw::write_tree_remove_cached(handle.get());
        const auto many = raw::write_tree_remove_path_count(handle.get());
        out.paths.reserve(many);
        for (std::uintptr_t index = 0; index < many; ++index) {
            out.paths.push_back(
                detail::take(raw::write_tree_remove_path_at(handle.get(), index)));
        }
        return out;
    }

    [[nodiscard]] const std::filesystem::path &root() const noexcept { return root_path_; }

private:
    std::filesystem::path root_path_;
    std::string root_;
};

} // namespace rola
