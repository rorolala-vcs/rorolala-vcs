#pragma once

#include <cstddef>
#include <cstdint>
#include <expected>
#include <filesystem>
#include <functional>
#include <iterator>
#include <memory>
#include <optional>
#include <span>
#include <stop_token>
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
#include "vault.hpp"
#include "workspace.hpp"
#include "write_request.hpp"
#include "write_result.hpp"

namespace rola {

class Progress;
class Cancellation;

namespace detail {

// The two readings keep their native handle private, and only a lock has to see
// it: every other write names the root and lets the native side locate again.
struct WriterAccess {
    [[nodiscard]] static RolaWorkspace *workspace(const WorkspaceReading &reading) noexcept {
        return reading.workspace_.get();
    }

    [[nodiscard]] static RolaVault *vault(const VaultReading &reading) noexcept {
        return reading.vault_.get();
    }
};

// The handles a request call runs with. The native side refuses a null handle
// rather than reading it as "nothing to say", so a caller that names neither is
// handed a real pair this object owns for the call.
class WriteHandles {
public:
    WriteHandles(WriteHandles &&) noexcept = default;
    WriteHandles &operator=(WriteHandles &&) noexcept = default;
    WriteHandles(const WriteHandles &) = delete;
    WriteHandles &operator=(const WriteHandles &) = delete;
    ~WriteHandles() = default;

    [[nodiscard]] static std::optional<WriteHandles> make(rola::Progress *watch,
                                                          rola::Cancellation *stop);

    const RolaProgress *progress = nullptr;
    const RolaCancellation *cancellation = nullptr;

private:
    friend class rola::Progress;
    friend class rola::Cancellation;

    WriteHandles() = default;

    Owned<RolaProgress> owned_progress_;
    Owned<RolaCancellation> owned_cancellation_;
};

// A C string span that points into the caller's strings. The strings are not
// copied, so they have to outlive the native call this span is handed to.
class StringSpan {
public:
    explicit StringSpan(const std::vector<std::string> &words) {
        items_.reserve(words.size());
        for (const auto &word : words) {
            items_.push_back(word.c_str());
        }
    }

    [[nodiscard]] RolaStrSpan span() const noexcept {
        return RolaStrSpan{.items = items_.empty() ? nullptr : items_.data(),
                           .count = items_.size()};
    }

private:
    std::vector<const char *> items_;
};

// A C string span built from borrowed views. The native side reads C strings,
// so each view is copied once here and the copies live for the call.
class ViewSpan {
public:
    explicit ViewSpan(std::span<const std::string_view> words) {
        owned_.reserve(words.size());
        for (const auto view : words) {
            owned_.emplace_back(view);
        }
        items_.reserve(owned_.size());
        for (const auto &word : owned_) {
            items_.push_back(word.c_str());
        }
    }

    [[nodiscard]] RolaStrSpan span() const noexcept {
        return RolaStrSpan{.items = items_.empty() ? nullptr : items_.data(),
                           .count = items_.size()};
    }

private:
    std::vector<std::string> owned_;
    std::vector<const char *> items_;
};

// The lock guards the ABI hands out are released through their own functions
// rather than `free_`, so the generated release policies do not cover them.
struct WorkspaceLockRelease {
    void operator()(RolaWorkspaceLocking *guard) const noexcept {
        if (guard != nullptr) {
            raw::workspace_lock_release(guard);
        }
    }
};

struct VaultLockRelease {
    void operator()(RolaVaultLocking *guard) const noexcept {
        if (guard != nullptr) {
            raw::vault_lock_release(guard);
        }
    }
};

using WorkspaceLock = std::unique_ptr<RolaWorkspaceLocking, WorkspaceLockRelease>;
using VaultLock = std::unique_ptr<RolaVaultLocking, VaultLockRelease>;

// The one kind a write failure is reported as. The families stay distinct so a
// caller can still branch on `no_account` or `locked`, but the word a caller
// shows comes from the payload, not from the tag.
Error write_error(const RolaWriteError *failure);

// One side of a collecting run, or nothing when the run did not sweep it.
inline std::optional<CollectSide> read_collect_side(const RolaWriteCollectSide *handle) {
    if (handle == nullptr) {
        return std::nullopt;
    }
    return CollectSide{
        .removed = raw::write_collect_side_removed(handle),
        .bytes = raw::write_collect_side_bytes(handle),
        .kept = raw::write_collect_side_kept(handle),
        .changed = raw::write_collect_side_changed(handle),
    };
}

// The failure a write reports when the native side refuses to make the
// progress or cancellation handle a request call needs.
inline Error control_error() {
    return Error{ErrorKind::internal, {},
                 "the progress or cancellation handle could not be made"};
}

} // namespace detail

// A cancellation a caller sets to stop a write at its next step. It is
// move-only because it owns the native flag.
class Cancellation {
public:
    Cancellation(Cancellation &&) noexcept = default;
    Cancellation &operator=(Cancellation &&) noexcept;
    Cancellation(const Cancellation &) = delete;
    Cancellation &operator=(const Cancellation &) = delete;
    ~Cancellation() = default;

