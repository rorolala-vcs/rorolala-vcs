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

namespace rola {

// One path that differs between two versions of a tree.
struct TreeChange {
    std::string path;
    std::string kind;
    std::string change;
    std::optional<std::string> old_path;
};

// One path a tree holds and its staging area does not.
struct TreeUntracked {
    std::string path;
    std::string kind;
};

// One path a merge left in conflict in a tree.
struct TreeConflict {
    std::string path;
    std::string theirs;
};

// A tree's mark: the Layout entry id it keeps and the description staged for
// its next version. The id is empty when the file is there and cannot be read.
struct TreeMark {
    std::string id;
    bool described = false;
    std::string description;
};

// How a tree stands: what it staged, what the work changed, and what is in
// conflict. Every list is read out whole, so the status has no handle behind it.
namespace detail {
struct TreeAccess;
}

class TreeStatus {
public:
    [[nodiscard]] std::span<const TreeChange> staged() const noexcept { return staged_; }
    [[nodiscard]] std::span<const TreeChange> unstaged() const noexcept { return unstaged_; }
    [[nodiscard]] std::span<const TreeUntracked> untracked() const noexcept { return untracked_; }
    [[nodiscard]] std::span<const TreeConflict> conflicts() const noexcept { return conflicts_; }

    // Whether the tree's own Layout entry is a change that must not be recorded.
    [[nodiscard]] bool wrong() const noexcept { return wrong_; }

    // Whether the tree has work a version would record.
    [[nodiscard]] bool has_work() const noexcept {
        return !staged_.empty() || !unstaged_.empty() || !untracked_.empty()
               || !conflicts_.empty();
    }

    // Whether the tree has nothing staged, changed or in conflict.
    [[nodiscard]] bool clean() const noexcept { return !has_work() && !wrong_; }

private:
    friend struct detail::TreeAccess;

    std::vector<TreeChange> staged_;
    std::vector<TreeChange> unstaged_;
    std::vector<TreeUntracked> untracked_;
    std::vector<TreeConflict> conflicts_;
    bool wrong_ = false;
};

// A tree: the directory a tree version is kept under, and the work inside it.
class TreeScope {
public:
    explicit TreeScope(std::filesystem::path root)
        : root_(std::filesystem::absolute(std::move(root)).lexically_normal()) {}

    [[nodiscard]] const std::filesystem::path &root() const noexcept { return root_; }

    // Whether a directory is the root of a tree.
    [[nodiscard]] static bool is_tree(const std::filesystem::path &directory);

    // The mark of the tree rooted at a directory, read without reading the
    // tree's work. Nothing when the directory is not the root of a tree.
    [[nodiscard]] static std::optional<TreeMark> mark_of(const std::filesystem::path &directory);

private:
    std::filesystem::path root_;
};

class TreeReading;

// How a tree stands and what its mark says, read out together.
class TreeReading {
public:
    TreeReading(const TreeReading &) = default;
    TreeReading &operator=(const TreeReading &) = default;
    TreeReading(TreeReading &&) noexcept = default;
    TreeReading &operator=(TreeReading &&) noexcept = default;
    ~TreeReading() = default;

    [[nodiscard]] const std::filesystem::path &root() const noexcept { return root_; }

    [[nodiscard]] const TreeStatus &status() const noexcept { return status_; }

    // Nothing when the directory keeps no mark.
    [[nodiscard]] const std::optional<TreeMark> &mark() const noexcept { return mark_; }

private:
    friend struct detail::TreeAccess;

    TreeReading(std::filesystem::path root, TreeStatus status, std::optional<TreeMark> mark)
        : root_(std::move(root)), status_(std::move(status)), mark_(std::move(mark)) {}

    std::filesystem::path root_;
    TreeStatus status_;
    std::optional<TreeMark> mark_;
};

namespace detail {

inline std::vector<TreeChange> tree_changes(const RolaTreeStatus *handle, bool staged) {
    const auto many = staged ? raw::tree_status_staged_count(handle)
                             : raw::tree_status_unstaged_count(handle);

    std::vector<TreeChange> out;
    out.reserve(many);
    for (std::uintptr_t index = 0; index < many; ++index) {
        Owned<RolaTreeChange> one{staged ? raw::tree_status_staged_at(handle, index)
                                         : raw::tree_status_unstaged_at(handle, index)};
        if (!one) {
            continue;
        }
        out.push_back(TreeChange{
            .path = take(raw::tree_change_path(one.get())),
            .kind = take(raw::tree_change_kind(one.get())),
            .change = take(raw::tree_change_change(one.get())),
            .old_path = presence(take(raw::tree_change_old_path(one.get()))),
        });
    }
    return out;
}

inline TreeMark tree_mark_of(const RolaTreeMark *handle) {
    return TreeMark{
        .id = take(raw::tree_mark_id(handle)),
        .described = raw::tree_mark_described(handle),
        .description = take(raw::tree_mark_description(handle)),
    };
}

struct TreeAccess {
    static TreeReading make(std::filesystem::path root, Owned<RolaTreeStatus> status,
                            std::optional<TreeMark> mark) {
        TreeStatus read;
        if (status) {
            read.staged_ = tree_changes(status.get(), true);
            read.unstaged_ = tree_changes(status.get(), false);

            const auto untracked = raw::tree_status_untracked_count(status.get());
            read.untracked_.reserve(untracked);
            for (std::uintptr_t index = 0; index < untracked; ++index) {
                Owned<RolaTreeUntracked> one{raw::tree_status_untracked_at(status.get(), index)};
                if (!one) {
                    continue;
                }
                read.untracked_.push_back(TreeUntracked{
                    .path = take(raw::tree_untracked_path(one.get())),
                    .kind = take(raw::tree_untracked_kind(one.get())),
                });
            }

            const auto conflicts = raw::tree_status_conflict_count(status.get());
            read.conflicts_.reserve(conflicts);
            for (std::uintptr_t index = 0; index < conflicts; ++index) {
                Owned<RolaTreeConflict> one{raw::tree_status_conflict_at(status.get(), index)};
                if (!one) {
                    continue;
                }
                read.conflicts_.push_back(TreeConflict{
                    .path = take(raw::tree_conflict_path(one.get())),
                    .theirs = take(raw::tree_conflict_theirs(one.get())),
                });
            }

            read.wrong_ = raw::tree_status_wrong(status.get());
        }

        return TreeReading{std::move(root), std::move(read), std::move(mark)};
    }
};

} // namespace detail

inline bool TreeScope::is_tree(const std::filesystem::path &directory) {
    const auto path = detail::utf8(directory);
    return raw::tree_is_tree(path.c_str());
}

inline std::optional<TreeMark> TreeScope::mark_of(const std::filesystem::path &directory) {
    const auto path = detail::utf8(directory);
    Owned<RolaTreeMark> handle{raw::tree_mark(path.c_str())};
    if (!handle) {
        return std::nullopt;
    }
    return detail::tree_mark_of(handle.get());
}

// Read how a tree stands and what its mark says. A directory that is not there,
// or a tree that does not read back, is the failure.
inline std::expected<TreeReading, Error> read(const TreeScope &scope) {
    const auto path = detail::utf8(scope.root());

    Owned<RolaTreeStatus> status{raw::tree_status(path.c_str())};
    if (!status) {
        return std::unexpected{
            Error{ErrorKind::unavailable, path, "no tree status could be read"}};
    }

    return detail::TreeAccess::make(scope.root(), std::move(status), TreeScope::mark_of(scope.root()));
}

} // namespace rola
