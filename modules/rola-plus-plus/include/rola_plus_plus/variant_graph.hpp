#pragma once

#include <cstdint>
#include <expected>
#include <filesystem>
#include <map>
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

// One entry a variant graph is centred on: the side it is named on and its path.
// The side is `local` for the Layout being worked in, or the name of a Vault
// whose copy was fetched.
struct VariantGraphFocus {
    std::string side;
    std::string path;
};

// What a variant graph is asked for. It is a window rather than a file: `focus`
// names the entries to stand on and the rest says how far to look.
struct VariantGraphRequest {
    std::vector<VariantGraphFocus> focus;
    std::uint32_t back = 64;
    std::uint32_t forward = 0;
    bool joins = true;
    bool branches = true;
    std::uint32_t max_nodes = 400;
};

// What an entry the reading stood on resolved to.
struct VariantGraphFocusResolution {
    std::string side;
    std::string path;
    std::optional<std::string> version;
    std::optional<std::string> variant;
};

// Where a node's walks were not taken in.
struct VariantGraphFrontier {
    bool parents = false;
    bool children = false;
    bool joined = false;
};

// One variant in the window.
struct VariantGraphNode {
    std::string hash;
    std::string base_version;
    std::optional<std::string> base_variant;
    std::optional<std::string> join;
    std::string storage;
    bool offline = false;
    std::string creator;
    std::string message;
    VariantGraphFrontier frontier;
};

// One version fixing a variant in the window.
struct VariantGraphVersion {
    std::string hash;
    std::string variant;
    std::optional<std::uint64_t> number;
    bool is_root = false;
};

// One edge between two variants in the window.
struct VariantGraphEdge {
    std::string from;
    std::string to;
    std::string kind;
};

// One key the window names but the index does not hold.
struct VariantGraphMissing {
    std::string hash;
    std::string kind;
};

// One entry of one side, named beside the graph.
struct VariantGraphEntry {
    std::string side;
    std::string path;
    std::string uuid;
    std::optional<std::string> version;
    std::optional<std::string> owner;
    bool deprecated = false;
};

// One merge in progress.
struct VariantGraphMerging {
    std::string variant;
    std::string target;
    std::string path;
    std::string file;
};

// The words of what the window's variants name, by the hash they name it by.
struct VariantGraphTexts {
    std::map<std::string, std::string> creators;
    std::map<std::string, std::string> messages;
};

// The variant graph around a set of entries, as the Desktop's Version Graph
// draws it. It is the index's graph; what only the Workspace knows is read
// beside it. A key the index does not hold is reported in `missing` rather than
// fetched, because a reading never reaches the network.
namespace detail {
struct VariantGraphAccess;
}

class VariantGraphReading {
public:
    VariantGraphReading(const VariantGraphReading &) = default;
    VariantGraphReading &operator=(const VariantGraphReading &) = default;
    VariantGraphReading(VariantGraphReading &&) noexcept = default;
    VariantGraphReading &operator=(VariantGraphReading &&) noexcept = default;
    ~VariantGraphReading() = default;

    // What each entry the request stood on resolved to, in the order asked.
    [[nodiscard]] std::span<const VariantGraphFocusResolution> focus() const noexcept {
        return focus_;
    }

    // The variants in the window, in key order.
    [[nodiscard]] std::span<const VariantGraphNode> nodes() const noexcept { return nodes_; }

    // The versions fixing them, in key order.
    [[nodiscard]] std::span<const VariantGraphVersion> versions() const noexcept {
        return versions_;
    }

    // The edges between variants in the window.
    [[nodiscard]] std::span<const VariantGraphEdge> edges() const noexcept { return edges_; }

    // The keys the window names but the index does not hold.
    [[nodiscard]] std::span<const VariantGraphMissing> missing() const noexcept {
        return missing_;
    }

    // Whether the window was cut short by its cap rather than walked out.
    [[nodiscard]] bool truncated() const noexcept { return truncated_; }

    // The entries whose version the window holds, the work's own first.
    [[nodiscard]] std::span<const VariantGraphEntry> entries() const noexcept { return entries_; }

    // The merges in progress.
    [[nodiscard]] std::span<const VariantGraphMerging> merging() const noexcept {
        return merging_;
    }

    // The words of what the window's variants name.
    [[nodiscard]] const VariantGraphTexts &texts() const noexcept { return texts_; }

private:
    friend struct detail::VariantGraphAccess;

    explicit VariantGraphReading(Owned<RolaVariantGraph> graph);

    std::vector<VariantGraphFocusResolution> focus_;
    std::vector<VariantGraphNode> nodes_;
    std::vector<VariantGraphVersion> versions_;
    std::vector<VariantGraphEdge> edges_;
    std::vector<VariantGraphMissing> missing_;
    bool truncated_ = false;
    std::vector<VariantGraphEntry> entries_;
    std::vector<VariantGraphMerging> merging_;
    VariantGraphTexts texts_;
};

