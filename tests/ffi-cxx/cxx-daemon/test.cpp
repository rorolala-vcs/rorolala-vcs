/*
 * The daemon requester across the C++ layer: an account found by name, and an action that reaches a
 * target nothing answers at.
 *
 * The fixture Workspace holds a private key, so the account is one the layer really finds rather
 * than a handle made for the test. What is checked is that the requester takes the reading and the
 * account, and that a Vault nothing listens at is a typed failure rather than a crash or a silent
 * success.
 */

#include <system_error>

#include <rola_plus_plus.hpp>

#include "harness.hpp"

int main() {
    using harness::want;

    const auto directory = harness::fixture("ROLA_WORKSPACE");
    const auto name = harness::text("ROLA_ACCOUNT");
    want(!directory.empty() && !name.empty(), "the runner names the fixture Workspace and account");

    // `Account::find` searches the keys beside the Workspace the current directory is inside, so the
    // program stands where the fixture Workspace is before asking.
    std::error_code here;
    std::filesystem::current_path(directory, here);
    want(!here, "the run stands in the fixture Workspace");

    auto scope = rola::WorkspaceScope::open(directory);
    want(scope.has_value(), "the fixture Workspace is found");
    if (!scope) {
        return harness::finish();
    }

    auto reading = rola::read(*scope);
    want(reading.has_value(), "the Workspace reads");
    if (!reading) {
        return harness::finish();
    }

    auto account = rola::Account::find(name);
    want(account.has_value(), "the fixture account is found by name");
    if (account) {
        want(account->valid(), "the account holds a key");
        want(account->name() == name, "the account answers the name it is found by");
    }

    want(!rola::Account::find("nobody-here").has_value(),
         "a name no key scope holds is not an account");

    if (account) {
        // Nothing listens at port 1, so the action fails before it exchanges anything.
        rola::DaemonRequester requester{*reading, *account, "tcp://127.0.0.1:1"};

        auto asked = requester.sync_index_all();
        want(!asked.has_value(), "a Vault nothing listens at refuses the action");
        if (!asked) {
            want(asked.error().kind() != rola::ErrorKind::none, "the refusal names a kind");
        }

        auto synced = requester.sync_all("all");
        want(!synced.has_value(), "a request reaches the same refusal");
    }

    return harness::finish();
}