    // A flag nobody has set. A native failure leaves an invalid object, which
    // `valid` reports; a write handed one makes its own.
    [[nodiscard]] static Cancellation make();

    // A flag that follows a `std::stop_token`: requesting a stop on the token
    // sets the native flag. The callback is dropped with this object, so the
    // token and the cancellation may die in either order.
    [[nodiscard]] static Cancellation make(std::stop_token token);

    void cancel() noexcept;

    [[nodiscard]] bool cancelled() const noexcept;

    [[nodiscard]] bool valid() const noexcept { return static_cast<bool>(handle_); }

private:
    friend class detail::WriteHandles;

    explicit Cancellation(Owned<RolaCancellation> handle) : handle_(std::move(handle)) {}

    // The callback reads the pointer `handle_` owns, so it is declared after it
    // and destroyed before it.
    Owned<RolaCancellation> handle_;
    std::unique_ptr<std::stop_callback<std::function<void()>>> callback_;
};

// How a write's event reads: a task has begun, has moved, or is over.
enum class ProgressKind { begin, advance, finish };

// Which way a task's work moves. `none` is a task that moves no data.
enum class ProgressDirection { none, up, down };

// One thing a write has said about itself, as this layer reads it. A field the
// native event does not carry stays at its default; `parent` and `total` are
// absent when the native side did not say them, which is not the same as zero.
struct ProgressEvent {
    ProgressKind kind = ProgressKind::advance;
    std::uint64_t id = 0;
    std::optional<std::uint64_t> parent;
    std::string what;
    ProgressDirection direction = ProgressDirection::none;
    std::optional<std::uint64_t> total;
    std::uint64_t done = 0;
};

class ProgressRange;

// What a write says about itself, collected until a caller asks. It is
// move-only because it owns the native handle.
//
// A write blocks while it runs and only says anything while it does, so the
// caller drains this from another thread; this type starts no thread of its own.
class Progress {
public:
    Progress(Progress &&) noexcept = default;
    Progress &operator=(Progress &&) noexcept = default;
    Progress(const Progress &) = delete;
    Progress &operator=(const Progress &) = delete;
    ~Progress() = default;

    [[nodiscard]] static Progress make();

    // The next event waiting, or nothing when there is none right now. Nothing
    // here means the write has not spoken since the last drain, not that it is
    // over: the write's own call says when it ended.
    [[nodiscard]] std::optional<ProgressEvent> next();

    // Hands every waiting event to `sink` and answers how many were there.
    std::size_t drain(const std::function<void(const ProgressEvent &)> &sink);

    // The waiting events as a range, drained until the first empty answer. It
    // is single pass and does not block.
    [[nodiscard]] ProgressRange events();

    [[nodiscard]] bool valid() const noexcept { return static_cast<bool>(handle_); }

private:
    friend class detail::WriteHandles;

    explicit Progress(Owned<RolaProgress> handle) : handle_(std::move(handle)) {}

    Owned<RolaProgress> handle_;
};

// The range `Progress::events` hands out. A lazy single-pass input range, so a
// caller that draws as it goes does not hold every event at once.
class ProgressRange {
public:
    class Iterator {
    public:
        using value_type = ProgressEvent;
        using difference_type = std::ptrdiff_t;
        using iterator_category = std::input_iterator_tag;
        using pointer = const ProgressEvent *;
        using reference = const ProgressEvent &;

