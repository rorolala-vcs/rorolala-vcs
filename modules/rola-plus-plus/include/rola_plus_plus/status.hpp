#pragma once

#include <algorithm>
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

// What one path of the work is, in the one word a reader acts on first.
enum class PathState {
    unknown,
    lost,
    untagged,
    modified,
    wrong,
    renamed,
    renamed_likely,
    merging,
};

// How the version recorded for a path stands beside the one upstream records.
enum class StandingRelation {
    same,
    ahead,
    behind,
    apart,
    unknown,
};

// Why a merge in progress cannot go on.
enum class BrokenKind {
    none,
    no_target,
    no_place,
    file_gone,
    no_index,
    no_variant,
    unreadable,
    changed,
};

// One path that moved.
struct Rename {
    std::string from;
    std::string to;
    bool modified = false;
    bool strong = false;
};

// One variant waiting to be joined into one file.
struct Merge {
    std::string target;
    std::string variant;
    std::string path;
    std::optional<std::string> moved;
};

// A merge that cannot go on, and what the reason is about.
struct BrokenReason {
    BrokenKind kind = BrokenKind::none;
    std::string what;
    std::string target;
};

// How one path stands beside upstream, and how many versions apart the two are
// when the chain says.
struct Standing {
    std::string path;
    StandingRelation relation = StandingRelation::unknown;
    std::optional<std::uint64_t> distance;
};

// A place a status can be read from: the Workspace's root and nothing else.
// It is cheap to copy, so it crosses calls the way a path does.
class StatusScope {
public:
    explicit StatusScope(std::filesystem::path root)
        : root_(std::filesystem::absolute(std::move(root)).lexically_normal()) {}

    [[nodiscard]] const std::filesystem::path &root() const noexcept { return root_; }

private:
    std::filesystem::path root_;
};

class StatusReading;

namespace detail {
struct StatusAccess;
}

// How the working tree stands beside the Layout it works from at one moment.
// Every list is read out whole when the reading is made, so the value a caller
// holds has no handle behind it and no lifetime to respect.
class StatusReading {
public:
    StatusReading(const StatusReading &) = default;
    StatusReading &operator=(const StatusReading &) = default;
    StatusReading(StatusReading &&) noexcept = default;
    StatusReading &operator=(StatusReading &&) noexcept = default;
    ~StatusReading() = default;

    [[nodiscard]] const std::filesystem::path &root() const noexcept { return root_; }

    [[nodiscard]] std::span<const std::string> lost() const noexcept { return lost_; }
    [[nodiscard]] std::span<const std::string> untagged() const noexcept { return untagged_; }
    [[nodiscard]] std::span<const std::string> ignored() const noexcept { return ignored_; }
    [[nodiscard]] std::span<const std::string> modified() const noexcept { return modified_; }
    [[nodiscard]] std::span<const std::string> wrong_base() const noexcept { return wrong_base_; }
    [[nodiscard]] std::span<const std::string> unowned() const noexcept { return unowned_; }

    [[nodiscard]] std::span<const Rename> renamed() const noexcept { return renamed_; }
    [[nodiscard]] std::span<const Merge> merging() const noexcept { return merging_; }
    [[nodiscard]] std::span<const BrokenReason> broken() const noexcept { return broken_; }
    [[nodiscard]] std::span<const Standing> standings() const noexcept { return standings_; }

    // Nothing when the reading is not about the path. Otherwise the state a
    // reader acts on first, in the order the native side answers it.
    [[nodiscard]] std::optional<PathState> state_of(std::string_view path) const {
        const auto at = detail::named(root_, path);
        if (!at) {
            return std::nullopt;
        }

        const auto holds = [&at](const std::vector<std::string> &list) {
            return std::find(list.begin(), list.end(), *at) != list.end();
        };

        if (holds(unowned_) || holds(wrong_base_)) {
            return PathState::wrong;
        }
        for (const auto &one : merging_) {
            if (one.target == *at) {
                return PathState::merging;
            }
        }
        if (holds(lost_)) {
            return PathState::lost;
        }
        for (const auto &one : renamed_) {
            if (one.from == *at) {
                return PathState::lost;
            }
        }
        if (holds(untagged_)) {
            return PathState::untagged;
        }
        for (const auto &one : renamed_) {
            if (one.to == *at) {
                return one.strong ? PathState::renamed : PathState::renamed_likely;
            }
        }
        if (holds(modified_)) {
            return PathState::modified;
        }

        return std::nullopt;
    }