// The variant graph of a directory, drawn around the entries a request names.
class VariantGraphScope {
public:
    explicit VariantGraphScope(std::filesystem::path root,
                               VariantGraphRequest request = {})
        : root_(std::filesystem::absolute(std::move(root)).lexically_normal()),
          request_(std::move(request)) {}

    [[nodiscard]] const std::filesystem::path &root() const noexcept { return root_; }

    [[nodiscard]] const VariantGraphRequest &request() const noexcept { return request_; }

private:
    std::filesystem::path root_;
    VariantGraphRequest request_;
};

namespace detail {

inline Error variant_graph_error(const RolaVariantGraphError *error_handle) {
    return from(error_handle);
}

} // namespace detail

namespace detail {

struct VariantGraphAccess {
    static VariantGraphReading make(Owned<RolaVariantGraph> graph) {
        return VariantGraphReading{std::move(graph)};
    }
};

} // namespace detail

inline VariantGraphReading::VariantGraphReading(Owned<RolaVariantGraph> graph) {
    if (!graph) {
        return;
    }

    const auto foci = raw::variant_graph_focus_count(graph.get());
    focus_.reserve(foci);
    for (std::uintptr_t index = 0; index < foci; ++index) {
        Owned<RolaVariantGraphFocusResolution> one{
            raw::variant_graph_focus_at(graph.get(), index)};
        if (!one) {
            continue;
        }
        focus_.push_back(VariantGraphFocusResolution{
            .side = detail::take(raw::variant_graph_focus_resolution_side(one.get())),
            .path = detail::take(raw::variant_graph_focus_resolution_path(one.get())),
            .version = detail::presence(
                detail::take(raw::variant_graph_focus_resolution_version(one.get()))),
            .variant = detail::presence(
                detail::take(raw::variant_graph_focus_resolution_variant(one.get()))),
        });
    }

    const auto nodes = raw::variant_graph_node_count(graph.get());
    nodes_.reserve(nodes);
    for (std::uintptr_t index = 0; index < nodes; ++index) {
        Owned<RolaVariantGraphNode> one{raw::variant_graph_node_at(graph.get(), index)};
        if (!one) {
            continue;
        }
        nodes_.push_back(VariantGraphNode{
            .hash = detail::take(raw::variant_graph_node_hash(one.get())),
            .base_version = detail::take(raw::variant_graph_node_base_version(one.get())),
            .base_variant =
                detail::presence(detail::take(raw::variant_graph_node_base_variant(one.get()))),
            .join = detail::presence(detail::take(raw::variant_graph_node_join(one.get()))),
            .storage = detail::take(raw::variant_graph_node_storage(one.get())),
            .offline = raw::variant_graph_node_offline(one.get()),
            .creator = detail::take(raw::variant_graph_node_creator(one.get())),
            .message = detail::take(raw::variant_graph_node_message(one.get())),
            .frontier = VariantGraphFrontier{
                .parents = raw::variant_graph_node_frontier_parents(one.get()),
                .children = raw::variant_graph_node_frontier_children(one.get()),
                .joined = raw::variant_graph_node_frontier_joined(one.get()),
            },
        });
    }

    const auto versions = raw::variant_graph_version_count(graph.get());
    versions_.reserve(versions);
    for (std::uintptr_t index = 0; index < versions; ++index) {
        Owned<RolaVariantGraphVersion> one{raw::variant_graph_version_at(graph.get(), index)};
        if (!one) {
            continue;
        }
        versions_.push_back(VariantGraphVersion{
            .hash = detail::take(raw::variant_graph_version_hash(one.get())),
            .variant = detail::take(raw::variant_graph_version_variant(one.get())),
            .number = raw::variant_graph_version_number_present(one.get())
                          ? std::optional<std::uint64_t>{
                                raw::variant_graph_version_number(one.get())}
                          : std::nullopt,
            .is_root = raw::variant_graph_version_is_root(one.get()),
        });
    }

    const auto edges = raw::variant_graph_edge_count(graph.get());
    edges_.reserve(edges);
    for (std::uintptr_t index = 0; index < edges; ++index) {
        Owned<RolaVariantGraphEdge> one{raw::variant_graph_edge_at(graph.get(), index)};
        if (!one) {
            continue;
        }
        edges_.push_back(VariantGraphEdge{
            .from = detail::take(raw::variant_graph_edge_from(one.get())),
            .to = detail::take(raw::variant_graph_edge_to(one.get())),
            .kind = detail::take(raw::variant_graph_edge_kind(one.get())),
        });
    }

    const auto missing = raw::variant_graph_missing_count(graph.get());
    missing_.reserve(missing);
    for (std::uintptr_t index = 0; index < missing; ++index) {
        Owned<RolaVariantGraphMissing> one{raw::variant_graph_missing_at(graph.get(), index)};
        if (!one) {
            continue;
        }
        missing_.push_back(VariantGraphMissing{
            .hash = detail::take(raw::variant_graph_missing_hash(one.get())),
            .kind = detail::take(raw::variant_graph_missing_kind(one.get())),
        });
    }

    truncated_ = raw::variant_graph_truncated(graph.get());

    const auto entries = raw::variant_graph_entry_count(graph.get());
    entries_.reserve(entries);
    for (std::uintptr_t index = 0; index < entries; ++index) {
        Owned<RolaVariantGraphEntry> one{raw::variant_graph_entry_at(graph.get(), index)};
        if (!one) {
            continue;
        }
        entries_.push_back(VariantGraphEntry{
            .side = detail::take(raw::variant_graph_entry_side(one.get())),
            .path = detail::take(raw::variant_graph_entry_path(one.get())),
            .uuid = detail::take(raw::variant_graph_entry_uuid(one.get())),
            .version = detail::presence(detail::take(raw::variant_graph_entry_version(one.get()))),
            .owner = detail::presence(detail::take(raw::variant_graph_entry_owner(one.get()))),
            .deprecated = raw::variant_graph_entry_deprecated(one.get()),
        });
    }

    const auto merging = raw::variant_graph_merging_count(graph.get());
    merging_.reserve(merging);
    for (std::uintptr_t index = 0; index < merging; ++index) {
        Owned<RolaVariantGraphMerging> one{raw::variant_graph_merging_at(graph.get(), index)};
        if (!one) {
            continue;
        }
        merging_.push_back(VariantGraphMerging{
            .variant = detail::take(raw::variant_graph_merging_variant(one.get())),
            .target = detail::take(raw::variant_graph_merging_target(one.get())),
            .path = detail::take(raw::variant_graph_merging_path(one.get())),
            .file = detail::take(raw::variant_graph_merging_file(one.get())),
        });
    }

    const auto words = [graph = graph.get()](bool creators) {
        std::map<std::string, std::string> out;
        const auto many = creators ? raw::variant_graph_creator_count(graph)
                                   : raw::variant_graph_message_count(graph);
        for (std::uintptr_t index = 0; index < many; ++index) {
            Owned<RolaVariantGraphText> one{creators
                                                ? raw::variant_graph_creator_at(graph, index)
                                                : raw::variant_graph_message_at(graph, index)};
            if (!one) {
                continue;
            }
            out[detail::take(raw::variant_graph_text_hash(one.get()))] =
                detail::take(raw::variant_graph_text_text(one.get()));
        }
        return out;
    };

    texts_.creators = words(true);
    texts_.messages = words(false);
}