        Iterator() noexcept = default;
        explicit Iterator(Progress *owner) : owner_(owner) { advance(); }

        [[nodiscard]] reference operator*() const noexcept { return *current_; }
        [[nodiscard]] pointer operator->() const noexcept { return &*current_; }

        Iterator &operator++() {
            advance();
            return *this;
        }

        void operator++(int) { advance(); }

        [[nodiscard]] bool operator==(const Iterator &other) const noexcept {
            return owner_ == other.owner_;
        }

    private:
        void advance() {
            if (owner_ == nullptr) {
                return;
            }
            current_ = owner_->next();
            if (!current_) {
                owner_ = nullptr;
            }
        }

        Progress *owner_ = nullptr;
        std::optional<ProgressEvent> current_;
    };

    explicit ProgressRange(Progress *owner) noexcept : owner_(owner) {}

    [[nodiscard]] Iterator begin() { return Iterator{owner_}; }
    [[nodiscard]] static Iterator end() noexcept { return Iterator{}; }

private:
    Progress *owner_;
};

// A lock a writer holds for as long as the guard lives. It is the only object
// here with a destructor that does work, and it moves because the native guard
// does.
class LockGuard {
public:
    LockGuard(LockGuard &&) noexcept = default;
    LockGuard &operator=(LockGuard &&) noexcept = default;
    LockGuard(const LockGuard &) = delete;
    LockGuard &operator=(const LockGuard &) = delete;
    ~LockGuard() = default;

private:
    friend class WorkspaceWriter;
    friend class VaultWriter;

    explicit LockGuard(detail::WorkspaceLock guard) : guard_(std::move(guard)) {}
    explicit LockGuard(detail::VaultLock guard) : guard_(std::move(guard)) {}

