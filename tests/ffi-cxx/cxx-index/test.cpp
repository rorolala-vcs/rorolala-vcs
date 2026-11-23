/*
 * The VCS index across the C++ layer: its listing read out whole, and one object read by key.
 *
 * An index is reached the way a Vault and a Workspace are: a reading or a failure. The listing is
 * read out when the reading is made, so it is a value; one object is read on demand, which is why
 * the reading moves and keeps the native handle for as long as it lives.
 */

#include <rola_plus_plus.hpp>

#include "harness.hpp"

int main() {
    using harness::want;

    const auto directory = harness::fixture("ROLA_WORKSPACE");
    auto scope = rola::WorkspaceScope::open(directory);
    want(scope.has_value(), "the fixture Workspace is found");
    if (!scope) {
        return harness::finish();
    }

    auto index = rola::read(scope->vcs_index());
    want(index.has_value(), "a Workspace keeps an index");
    if (index) {
        want(index->root() == scope->root(), "the reading names the root it was made at");
        want(index->root_version().empty(), "an index with nothing in it has no root version");
        want(index->listing().versions.empty(), "an empty index lists no versions");
        want(index->listing().variants.empty(), "an empty index lists no variants");
        want(index->listing().string_hashes.empty(), "an empty index lists no text objects");

        const auto missing = index->object_view(
            "blake3:2222222222222222222222222222222222222222222222222222222222222222");
        want(!missing.has_value(), "a key nothing is stored under is a failure");
        if (!missing) {
            want(missing.error().kind() != rola::ErrorKind::none,
                 "the failure names a kind");
        }
    }

    auto outside = rola::read(rola::VcsIndexScope{"/nonexistent/rola/index"});
    want(!outside.has_value(), "a directory outside every index is a failure");
    if (!outside) {
        want(outside.error().kind() == rola::ErrorKind::no_index, "the failure is no_index");
    }

    return harness::finish();
}
