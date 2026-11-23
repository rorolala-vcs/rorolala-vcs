/*
 * The Workspace across the C++ layer: located, read, and its configuration turned into values.
 *
 * What a caller leans on is `WorkspaceScope::open` finding the root by walking upward, a reading
 * whose configuration names the Vaults the Workspace knows and the one it reaches for, and a
 * failure that says no Workspace holds the directory rather than handing back an empty one.
 */

#include <rola_plus_plus.hpp>

#include "harness.hpp"

int main() {
    using harness::want;

    const auto directory = harness::fixture("ROLA_WORKSPACE");
    want(!directory.empty(), "the runner names the fixture Workspace");
    if (directory.empty()) {
        return harness::finish();
    }

    auto scope = rola::WorkspaceScope::open(directory);
    want(scope.has_value(), "a Workspace is found from a directory inside it");
    if (!scope) {
        return harness::finish();
    }

    auto reading = rola::read(*scope);
    want(reading.has_value(), "the Workspace reads");
    if (!reading) {
        return harness::finish();
    }

    want(reading->root() == scope->root(), "the reading names the root it was made at");
    want(!reading->locked(), "a Workspace is unlocked before a write takes it");

    const auto &config = reading->config();
    want(config.default_vault.has_value() && *config.default_vault == "studio",
         "the configuration names the Vault it reaches for");

    bool studio = false;
    for (const auto &vault : config.vaults) {
        if (vault.name == "studio") {
            studio = vault.is_default && vault.address == "tcp://127.0.0.1:9270";
        }
    }
    want(studio, "a bound Vault carries its address and the mark of the default");
    want(config.vaults.size() == 2, "every bound Vault is listed");
    want(!config.storages.empty(), "the configuration names the stores the Workspace reads");

    auto missing = rola::WorkspaceScope::open("/nonexistent/rola/workspace");
    want(!missing.has_value(), "a directory outside every Workspace is a failure");
    if (!missing) {
        want(missing.error().kind() == rola::ErrorKind::no_workspace,
             "the failure is no_workspace");
        want(!missing.error().subject().empty(), "the failure names the directory it looked from");
    }

    return harness::finish();
}
