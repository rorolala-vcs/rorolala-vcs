/*
 * The Layout across the C++ layer: what a place holds, what an entry says, and the failures a name
 * can come to.
 *
 * The fixture is made by the runner: a Workspace whose Layout names a file under art/ and one the
 * Vault has deprecated. What a caller leans on is the entries read out whole as values, so they
 * outlive no handle, and the three failures a name answers with: a name no place has, a copy that
 * was never fetched, and a directory outside every place.
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

    auto layout = rola::read(scope->layout());
    want(layout.has_value(), "the working Layout reads");
    if (layout) {
        want(layout->entries().size() == 2, "the Layout lists its two entries");

        const auto hero = layout->find("art/hero.psd");
        want(hero.has_value(), "the entry is found at its path");
        if (hero) {
            want(hero->owner == "alice", "the entry carries its owner");
            want(hero->description == "the hero", "the entry carries what it says about itself");
            want(hero->version.size() == 64, "the entry carries the version it is at, as hex");
            want(!hero->deprecated, "an entry in the work is not deprecated");
        }

        const auto removed = layout->find("@/removed/old.psd");
        want(removed.has_value(), "a deprecated entry keeps the path the Vault moved it to");
        if (removed) {
            want(removed->deprecated, "that entry is marked deprecated");
        }

        want(!layout->find("art/nowhere.psd").has_value(),
             "a path the Layout does not name is not found");
    }

    auto named = rola::read(rola::LayoutScope{directory, std::string{"main"}});
    want(named.has_value(), "the working Layout answers when named");

    auto missing = rola::read(rola::LayoutScope{directory, std::string{"nowhere"}});
    want(!missing.has_value(), "a name no place has is a failure");
    if (!missing) {
        want(missing.error().kind() == rola::ErrorKind::not_found,
             "the failure is not_found");
    }

    auto unfetched = rola::read(rola::LayoutScope{directory, std::string{"truth@studio"}});
    want(!unfetched.has_value(), "a copy that was never fetched is a failure");
    if (!unfetched) {
        want(unfetched.error().kind() == rola::ErrorKind::not_cached,
             "the failure is not_cached");
    }

    auto outside = rola::read(rola::LayoutScope{"/nonexistent/rola/layout"});
    want(!outside.has_value(), "a directory outside every place is a failure");
    if (!outside) {
        want(outside.error().kind() == rola::ErrorKind::no_workspace,
             "the failure is no_workspace");
    }

    /* A batch carries the soft delete as a change of its own, and the archive's path is worked out by
       the native side: a path already under `@/removed/` is refused rather than archived twice, so the
       fixture is left as it was read. */
    rola::LayoutApplyRequest archive;
    rola::LayoutOp change;
    change.kind = rola::LayoutOpKind::soft_delete_path;
    change.path = "@/removed/old.psd";
    archive.ops.push_back(change);

    auto archived = rola::LayoutWriter{directory, std::string{"main"}}.apply(archive);
    want(!archived.has_value(), "a path already under the archive is not archived again");
    if (!archived) {
        want(archived.error().kind() == rola::ErrorKind::invalid_path,
             "the failure is invalid_path");
    }

    return harness::finish();
}
