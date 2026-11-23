/*
 * The status across the C++ layer: how the work stands beside the Layout, read out whole.
 *
 * A reading copies every list out while its native handle is alive, so the value a caller holds has
 * no lifetime to respect. What is checked here is that read shape, the lookups a reader uses on it,
 * and the failure a directory outside every Workspace answers with.
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

    auto status = rola::read(scope->status());
    want(status.has_value(), "a Workspace's status reads");
    if (status) {
        want(status->root() == scope->root(), "the reading names the root it was made at");

        // The lists are read out whole, so reading them is reading the owner's own values; what
        // they hold is the fixture's business, not this layer's.
        (void)status->lost();
        (void)status->untagged();
        (void)status->ignored();
        (void)status->modified();
        (void)status->wrong_base();
        (void)status->unowned();

        // An absolute path under the reading is named the way the reading names its lists, so a
        // path it holds no state for is answered with nothing rather than with a default.
        const auto absent = directory / "art" / "nowhere.psd";
        want(!status->state_of(absent.string()).has_value(),
             "a path the reading is not about has no state");
        want(!status->standing_of(absent.string()).has_value(),
             "a path the reading is not about has no standing");
        want(!status->distance_of(absent.string()).has_value(),
             "a path the reading is not about is no distance from upstream");
    }

    auto outside = rola::read(rola::StatusScope{"/nonexistent/rola/status"});
    want(!outside.has_value(), "a directory outside every Workspace is a failure");
    if (!outside) {
        want(outside.error().kind() == rola::ErrorKind::unavailable,
             "the failure is unavailable");
    }

    return harness::finish();
}
