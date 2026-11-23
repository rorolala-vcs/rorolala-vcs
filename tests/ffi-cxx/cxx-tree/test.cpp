/*
 * The tree across the C++ layer: the mark a listing asks about, and the failure a path with no tree
 * answers with.
 *
 * The mark is the cheap question a listing asks about every directory it draws, so it is read
 * without reading the tree's work. What crosses is the id and the description staged for the next
 * version, both values.
 */

#include <rola_plus_plus.hpp>

#include "harness.hpp"

int main() {
    using harness::want;

    const auto tree = harness::fixture("ROLA_TREE");
    const auto directory = harness::fixture("ROLA_WORKSPACE");
    want(!tree.empty(), "the runner names the fixture tree");

    want(rola::TreeScope::is_tree(tree), "a directory with the mark is a tree");
    want(!rola::TreeScope::is_tree(directory), "a Workspace's root is not a tree");

    const auto mark = rola::TreeScope::mark_of(tree);
    want(mark.has_value(), "a tree has a mark to read");
    if (mark) {
        want(mark->id == "tree-id", "a tree carries its id");
        want(mark->described, "a staged description is marked as one");
        want(mark->description == "what the next version says",
             "the staged description crosses as data");
    }

    want(!rola::TreeScope::mark_of(directory).has_value(),
         "a directory without the mark has no mark to read");
    want(!rola::TreeScope::mark_of("/nonexistent/rola/tree").has_value(),
         "a directory that is not there has no mark to read");

    auto outside = rola::read(rola::TreeScope{"/nonexistent/rola/tree"});
    want(!outside.has_value(), "a directory outside every tree is a failure");
    if (!outside) {
        want(outside.error().kind() == rola::ErrorKind::unavailable,
             "the failure is unavailable");
    }

    return harness::finish();
}
