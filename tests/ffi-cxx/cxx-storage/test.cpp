/*
 * The store across the C++ layer: what it holds, and the chunks one object is put back together from.
 *
 * The fixture Workspace holds one small file in its store, made by the runner. What a caller leans
 * on is the two listings read out whole as values, the manifest of one key read on demand, and the
 * failure a directory with no store answers with.
 */

#include <rola_plus_plus.hpp>

#include "harness.hpp"

int main() {
    using harness::want;

    const auto directory = harness::fixture("ROLA_WORKSPACE");
    const auto key = harness::text("ROLA_STORAGE_KEY");
    want(!key.empty(), "the runner names the key the fixture content is stored under");

    auto scope = rola::WorkspaceScope::open(directory);
    want(scope.has_value(), "the fixture Workspace is found");
    if (!scope || key.empty()) {
        return harness::finish();
    }

    auto storage = rola::read(scope->storage());
    want(storage.has_value(), "a Workspace's store reads");
    if (storage) {
        bool found = false;
        for (const auto &listed : storage->keys()) {
            if (listed == key) {
                found = true;
            }
        }
        want(found, "the content the fixture stored is listed under its key");
        want(storage->place_root() == scope->root(), "the reading names the place it reads");

        auto manifest = storage->manifest(key);
        want(manifest.has_value(), "the manifest of a stored key reads");
        if (manifest) {
            want(!manifest->has_value(), "a small object is kept whole, with no manifest");
        }
    }

    auto outside = rola::read(rola::StorageScope{"/nonexistent/rola/store"});
    want(!outside.has_value(), "a directory outside every place is a failure");
    if (!outside) {
        want(outside.error().kind() == rola::ErrorKind::no_store,
             "the failure is no_store");
    }

    return harness::finish();
}
