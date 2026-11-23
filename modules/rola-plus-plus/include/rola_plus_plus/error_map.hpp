#pragma once

#include <cstdint>
#include <string>
#include <string_view>

#include <rola_plus_plus/gen/error_kind.hpp>
#include <rola_plus_plus/gen/raw.hpp>
#include <rola_plus_plus/gen/release.hpp>

#include "detail.hpp"
#include "error.hpp"

namespace rola::detail {

// One place where every native error family becomes the one `rola::Error`. Each
// overload keeps the family's payload as `subject` and its reason as `message`,
// and leaves the family member's number in `raw_tag` for diagnosis.
//
// A tag this build does not name becomes `ErrorKind::internal` with the number
// kept, so a newer ABI still runs against an older `rola++`. That fallback is
// also why `internal` does not always mean a bug in the library.

inline Error family_error(ErrorKind kind, std::string subject, std::string message,
                          std::uint32_t tag) {
    return Error{kind, std::move(subject), std::move(message), tag};
}

// A payload that names both what failed and why reads as one sentence, and an
// empty half never leaves a bare separator behind.
inline std::string joined(std::string what, std::string reason) {
    if (what.empty()) {
        return reason;
    }
    if (reason.empty()) {
        return what;
    }
    return what + ": " + reason;
}

// The carriers are opaque handles with their own getters. They are taken here,
// so the caller's family handle stays the only one it has to release.
inline Error from(const RolaIoError *failure) {
    if (failure == nullptr) {
        return Error{ErrorKind::io, {}, "an I/O failure carried no reason"};
    }

    return family_error(ErrorKind::io, {}, take(raw::io_error_message(failure)),
                        static_cast<std::uint32_t>(raw::io_error_kind(failure)));
}

inline Error from(const RolaBincodeError *failure) {
    if (failure == nullptr) {
        return Error{ErrorKind::malformed, {}, "a value could not be encoded or decoded"};
    }

    return Error{ErrorKind::malformed, {}, take(raw::bincode_error_message(failure))};
}

inline Error from(const RolaAddrError *failure) {
    if (failure == nullptr) {
        return Error{ErrorKind::invalid_argument, {}, "a target did not read as an address"};
    }

    return Error{ErrorKind::invalid_argument, take(raw::addr_error_target(failure)),
                 "a target did not read as an address"};
}

inline Error from(const RolaLayoutError *failure) {
    if (failure == nullptr) {
        return Error{ErrorKind::unreadable, {}, "the Layout read failed without a reason"};
    }

    const auto tag = static_cast<std::uint32_t>(failure->tag);
    switch (failure->tag) {
    case RolaLayoutError_NoWorkspace:
        return family_error(ErrorKind::no_workspace, {}, {}, tag);
    case RolaLayoutError_NoLayout:
        return family_error(ErrorKind::no_layout, {}, {}, tag);
    case RolaLayoutError_NotCached:
        return family_error(ErrorKind::not_cached, {}, {}, tag);
    case RolaLayoutError_Missing:
        return family_error(ErrorKind::not_found, {},
                            "the name is not one a place has a Layout under", tag);
    case RolaLayoutError_Read:
        return family_error(ErrorKind::unreadable, {},
                            take(failure->payload.Read.reason), tag);
    }

    return family_error(ErrorKind::unreadable, {}, {}, tag);
}

inline Error from(const RolaStorageError *failure) {
    if (failure == nullptr) {
        return Error{ErrorKind::unreadable, {}, "the store read failed without a reason"};
    }

    const auto tag = static_cast<std::uint32_t>(failure->tag);
    switch (failure->tag) {
    case RolaStorageError_NoStore:
        return family_error(ErrorKind::no_store, {}, {}, tag);
    case RolaStorageError_NotFound:
        return family_error(ErrorKind::not_found, take(failure->payload.NotFound.key),
                            "nothing is stored under the key", tag);
    case RolaStorageError_Exists:
        return family_error(ErrorKind::exists, take(failure->payload.Exists.path),
                            "the destination is already there", tag);
    case RolaStorageError_Malformed:
        return family_error(ErrorKind::malformed, {},
                            "the key does not read as one this build knows", tag);
    case RolaStorageError_NotAstructure:
        return family_error(ErrorKind::malformed, {},
                            "the content is not a tree structure this build reads", tag);
    case RolaStorageError_Read:
        return family_error(ErrorKind::unreadable, {},
                            take(failure->payload.Read.reason), tag);
    }

    return family_error(ErrorKind::unreadable, {}, {}, tag);
}

inline Error from(const RolaWriteError *failure) {
    if (failure == nullptr) {
        return Error{ErrorKind::internal, {}, "the write failed without a reason"};
    }

    const auto tag = static_cast<std::uint32_t>(failure->tag);
    switch (failure->tag) {
    case RolaWriteError_NoWorkspace:
        return family_error(ErrorKind::no_workspace, {}, {}, tag);
    case RolaWriteError_NoLayout:
        return family_error(ErrorKind::no_layout, {}, {}, tag);
    case RolaWriteError_NoStore:
        return family_error(ErrorKind::no_store, {}, {}, tag);
    case RolaWriteError_NoIndex:
        return family_error(ErrorKind::no_index, {}, {}, tag);
    case RolaWriteError_NoAccount:
        return family_error(ErrorKind::no_account, {}, {}, tag);
    case RolaWriteError_NoTrack:
        return family_error(ErrorKind::no_track, {}, {}, tag);
    case RolaWriteError_NotCached:
        return family_error(ErrorKind::not_cached, {}, {}, tag);
    case RolaWriteError_Offline:
        return family_error(ErrorKind::offline, {}, {}, tag);
    case RolaWriteError_NoVault:
        return family_error(ErrorKind::no_vault, {}, {}, tag);
    case RolaWriteError_Locked:
        return family_error(ErrorKind::locked, take(failure->payload.Locked.path), {}, tag);
    case RolaWriteError_InvalidPath:
        return family_error(ErrorKind::invalid_path, take(failure->payload.InvalidPath.path),
                            take(failure->payload.InvalidPath.reason), tag);
    case RolaWriteError_NotFound:
        return family_error(ErrorKind::not_found, {},
                            take(failure->payload.NotFound.what), tag);
    case RolaWriteError_Precondition:
        return family_error(ErrorKind::precondition, {},
                            joined(take(failure->payload.Precondition.what),
                                   take(failure->payload.Precondition.reason)),
                            tag);
    case RolaWriteError_Ownership:
        return family_error(ErrorKind::ownership, take(failure->payload.Ownership.path),
                            joined(take(failure->payload.Ownership.what),
                                   take(failure->payload.Ownership.reason)),
                            tag);
    case RolaWriteError_Cancelled:
        return family_error(ErrorKind::cancelled, {}, {}, tag);
    case RolaWriteError_Action:
        return family_error(ErrorKind::action, {}, take(failure->payload.Action.reason), tag);
    case RolaWriteError_Layout:
        return family_error(ErrorKind::layout, {}, take(failure->payload.Layout.reason), tag);
    case RolaWriteError_Storage:
        return family_error(ErrorKind::storage, {}, take(failure->payload.Storage.reason), tag);
    case RolaWriteError_ReadOnly:
        return family_error(ErrorKind::read_only, take(failure->payload.ReadOnly.path), {}, tag);
    case RolaWriteError_Index:
        return family_error(ErrorKind::index, {}, take(failure->payload.Index.reason), tag);
    case RolaWriteError_Io:
        return family_error(ErrorKind::io, take(failure->payload.Io.path),
                            take(failure->payload.Io.reason), tag);
    case RolaWriteError_Internal:
        return family_error(ErrorKind::internal, {}, take(failure->payload.Internal.reason),
                            tag);
    case RolaWriteError_Tree:
        return family_error(ErrorKind::tree, {}, take(failure->payload.Tree.reason), tag);
    case RolaWriteError_NotAtree:
        return family_error(ErrorKind::not_a_tree, take(failure->payload.NotATree.path), {},
                            tag);
    }

    return family_error(ErrorKind::internal, {}, {}, tag);
}

inline Error from(const RolaAuthError *failure) {
    if (failure == nullptr) {
        return Error{ErrorKind::authentication, {}, "an identity or a session did not hold"};
    }

    const auto tag = static_cast<std::uint32_t>(failure->tag);
    switch (failure->tag) {
    case RolaAuthError_Malformed:
        return family_error(ErrorKind::authentication, {},
                            "a key or a signature was not the shape its algorithm states", tag);
    case RolaAuthError_BadSignature:
        return family_error(ErrorKind::authentication, {},
                            "a signature did not verify under the key it was checked against",
                            tag);
    case RolaAuthError_UnexpectedIdentity:
        return family_error(ErrorKind::authentication, {},
                            "the peer proved an identity other than the one expected", tag);
    case RolaAuthError_Handshake:
        return family_error(ErrorKind::authentication, {},
                            "a handshake did not follow the protocol", tag);
    case RolaAuthError_BadRecord:
        return family_error(ErrorKind::authentication, {},
                            "a record did not decrypt", tag);
    case RolaAuthError_Io: {
        Owned<RolaIoError> io{failure->payload.Io};
        auto inner = from(io.get());
        return family_error(ErrorKind::authentication, std::string{inner.subject()},
                            std::string{inner.message()}, tag);
    }
    }

    return family_error(ErrorKind::authentication, {}, {}, tag);
}

inline Error from(const RolaBookError *failure) {
    if (failure == nullptr) {
        return Error{ErrorKind::book, {}, "the book read failed without a reason"};
    }

    const auto tag = static_cast<std::uint32_t>(failure->tag);
    switch (failure->tag) {
    case RolaBookError_Io: {
        Owned<RolaIoError> io{failure->payload.Io};
        auto inner = from(io.get());
        return family_error(ErrorKind::book, std::string{inner.subject()},
                            std::string{inner.message()}, tag);
    }
    case RolaBookError_Malformed:
        return family_error(ErrorKind::book, {}, take(failure->payload.Malformed.detail), tag);
    case RolaBookError_Unsupported:
        return family_error(ErrorKind::book, {},
                            "the book is in a format version this build does not read", tag);
    case RolaBookError_Duplicate:
        return family_error(ErrorKind::book, take(failure->payload.Duplicate.name),
                            "two articles were given the same name", tag);
    case RolaBookError_Empty:
        return family_error(ErrorKind::book, {}, "a book with no articles was asked to be written",
                            tag);
    case RolaBookError_Oversized:
        return family_error(ErrorKind::book, {}, take(failure->payload.Oversized.detail), tag);
    }

    return family_error(ErrorKind::book, {}, {}, tag);
}

inline Error from(const RolaVCSIndexReadingError *failure) {
    if (failure == nullptr) {
        return Error{ErrorKind::unreadable, {}, "the index read failed without a reason"};
    }

    const auto tag = static_cast<std::uint32_t>(failure->tag);
    switch (failure->tag) {
    case RolaVCSIndexReadingError_NotFound:
        return family_error(ErrorKind::not_found, take(failure->payload.NotFound.key),
                            "nothing is stored under the key", tag);
    case RolaVCSIndexReadingError_Malformed:
        return family_error(ErrorKind::malformed, {},
                            "what is stored is not an index object", tag);
    case RolaVCSIndexReadingError_Corrupt:
        return family_error(ErrorKind::unreadable, take(failure->payload.Corrupt.key),
                            "what is stored under the key does not hash to it", tag);
    case RolaVCSIndexReadingError_Io: {
        Owned<RolaIoError> io{failure->payload.Io};
        auto inner = from(io.get());
        return family_error(ErrorKind::unreadable, std::string{inner.subject()},
                            std::string{inner.message()}, tag);
    }
    }

    return family_error(ErrorKind::unreadable, {}, {}, tag);
}

inline Error from(const RolaInverseIndexError *failure) {
    if (failure == nullptr) {
        return Error{ErrorKind::index, {}, "the inverse index failed without a reason"};
    }

    const auto tag = static_cast<std::uint32_t>(failure->tag);
    switch (failure->tag) {
    case RolaInverseIndexError_Read: {
        auto inner = from(&failure->payload.Read);
        return family_error(ErrorKind::index, std::string{inner.subject()},
                            std::string{inner.message()}, tag);
    }
    case RolaInverseIndexError_Malformed:
        return family_error(ErrorKind::index, {},
                            "what is there does not read as an inverse index this build knows",
                            tag);
    case RolaInverseIndexError_Io: {
        Owned<RolaIoError> io{failure->payload.Io};
        auto inner = from(io.get());
        return family_error(ErrorKind::index, std::string{inner.subject()},
                            std::string{inner.message()}, tag);
    }
    }

    return family_error(ErrorKind::index, {}, {}, tag);
}

inline Error from(const RolaInverseIndexReadingError *failure) {
    if (failure == nullptr) {
        return Error{ErrorKind::index, {}, "the inverse index read failed without a reason"};
    }

    const auto tag = static_cast<std::uint32_t>(failure->tag);
    switch (failure->tag) {
    case RolaInverseIndexReadingError_NotFound:
        return family_error(ErrorKind::not_found, take(failure->payload.NotFound.key),
                            "nothing is stored under the key", tag);
    case RolaInverseIndexReadingError_Malformed:
        return family_error(ErrorKind::malformed, {},
                            "what is there is not a kind of object this build knows", tag);
    case RolaInverseIndexReadingError_Read:
        return family_error(ErrorKind::index, {}, take(failure->payload.Read.cause), tag);
    case RolaInverseIndexReadingError_Io: {
        Owned<RolaIoError> io{failure->payload.Io};
        auto inner = from(io.get());
        return family_error(ErrorKind::index, std::string{inner.subject()},
                            std::string{inner.message()}, tag);
    }
    }

    return family_error(ErrorKind::index, {}, {}, tag);
}

inline Error from(const RolaActionError *failure) {
    if (failure == nullptr) {
        return Error{ErrorKind::action, {}, "the exchange failed without a reason"};
    }

    const auto tag = static_cast<std::uint32_t>(failure->tag);
    switch (failure->tag) {
    case RolaActionError_NoChannel:
        return family_error(ErrorKind::action, {}, "the action has no channel to speak over", tag);
    case RolaActionError_MissingValue:
        return family_error(ErrorKind::action, {}, "a value the exchange needed was held by neither side",
                            tag);
    case RolaActionError_MissingObject:
        return family_error(ErrorKind::action, {}, "an object the exchange needed was held by neither side",
                            tag);
    case RolaActionError_ValueTooLarge:
        return family_error(ErrorKind::action, {}, "a value is longer than a frame can carry", tag);
    case RolaActionError_Io: {
        Owned<RolaIoError> io{failure->payload.Io};
        auto inner = from(io.get());
        return family_error(ErrorKind::action, std::string{inner.subject()},
                            std::string{inner.message()}, tag);
    }
    case RolaActionError_Codec: {
        Owned<RolaBincodeError> codec{failure->payload.Codec};
        auto inner = from(codec.get());
        return family_error(ErrorKind::action, std::string{inner.subject()},
                            std::string{inner.message()}, tag);
    }
    case RolaActionError_Store:
        return family_error(ErrorKind::action, {}, take(failure->payload.Store), tag);
    case RolaActionError_UnknownAction:
        return family_error(ErrorKind::action, {},
                            "no action answers to the id asked for", tag);
    case RolaActionError_Json:
        return family_error(ErrorKind::action, {}, take(failure->payload.Json.message), tag);
    case RolaActionError_Addr: {
        Owned<RolaAddrError> addr{failure->payload.Addr};
        auto inner = from(addr.get());
        return family_error(ErrorKind::action, std::string{inner.subject()},
                            std::string{inner.message()}, tag);
    }
    case RolaActionError_Auth: {
        auto inner = from(&failure->payload.Auth);
        return family_error(ErrorKind::action, std::string{inner.subject()},
                            std::string{inner.message()}, tag);
    }
    case RolaActionError_LocalFile: {
        Owned<RolaIoError> io{failure->payload.LocalFile};
        auto inner = from(io.get());
        return family_error(ErrorKind::action, std::string{inner.subject()},
                            std::string{inner.message()}, tag);
    }
    case RolaActionError_NotConfirmed:
        return family_error(ErrorKind::action, {}, take(failure->payload.NotConfirmed), tag);
    case RolaActionError_Transport:
        return family_error(ErrorKind::action, {}, take(failure->payload.Transport), tag);
    }

    return family_error(ErrorKind::action, {}, {}, tag);
}

inline Error from(const RolaExistenceError *failure) {
    if (failure == nullptr) {
        return Error{ErrorKind::unreadable, {}, "the existence read failed without a reason"};
    }

    const auto tag = static_cast<std::uint32_t>(failure->tag);
    switch (failure->tag) {
    case RolaExistenceError_NoWorkspace:
        return family_error(ErrorKind::no_workspace, {}, {}, tag);
    case RolaExistenceError_NoLayout:
        return family_error(ErrorKind::no_layout, {}, {}, tag);
    case RolaExistenceError_NoIndex:
        return family_error(ErrorKind::no_index, {}, {}, tag);
    case RolaExistenceError_BadRequest:
        return family_error(ErrorKind::invalid_argument, {},
                            take(failure->payload.BadRequest.reason), tag);
    case RolaExistenceError_Read:
        return family_error(ErrorKind::unreadable, {}, take(failure->payload.Read.reason), tag);
    case RolaExistenceError_NoAccount:
        return family_error(ErrorKind::no_account, take(failure->payload.NoAccount.name), {}, tag);
    case RolaExistenceError_NoCurrentAccount:
        return family_error(ErrorKind::no_account, {}, "no account was named", tag);
    }

    return family_error(ErrorKind::unreadable, {}, {}, tag);
}

inline Error from(const RolaVariantGraphError *failure) {
    if (failure == nullptr) {
        return Error{ErrorKind::unreadable, {}, "the graph read failed without a reason"};
    }

    const auto tag = static_cast<std::uint32_t>(failure->tag);
    switch (failure->tag) {
    case RolaVariantGraphError_NoWorkspace:
        return family_error(ErrorKind::no_workspace, {}, {}, tag);
    case RolaVariantGraphError_NoLayout:
        return family_error(ErrorKind::no_layout, {},
                            "the Workspace is working in no Layout", tag);
    case RolaVariantGraphError_NoIndex:
        return family_error(ErrorKind::no_index, {}, {}, tag);
    case RolaVariantGraphError_Read:
        return family_error(ErrorKind::unreadable, {}, take(failure->payload.Read.reason), tag);
    }

    return family_error(ErrorKind::unreadable, {}, {}, tag);
}

inline Error from(const RolaVCSIndexWritingError *failure) {
    if (failure == nullptr) {
        return Error{ErrorKind::index, {}, "the index write failed without a reason"};
    }

    const auto tag = static_cast<std::uint32_t>(failure->tag);
    switch (failure->tag) {
    case RolaVCSIndexWritingError_Malformed:
        return family_error(ErrorKind::malformed, {},
                            "what was handed over does not encode as an index object", tag);
    case RolaVCSIndexWritingError_Io: {
        Owned<RolaIoError> io{failure->payload.Io};
        auto inner = from(io.get());
        return family_error(ErrorKind::index, std::string{inner.subject()},
                            std::string{inner.message()}, tag);
    }
    }

    return family_error(ErrorKind::index, {}, {}, tag);
}

inline std::string configure_reason_text(RolaConfigureErrorReason reason) {
    switch (reason) {
    case RolaConfigureErrorReason_Missing:
        return "nothing is there";
    case RolaConfigureErrorReason_Denied:
        return "the file is there but not allowed";
    case RolaConfigureErrorReason_WrongKind:
        return "the file is not what the operation needed";
    case RolaConfigureErrorReason_Busy:
        return "the file is in use";
    case RolaConfigureErrorReason_Other:
        return "the filesystem refused for an unnamed reason";
    }

    return "the filesystem refused";
}

inline Error from(const RolaConfigureError *failure) {
    if (failure == nullptr) {
        return Error{ErrorKind::internal, {}, "a configuration file failed without a reason"};
    }

    const auto tag = static_cast<std::uint32_t>(failure->tag);
    switch (failure->tag) {
    case RolaConfigureError_Read:
        return family_error(ErrorKind::unreadable, take(failure->payload.Read.file),
                            configure_reason_text(failure->payload.Read.reason), tag);
    case RolaConfigureError_Stage:
        return family_error(ErrorKind::io, take(failure->payload.Stage.file),
                            configure_reason_text(failure->payload.Stage.reason), tag);
    case RolaConfigureError_Publish:
        return family_error(ErrorKind::io, take(failure->payload.Publish.file),
                            configure_reason_text(failure->payload.Publish.reason), tag);
    case RolaConfigureError_Parse:
        return family_error(ErrorKind::malformed, take(failure->payload.Parse.file),
                            "the file's contents do not parse as the configuration type", tag);
    case RolaConfigureError_Render:
        return family_error(ErrorKind::malformed, take(failure->payload.Render.file),
                            "the configuration could not be rendered in the file's format", tag);
    case RolaConfigureError_Locked:
        return family_error(ErrorKind::locked, take(failure->payload.Locked.file),
                            "a staging file was left behind by an earlier edit", tag);
    }

    return family_error(ErrorKind::internal, {}, {}, tag);
}

inline Error from(RolaVaultCreationError failure) {
    const auto tag = static_cast<std::uint32_t>(failure);
    switch (failure) {
    case RolaVaultCreationError_DirCreateFailed:
        return family_error(ErrorKind::io, {}, "the Vault's directory could not be made", tag);
    case RolaVaultCreationError_LayoutCreateFailed:
        return family_error(ErrorKind::layout, {}, "the Vault's Layout could not be made", tag);
    case RolaVaultCreationError_ConfigLocked:
        return family_error(ErrorKind::locked, {}, "the Vault's configuration is being written", tag);
    case RolaVaultCreationError_ConfigRenderFailed:
    case RolaVaultCreationError_ConfigWriteRenderFailed:
        return family_error(ErrorKind::malformed, {},
                            "the Vault's configuration could not be rendered", tag);
    case RolaVaultCreationError_ConfigStageFailed:
    case RolaVaultCreationError_ConfigPublishFailed:
    case RolaVaultCreationError_ConfigWriteStageFailed:
        return family_error(ErrorKind::io, {},
                            "the Vault's configuration could not be written", tag);
    case RolaVaultCreationError_UnknownError:
        return family_error(ErrorKind::internal, {},
                            "the Vault could not be made for an unnamed reason", tag);
    }

    return family_error(ErrorKind::internal, {}, {}, tag);
}

inline Error from(RolaWorkspaceCreationError failure) {
    const auto tag = static_cast<std::uint32_t>(failure);
    switch (failure) {
    case RolaWorkspaceCreationError_DataDirCreateFailed:
        return family_error(ErrorKind::io, {}, "the Workspace's data directory could not be made",
                            tag);
    case RolaWorkspaceCreationError_ConfigLocked:
        return family_error(ErrorKind::locked, {},
                            "the Workspace's configuration is being written", tag);
    case RolaWorkspaceCreationError_ConfigRenderFailed:
    case RolaWorkspaceCreationError_ConfigWriteRenderFailed:
        return family_error(ErrorKind::malformed, {},
                            "the Workspace's configuration could not be rendered", tag);
    case RolaWorkspaceCreationError_ConfigStageFailed:
    case RolaWorkspaceCreationError_ConfigPublishFailed:
    case RolaWorkspaceCreationError_ConfigWriteStageFailed:
        return family_error(ErrorKind::io, {},
                            "the Workspace's configuration could not be written", tag);
    case RolaWorkspaceCreationError_UnknownError:
        return family_error(ErrorKind::internal, {},
                            "the Workspace could not be made for an unnamed reason", tag);
    }

    return family_error(ErrorKind::internal, {}, {}, tag);
}

inline Error from(RolaVCSIndexObjectParseError failure) {
    const auto tag = static_cast<std::uint32_t>(failure);
    switch (failure) {
    case RolaVCSIndexObjectParseError_ExpectVariant:
        return family_error(ErrorKind::malformed, {}, "a Variant was expected, and none was there",
                            tag);
    case RolaVCSIndexObjectParseError_ExpectVersion:
        return family_error(ErrorKind::malformed, {}, "a Version was expected, and none was there",
                            tag);
    case RolaVCSIndexObjectParseError_ExpectCreator:
        return family_error(ErrorKind::malformed, {}, "a Creator was expected, and none was there",
                            tag);
    case RolaVCSIndexObjectParseError_ExpectMessage:
        return family_error(ErrorKind::malformed, {}, "a Message was expected, and none was there",
                            tag);
    }

    return family_error(ErrorKind::malformed, {}, {}, tag);
}

} // namespace rola::detail
