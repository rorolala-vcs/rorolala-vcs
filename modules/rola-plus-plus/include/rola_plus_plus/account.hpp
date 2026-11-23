#pragma once

#include <optional>
#include <string>
#include <string_view>
#include <utility>

#include <rola_plus_plus/gen/raw.hpp>
#include <rola_plus_plus/gen/release.hpp>

#include "detail.hpp"

namespace rola {

namespace detail {
struct AccountAccess;
}

// An account the run can sign as. It is move-only because it owns the native
// handle, and it exists so the daemon requester never has to name a raw one.
//
// The search follows the same key scopes the C# side uses: a rule with its
// defaults looks beside the user, the environment and the current place.
class Account {
public:
    Account(Account &&) noexcept = default;
    Account &operator=(Account &&) noexcept = default;
    Account(const Account &) = delete;
    Account &operator=(const Account &) = delete;
    ~Account() = default;

    // The account `name` names, when a key scope holds its private key.
    [[nodiscard]] static std::optional<Account> find(std::string_view name);

    // The account the key scopes rank highest, when any holds one.
    [[nodiscard]] static std::optional<Account> locate();

    // The name the account is known by, which is the stem of its key file.
    [[nodiscard]] std::string name() const;

    [[nodiscard]] bool valid() const noexcept { return static_cast<bool>(handle_); }

private:
    friend struct detail::AccountAccess;

    explicit Account(Owned<RolaAccount> handle) : handle_(std::move(handle)) {}

    Owned<RolaAccount> handle_;
};

namespace detail {

struct AccountAccess {
    [[nodiscard]] static RolaAccount *handle(const Account &value) noexcept {
        return value.handle_.get();
    }

    // Every lookup makes its own rule so the caller never has to keep one. The
    // defaults are what the other layer searches with.
    [[nodiscard]] static Owned<RolaKeyLocateRule> rule() {
        return Owned<RolaKeyLocateRule>{raw::key_locate_rule_new()};
    }
};

} // namespace detail

inline std::optional<Account> Account::find(std::string_view name) {
    auto rule = detail::AccountAccess::rule();
    const std::string text{name};
    Owned<RolaAccount> one{raw::account_find(text.c_str(), rule.get())};
    if (!one) {
        return std::nullopt;
    }
    return Account{std::move(one)};
}

inline std::optional<Account> Account::locate() {
    auto rule = detail::AccountAccess::rule();
    Owned<RolaAccounts> found{raw::account_locate_local(rule.get())};
    if (!found || raw::accounts_is_empty(found.get())) {
        return std::nullopt;
    }

    Owned<RolaAccount> one{raw::accounts_at(found.get(), 0)};
    if (!one) {
        return std::nullopt;
    }
    return Account{std::move(one)};
}

inline std::string Account::name() const {
    if (!handle_) {
        return {};
    }
    return detail::take(raw::account_name(handle_.get()));
}

} // namespace rola