    // Nothing when the reading holds no standing for the path.
    [[nodiscard]] std::optional<Standing> standing_of(std::string_view path) const {
        const auto at = detail::named(root_, path);
        if (!at) {
            return std::nullopt;
        }
        for (const auto &one : standings_) {
            if (one.path == *at) {
                return one;
            }
        }
        return std::nullopt;
    }

    // Nothing when the chain does not say how far apart the two versions are,
    // which is also what a path the reading holds no standing for answers.
    [[nodiscard]] std::optional<std::int64_t> distance_of(std::string_view path) const {
        const auto at = detail::named(root_, path);
        if (!at) {
            return std::nullopt;
        }
        for (const auto &one : standings_) {
            if (one.path == *at) {
                return one.distance ? std::optional<std::int64_t>{
                                          static_cast<std::int64_t>(*one.distance)}
                                    : std::nullopt;
            }
        }
        return std::nullopt;
    }

    // Whether the work stands as it was recorded, with nothing to confirm or
    // record. The ignored paths are not work, so they do not count.
    [[nodiscard]] bool clean() const noexcept {
        return lost_.empty() && untagged_.empty() && modified_.empty()
               && unowned_.empty() && renamed_.empty() && merging_.empty() && broken_.empty();
    }

private:
    friend struct detail::StatusAccess;

    explicit StatusReading(Owned<RolaStatus> status);

    std::filesystem::path root_;
    std::vector<std::string> lost_;
    std::vector<std::string> untagged_;
    std::vector<std::string> ignored_;
    std::vector<std::string> modified_;
    std::vector<std::string> wrong_base_;
    std::vector<std::string> unowned_;
    std::vector<Rename> renamed_;
    std::vector<Merge> merging_;
    std::vector<BrokenReason> broken_;
    std::vector<Standing> standings_;
};

namespace detail {

inline PathState from_native(RolaPathState state) noexcept {
    switch (state) {
    case RolaPathState_Lost:
        return PathState::lost;
    case RolaPathState_Untagged:
        return PathState::untagged;
    case RolaPathState_Modified:
        return PathState::modified;
    case RolaPathState_Wrong:
        return PathState::wrong;
    case RolaPathState_Renamed:
        return PathState::renamed;
    case RolaPathState_RenamedLikely:
        return PathState::renamed_likely;
    case RolaPathState_Merging:
        return PathState::merging;
    case RolaPathState_Unknown:
        return PathState::unknown;
    }

    return PathState::unknown;
}

inline StandingRelation from_native(RolaStanding relation) noexcept {
    switch (relation) {
    case RolaStanding_Same:
        return StandingRelation::same;
    case RolaStanding_Ahead:
        return StandingRelation::ahead;
    case RolaStanding_Behind:
        return StandingRelation::behind;
    case RolaStanding_Apart:
        return StandingRelation::apart;
    case RolaStanding_Unknown:
        return StandingRelation::unknown;
    }

    return StandingRelation::unknown;
}

inline BrokenKind from_native(RolaStatusBroken kind) noexcept {
    switch (kind) {
    case RolaStatusBroken_NoTarget:
        return BrokenKind::no_target;
    case RolaStatusBroken_NoPlace:
        return BrokenKind::no_place;
    case RolaStatusBroken_FileGone:
        return BrokenKind::file_gone;
    case RolaStatusBroken_NoIndex:
        return BrokenKind::no_index;
    case RolaStatusBroken_NoVariant:
        return BrokenKind::no_variant;
    case RolaStatusBroken_Unreadable:
        return BrokenKind::unreadable;
    case RolaStatusBroken_Changed:
        return BrokenKind::changed;
    case RolaStatusBroken_None:
        return BrokenKind::none;
    }

    return BrokenKind::none;
}

} // namespace detail

