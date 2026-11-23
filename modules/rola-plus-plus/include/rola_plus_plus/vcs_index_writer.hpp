#pragma once

#include <expected>
#include <filesystem>
#include <string>
#include <type_traits>
#include <utility>
#include <variant>

#include <rola_plus_plus/gen/raw.hpp>
#include <rola_plus_plus/gen/release.hpp>

#include "detail.hpp"
#include "error.hpp"
#include "vcs_index.hpp"
#include "write.hpp"
#include "write_request.hpp"
#include "write_result.hpp"

namespace rola {

// Writes objects into the VCS index a place keeps.
class VcsIndexWriter {
public:
    explicit VcsIndexWriter(std::filesystem::path root)
        : root_path_(std::filesystem::absolute(std::move(root)).lexically_normal()),
          root_(detail::utf8(root_path_)) {}

    // Writes one object and answers the key it was written under.
    [[nodiscard]] std::expected<std::string, Error> write_object(
        const IndexObjectRequest &request) const {
        RolaWriteIndexObjectRequest native{};
        std::visit(
            [&native](const auto &object) {
                using Kind = std::decay_t<decltype(object)>;
                if constexpr (std::is_same_v<Kind, IndexVariantObject>) {
                    native.tag = RolaWriteIndexObjectRequest_Variant;
                    native.payload.Variant = RolaWriteIndexObjectRequestVariant{
                        .storage = const_cast<char *>(object.storage.c_str()),
                        .base_version = const_cast<char *>(object.base_version.c_str()),
                        .join = const_cast<char *>(object.join.c_str()),
                        .creator = const_cast<char *>(object.creator.c_str()),
                        .message = const_cast<char *>(object.message.c_str()),
                    };
                } else if constexpr (std::is_same_v<Kind, IndexVersionObject>) {
                    native.tag = RolaWriteIndexObjectRequest_Version;
                    native.payload.Version = RolaWriteIndexObjectRequestVersion{
                        .variant = const_cast<char *>(object.variant.c_str()),
                    };
                } else if constexpr (std::is_same_v<Kind, IndexCreatorObject>) {
                    native.tag = RolaWriteIndexObjectRequest_Creator;
                    native.payload.Creator = RolaWriteIndexObjectRequestCreator{
                        .text = const_cast<char *>(object.text.c_str()),
                    };
                } else {
                    native.tag = RolaWriteIndexObjectRequest_Message;
                    native.payload.Message = RolaWriteIndexObjectRequestMessage{
                        .text = const_cast<char *>(object.text.c_str()),
                    };
                }
            },
            request);

        auto answer = take<raw::write_index_object>(
            raw::write_index_object(root_.c_str(), native));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteIndexObject> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, {}, "the index answered no key"}};
        }
        return detail::take(raw::write_index_object_key(handle.get()));
    }

    // Records a file's content as a variant and a version.
    [[nodiscard]] std::expected<FileVersionResult, Error> write_file_version(
        const FileVersionRequest &request) const {
        auto answer = take<raw::write_index_file_version>(raw::write_index_file_version(
            root_.c_str(), request.storage.c_str(), request.base_version.c_str(),
            request.join.c_str(), request.creator.c_str(), request.message.c_str()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWriteFileVersion> handle{std::move(*answer)};
        if (!handle) {
            return std::unexpected{
                Error{ErrorKind::internal, {}, "the index answered no version"}};
        }
        return FileVersionResult{
            .variant = detail::take(raw::write_file_version_variant(handle.get())),
            .version = detail::take(raw::write_file_version_version(handle.get())),
        };
    }

    // Lays the index out afresh.
    [[nodiscard]] std::expected<Packed, Error> pack() const {
        auto answer = take<raw::write_pack_index>(raw::write_pack_index(root_.c_str()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        Owned<RolaWritePack> handle{std::move(*answer)};
        return Packed{.changed = handle ? raw::write_pack_changed(handle.get()) : false};
    }

    [[nodiscard]] const std::filesystem::path &root() const noexcept { return root_path_; }

private:
    std::filesystem::path root_path_;
    std::string root_;
};

} // namespace rola
