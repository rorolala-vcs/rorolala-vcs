#pragma once

#include <cstdint>
#include <string>
#include <string_view>
#include <system_error>
#include <type_traits>
#include <utility>

namespace rola {

// The one kind a caller branches on. It is not the native family's tag: a Layout
// read and a write both report a missing Workspace, and a caller that decides
// whether it can carry on cares about that, not about who spoke.
//
// The generated `raw::ErrorKind` keeps the per-family members, which is where a
// failure's `raw_tag` comes from.
enum class ErrorKind {
    none,
    no_workspace,
    no_vault,
    no_layout,
    no_store,
    no_index,
    no_account,
    no_track,
    not_cached,
    offline,
    locked,
    invalid_path,
    invalid_argument,
    not_found,
    exists,
    malformed,
    precondition,
    ownership,
    read_only,
    unavailable,
    unreadable,
    cancelled,
    action,
    layout,
    storage,
    index,
    tree,
    not_a_tree,
    book,
    authentication,
    io,
    internal,
};

namespace detail {

// The short sentence a kind stands for when the native side gave no reason. The
// words belong to whoever shows them; this is only what `message` falls back to.
inline std::string_view text_of(ErrorKind kind) noexcept {
    switch (kind) {
    case ErrorKind::none:
        return {};
    case ErrorKind::no_workspace:
        return "no Workspace holds the directory";
    case ErrorKind::no_vault:
        return "no Vault holds the directory";
    case ErrorKind::no_layout:
        return "no Layout is there";
    case ErrorKind::no_store:
        return "no store holds the directory";
    case ErrorKind::no_index:
        return "no index holds the directory";
    case ErrorKind::no_account:
        return "no account is here";
    case ErrorKind::no_track:
        return "the Layout tracks no Vault";
    case ErrorKind::not_cached:
        return "the copy was never fetched here";
    case ErrorKind::offline:
        return "the run was told to stay offline";
    case ErrorKind::locked:
        return "another run holds the lock";
    case ErrorKind::invalid_path:
        return "the path is not one the run accepts";
    case ErrorKind::invalid_argument:
        return "an argument is not one the run accepts";
    case ErrorKind::not_found:
        return "the thing named is not there";
    case ErrorKind::exists:
        return "the destination is already there";
    case ErrorKind::malformed:
        return "what is there does not read as this build knows";
    case ErrorKind::precondition:
        return "the run would break a rule it cannot break";
    case ErrorKind::ownership:
        return "ownership refuses the change";
    case ErrorKind::read_only:
        return "the place cannot be written";
    case ErrorKind::unavailable:
        return "what was asked for is not available";
    case ErrorKind::unreadable:
        return "what was asked for could not be read";
    case ErrorKind::cancelled:
        return "the caller asked the run to stop";
    case ErrorKind::action:
        return "the exchange with the Vault failed";
    case ErrorKind::layout:
        return "a Layout could not be read or written";
    case ErrorKind::storage:
        return "a store could not be read or written";
    case ErrorKind::index:
        return "an index could not be read or written";
    case ErrorKind::tree:
        return "a tree could not be read or written";
    case ErrorKind::not_a_tree:
        return "no tree holds the path";
    case ErrorKind::book:
        return "the book could not be read or written";
    case ErrorKind::authentication:
        return "a key or a session did not hold";
    case ErrorKind::io:
        return "a file or a stream failed";
    case ErrorKind::internal:
        return "the run stopped for a reason this build does not name";
    }

    return "the run failed";
}

class ErrorCategoryImpl final : public std::error_category {
public:
    [[nodiscard]] const char *name() const noexcept override { return "rola"; }

    [[nodiscard]] std::string message(int value) const override {
        return std::string{text_of(static_cast<ErrorKind>(value))};
    }
};

} // namespace detail

// A failure as this layer reports it: one kind to branch on, the subject it is
// about and the reason the native side gave.
//
// `raw_tag` is kept for diagnosis only. One `ErrorKind::invalid_path` may come
// from a Layout family or a write family, and `raw_tag` says which member of
// which family spoke. It is not for branching.
class Error {
public:
    Error() noexcept = default;

    Error(ErrorKind kind, std::string subject = {}, std::string message = {},
          std::uint32_t raw_tag = 0)
        : kind_(kind), subject_(std::move(subject)), message_(std::move(message)),
          raw_tag_(raw_tag) {
        if (kind_ != ErrorKind::none && message_.empty()) {
            message_ = std::string{detail::text_of(kind_)};
        }
    }

    [[nodiscard]] ErrorKind kind() const noexcept { return kind_; }

    // The path, name or hash the failure is about, or an empty view when it is
    // about nothing a caller supplied.
    [[nodiscard]] std::string_view subject() const noexcept { return subject_; }

    // The native reason when there is one, and a short sentence from this layer
    // when there is not. The words belong to whoever shows them.
    [[nodiscard]] std::string_view message() const noexcept { return message_; }

    // The native family member's number, for diagnosis. An error this layer made
    // itself carries zero.
    [[nodiscard]] std::uint32_t raw_tag() const noexcept { return raw_tag_; }

    [[nodiscard]] std::error_code code() const noexcept;

    [[nodiscard]] explicit operator bool() const noexcept { return kind_ != ErrorKind::none; }

private:
    ErrorKind kind_ = ErrorKind::none;
    std::string subject_;
    std::string message_;
    std::uint32_t raw_tag_ = 0;
};

// The category that lets a caller hand a kind to code written against
// `std::error_code`. It carries the kind alone; `Error` carries the subject and
// the native reason, so the two are not interchangeable.
[[nodiscard]] inline const std::error_category &category() noexcept {
    static const detail::ErrorCategoryImpl one;
    return one;
}

[[nodiscard]] inline std::error_code make_error_code(ErrorKind kind) noexcept {
    return {static_cast<int>(kind), category()};
}

inline std::error_code Error::code() const noexcept { return make_error_code(kind_); }

} // namespace rola

template <> struct std::is_error_code_enum<rola::ErrorKind> : std::true_type {};