inline StatusReading::StatusReading(Owned<RolaStatus> status) {
    root_ = detail::from_utf8(detail::take(raw::status_root(status.get())));
    lost_ = detail::strings(status.get(), raw::status_lost_count, raw::status_lost_at);
    untagged_ =
        detail::strings(status.get(), raw::status_untagged_count, raw::status_untagged_at);
    ignored_ = detail::strings(status.get(), raw::status_ignored_count, raw::status_ignored_at);
    modified_ =
        detail::strings(status.get(), raw::status_modified_count, raw::status_modified_at);
    wrong_base_ =
        detail::strings(status.get(), raw::status_wrong_base_count, raw::status_wrong_base_at);
    unowned_ = detail::strings(status.get(), raw::status_unowned_count, raw::status_unowned_at);

    const auto moves = raw::status_renamed_count(status.get());
    renamed_.reserve(moves);
    for (std::uintptr_t index = 0; index < moves; ++index) {
        renamed_.push_back(Rename{
            .from = detail::take(raw::status_renamed_from(status.get(), index)),
            .to = detail::take(raw::status_renamed_to(status.get(), index)),
            .modified = raw::status_renamed_modified(status.get(), index),
            .strong = raw::status_renamed_strong(status.get(), index),
        });
    }

    const auto merges = raw::status_merging_count(status.get());
    merging_.reserve(merges);
    for (std::uintptr_t index = 0; index < merges; ++index) {
        Merge one{
            .target = detail::take(raw::status_merging_target(status.get(), index)),
            .variant = detail::take(raw::status_merging_variant(status.get(), index)),
            .path = detail::take(raw::status_merging_path(status.get(), index)),
            .moved = std::nullopt,
        };
        auto moved = detail::take(raw::status_merging_moved(status.get(), index));
        if (!moved.empty()) {
            one.moved = std::move(moved);
        }
        merging_.push_back(std::move(one));
    }

    const auto breaks = raw::status_broken_count(status.get());
    broken_.reserve(breaks);
    for (std::uintptr_t index = 0; index < breaks; ++index) {
        broken_.push_back(BrokenReason{
            .kind = detail::from_native(raw::status_broken_kind(status.get(), index)),
            .what = detail::take(raw::status_broken_what(status.get(), index)),
            .target = detail::take(raw::status_broken_target(status.get(), index)),
        });
    }

    const auto stands = raw::status_standing_count(status.get());
    standings_.reserve(stands);
    for (std::uintptr_t index = 0; index < stands; ++index) {
        auto path = detail::take(raw::status_standing_path(status.get(), index));
        const auto apart = raw::status_standing_distance_of(status.get(), path.c_str());

        Standing one{
            .path = std::move(path),
            .relation = detail::from_native(raw::status_standing_relation(status.get(), index)),
            .distance = std::nullopt,
        };
        if (apart >= 0) {
            one.distance = static_cast<std::uint64_t>(apart);
        }
        standings_.push_back(std::move(one));
    }
}

namespace detail {

struct StatusAccess {
    static StatusReading make(Owned<RolaStatus> status) {
        return StatusReading{std::move(status)};
    }
};

} // namespace detail

// Read how the work under a status scope stands. A scope that is not there, or
// a Workspace that has no status, is the failure.
inline std::expected<StatusReading, Error> read(const StatusScope &scope) {
    const auto path = detail::utf8(scope.root());
    Owned<RolaStatus> status{raw::status_locate(path.c_str())};
    if (!status) {
        return std::unexpected{Error{ErrorKind::unavailable, path,
                                     "the Workspace has no status to read"}};
    }
    return detail::StatusAccess::make(std::move(status));
}

} // namespace rola