    std::variant<detail::WorkspaceLock, detail::VaultLock> guard_;
};

inline Cancellation Cancellation::make() {
    return Cancellation{Owned<RolaCancellation>{raw::cancellation_new()}};
}

inline Cancellation Cancellation::make(std::stop_token token) {
    Cancellation out{Owned<RolaCancellation>{raw::cancellation_new()}};
    if (out.handle_) {
        auto *native = out.handle_.get();
        out.callback_ = std::make_unique<std::stop_callback<std::function<void()>>>(
            token, [native] { raw::cancellation_cancel(native); });
    }
    return out;
}

// The old callback is dropped before the old native flag, so a stop requested
// during the move never reaches a freed flag.
inline Cancellation &Cancellation::operator=(Cancellation &&other) noexcept {
    if (this != &other) {
        callback_.reset();
        handle_ = std::move(other.handle_);
        callback_ = std::move(other.callback_);
    }
    return *this;
}

inline void Cancellation::cancel() noexcept {
    if (handle_) {
        raw::cancellation_cancel(handle_.get());
    }
}

inline bool Cancellation::cancelled() const noexcept {
    return handle_ ? raw::cancellation_is_cancelled(handle_.get()) : false;
}

inline Progress Progress::make() {
    return Progress{Owned<RolaProgress>{raw::progress_new()}};
}

inline std::optional<ProgressEvent> Progress::next() {
    if (!handle_) {
        return std::nullopt;
    }

    Owned<RolaProgressEvent> said{raw::progress_next(handle_.get())};
    if (!said) {
        return std::nullopt;
    }

    const auto signal = raw::progress_event_signal(said.get());
    ProgressEvent event;

    switch (signal.tag) {
    case RolaProgressSignal_Begin: {
        const auto &begin = signal.payload.Begin;
        event.kind = ProgressKind::begin;
        event.id = begin.id;
        if (begin.parent_present) {
            event.parent = begin.parent;
        }
        event.what = detail::take(begin.what);
        const auto direction = detail::take(begin.direction);
        if (direction == "up") {
            event.direction = ProgressDirection::up;
        } else if (direction == "down") {
            event.direction = ProgressDirection::down;
        }
        if (begin.total_present) {
            event.total = begin.total;
        }
        return event;
    }
    case RolaProgressSignal_Advance:
        event.kind = ProgressKind::advance;
        event.id = signal.payload.Advance.id;
        event.done = signal.payload.Advance.done;
        return event;
    case RolaProgressSignal_Finish:
        event.kind = ProgressKind::finish;
        event.id = signal.payload.Finish.id;
        return event;
    }

    return std::nullopt;
}

inline std::size_t Progress::drain(const std::function<void(const ProgressEvent &)> &sink) {
    std::size_t many = 0;
    while (auto event = next()) {
        ++many;
        if (sink) {
            sink(*event);
        }
    }
    return many;
}

inline ProgressRange Progress::events() { return ProgressRange{this}; }

inline std::optional<detail::WriteHandles> detail::WriteHandles::make(rola::Progress *watch,
                                                                      rola::Cancellation *stop) {
    WriteHandles handles;

    const RolaProgress *watched = watch != nullptr ? watch->handle_.get() : nullptr;
    if (watched == nullptr) {
        handles.owned_progress_ = Owned<RolaProgress>{raw::progress_new()};
        watched = handles.owned_progress_.get();
    }

    const RolaCancellation *stopped = stop != nullptr ? stop->handle_.get() : nullptr;
    if (stopped == nullptr) {
        handles.owned_cancellation_ = Owned<RolaCancellation>{raw::cancellation_new()};
        stopped = handles.owned_cancellation_.get();
    }

    if (watched == nullptr || stopped == nullptr) {
        return std::nullopt;
    }

    handles.progress = watched;
    handles.cancellation = stopped;
    return handles;
}

namespace detail {

// One payload string that belongs to the failure, copied before the failure is
// given back. The mapping itself lives in `error_map.hpp`, so every family
// reaches a caller through the same `rola::Error`.
inline Error write_error(const RolaWriteError *failure) { return from(failure); }

// A tracked file's strings, and the same for the other small answers, are read
// out while the handle is alive, so every result a caller holds has no handle
// behind it.

// The native target a request's target names. The payload holds a pointer into
// the request's string, which outlives the call this is handed to.
inline RolaWriteTarget native_target(const WriteTarget &target) {
    RolaWriteTarget out{};
    switch (target.kind) {
    case WriteTargetKind::named:
        out.tag = RolaWriteTarget_Named;
        out.payload.Named.name = const_cast<char *>(target.name.c_str());
        break;
    case WriteTargetKind::address:
        out.tag = RolaWriteTarget_Address;
        out.payload.Address.address = const_cast<char *>(target.address.c_str());
        break;
    case WriteTargetKind::tracked:
        out.tag = RolaWriteTarget_Tracked;
        break;
    }
    return out;
}

// The native labels a request's labels name. An empty field is passed as an
// empty string, which the native side reads as keeping its own word.
inline RolaWriteLabels native_labels(const WriteLabels &labels) {
    return RolaWriteLabels{
        .reading = labels.reading.c_str(),
        .writing = labels.writing.c_str(),
        .fetching = labels.fetching.c_str(),
        .uploading = labels.uploading.c_str(),
        .downloading = labels.downloading.c_str(),
        .checking = labels.checking.c_str(),
        .applying = labels.applying.c_str(),
    };
}

// The native fields every request carries. `workspace` is the writer's root,
// and every other pointer is into the request's strings; both outlive the call.
inline RolaWriteCommon native_common(const std::string &workspace, const WriteCommon &common) {
    return RolaWriteCommon{
        .workspace = workspace.c_str(),
        .account = common.account.c_str(),
        .target = native_target(common.target),
        .offline = common.offline,
        .force = common.force,
        .concurrency = common.concurrency,
        .labels = native_labels(common.labels),
    };
}

inline RolaWriteAlignMode native_align_mode(const AlignMode &mode) {
    RolaWriteAlignMode out{};
    switch (mode.kind) {
    case AlignModeKind::confirm_delete:
        out.tag = RolaWriteAlignMode_Delete;
        break;
    case AlignModeKind::confirm_rename:
        out.tag = RolaWriteAlignMode_Rename;
        break;
    case AlignModeKind::break_move:
        out.tag = RolaWriteAlignMode_Break;
        break;
    case AlignModeKind::move_to:
        out.tag = RolaWriteAlignMode_MoveTo;
        out.payload.MoveTo.path = const_cast<char *>(mode.move_to.c_str());
        break;
    case AlignModeKind::restore_move:
        out.tag = RolaWriteAlignMode_RestoreMove;
        break;
    case AlignModeKind::restore_modify:
        out.tag = RolaWriteAlignMode_RestoreModify;
        break;
    case AlignModeKind::restore_staged:
        out.tag = RolaWriteAlignMode_RestoreStaged;
        break;
    case AlignModeKind::restore_delete:
        out.tag = RolaWriteAlignMode_RestoreDelete;
        break;
    case AlignModeKind::restore:
        out.tag = RolaWriteAlignMode_Restore;
        break;
    }
    return out;
}

inline RolaWriteDirection native_direction(SyncDirection direction) noexcept {
    switch (direction) {
    case SyncDirection::up:
        return RolaWriteDirection_Up;
    case SyncDirection::down:
        return RolaWriteDirection_Down;
    case SyncDirection::both:
        return RolaWriteDirection_Both;
    }
    return RolaWriteDirection_Both;
}

inline RolaWriteOwnershipKind native_ownership(OwnershipKind kind) noexcept {
    return kind == OwnershipKind::giveup ? RolaWriteOwnershipKind_Giveup
                                          : RolaWriteOwnershipKind_Hold;
}

inline RolaWriteLayoutOp native_layout_op(const LayoutOp &op) noexcept {
    RolaWriteLayoutOp out{};
    switch (op.kind) {
    case LayoutOpKind::create_entry:
        out.tag = RolaWriteLayoutOp_CreateEntry;
        out.payload.CreateEntry = RolaWriteLayoutOpCreateEntry{
            .uuid = const_cast<char *>(op.uuid.c_str()),
            .path = const_cast<char *>(op.path.c_str()),
            .version = const_cast<char *>(op.version.c_str()),
            .owner = const_cast<char *>(op.owner.c_str()),
            .description = const_cast<char *>(op.description.c_str()),
            .kind = const_cast<char *>(op.entry_kind.c_str()),
        };
        break;
    case LayoutOpKind::update_entry:
        out.tag = RolaWriteLayoutOp_UpdateEntry;
        out.payload.UpdateEntry = RolaWriteLayoutOpUpdateEntry{
            .uuid = const_cast<char *>(op.uuid.c_str()),
            .version = const_cast<char *>(op.version.c_str()),
            .owner = const_cast<char *>(op.owner.c_str()),
            .description = const_cast<char *>(op.description.c_str()),
        };
        break;
    case LayoutOpKind::remove_entry:
        out.tag = RolaWriteLayoutOp_RemoveEntry;
        out.payload.RemoveEntry = RolaWriteLayoutOpRemoveEntry{
            .uuid = const_cast<char *>(op.uuid.c_str()),
        };
        break;
    case LayoutOpKind::create_path:
        out.tag = RolaWriteLayoutOp_CreatePath;
        out.payload.CreatePath = RolaWriteLayoutOpCreatePath{
            .path = const_cast<char *>(op.path.c_str()),
            .uuid = const_cast<char *>(op.uuid.c_str()),
        };
        break;
    case LayoutOpKind::move_path:
        out.tag = RolaWriteLayoutOp_MovePath;
        out.payload.MovePath = RolaWriteLayoutOpMovePath{
            .from = const_cast<char *>(op.from.c_str()),
            .to = const_cast<char *>(op.to.c_str()),
        };
        break;
    case LayoutOpKind::remove_path:
        out.tag = RolaWriteLayoutOp_RemovePath;
        out.payload.RemovePath = RolaWriteLayoutOpRemovePath{
            .path = const_cast<char *>(op.path.c_str()),
        };
        break;
    case LayoutOpKind::change_ownership:
        out.tag = RolaWriteLayoutOp_ChangeOwnership;
        out.payload.ChangeOwnership = RolaWriteLayoutOpChangeOwnership{
            .uuid = const_cast<char *>(op.uuid.c_str()),
            .owner = const_cast<char *>(op.owner.c_str()),
        };
        break;
    case LayoutOpKind::change_uuid:
        out.tag = RolaWriteLayoutOp_ChangeUuid;
        out.payload.ChangeUuid = RolaWriteLayoutOpChangeUuid{
            .from = const_cast<char *>(op.from.c_str()),
            .to = const_cast<char *>(op.to.c_str()),
        };
        break;
    }
    return out;
}

} // namespace detail

} // namespace rola