// Read the variant graph around the entries a request names. A place with no
// Workspace, no Layout or no index, and a graph that does not read back, are the
// failures.
inline std::expected<VariantGraphReading, Error> read(const VariantGraphScope &scope) {
    const auto directory = detail::utf8(scope.root());
    const auto &request = scope.request();

    // The native side borrows every string for the call. Reserving before the
    // first push keeps each one where its pointer was taken from.
    std::vector<std::string> names;
    names.reserve(request.focus.size());
    for (const auto &one : request.focus) {
        names.push_back(one.side == "local" ? std::string{} : one.side);
    }

    std::vector<std::string> paths;
    paths.reserve(request.focus.size());
    for (const auto &one : request.focus) {
        paths.push_back(one.path);
    }

    std::vector<RolaVariantGraphFocus> focus;
    focus.reserve(request.focus.size());
    for (std::size_t index = 0; index < request.focus.size(); ++index) {
        RolaVariantGraphFocus one{};
        if (names[index].empty()) {
            one.side.tag = RolaVariantGraphSide_Local;
        } else {
            one.side.tag = RolaVariantGraphSide_Vault;
            one.side.payload.Vault.name = const_cast<char *>(names[index].c_str());
        }
        one.path = paths[index].c_str();
        focus.push_back(one);
    }

    RolaVariantGraphWindow window{};
    window.back = request.back;
    window.forward = request.forward;
    window.joins = request.joins;
    window.branches = request.branches;
    window.max_nodes = request.max_nodes;

    auto answer = take<raw::variant_graph_locate>(raw::variant_graph_locate(
        directory.c_str(), &window, focus.empty() ? nullptr : focus.data(), focus.size()));
    if (!answer) {
        return std::unexpected{detail::variant_graph_error(answer.error().get())};
    }

    Owned<RolaVariantGraph> graph{std::move(*answer)};
    if (!graph) {
        return std::unexpected{
            Error{ErrorKind::unavailable, directory, "the variant graph read as nothing"}};
    }

    return detail::VariantGraphAccess::make(std::move(graph));
}

} // namespace rola
