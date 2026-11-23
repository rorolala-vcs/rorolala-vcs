#pragma once

#include <cstddef>
#include <expected>
#include <filesystem>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include <rola_plus_plus/gen/raw.hpp>
#include <rola_plus_plus/gen/release.hpp>

#include "detail.hpp"
#include "error.hpp"
#include "write.hpp"
#include "write_request.hpp"

namespace rola {

// Writes a Layout a place holds. It names the root and the Layout by name the
// way the native side does, so the same type covers the local Layout (no name)
// and a truth Layout or a fetched copy (a name).
class LayoutWriter {
public:
    explicit LayoutWriter(std::filesystem::path root,
                          std::optional<std::string> name = std::nullopt)
        : root_path_(std::filesystem::absolute(std::move(root)).lexically_normal()),
          root_(detail::utf8(root_path_)), name_(name ? std::move(*name) : std::string{}) {}

    // Applies a batch of changes in the order the request names them. The
    // native side answers nothing on success, so the result is `void`.
    [[nodiscard]] std::expected<void, Error> apply(const LayoutApplyRequest &request) const {
        std::vector<RolaWriteLayoutOp> ops;
        ops.reserve(request.ops.size());
        for (const auto &op : request.ops) {
            ops.push_back(detail::native_layout_op(op));
        }

        auto answer = take<raw::write_layout_apply>(raw::write_layout_apply(
            root_.c_str(), name_.c_str(), ops.empty() ? nullptr : ops.data(), ops.size()));
        if (!answer) {
            return std::unexpected{detail::write_error(answer.error().get())};
        }
        return {};
    }

    [[nodiscard]] const std::filesystem::path &root() const noexcept {
        return root_path_;
    }

private:
    std::filesystem::path root_path_;
    std::string root_;
    std::string name_;
};

} // namespace rola
