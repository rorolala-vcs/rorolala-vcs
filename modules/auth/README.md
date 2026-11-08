# rorolala-auth

## Introduction

Member identity and the encrypted channel: a member is a public key file, an account is the private key it signs with, and a socket becomes a session through a signed handshake this crate performs.

## Boundary

It owns the identity model, the scopes a key search covers, the rule that narrows such a search, the account the work acts as, and the `SecureStream` that carries a session. It does not own the session itself: the actions, their context, framing and the values that cross are `modules/protocol`, and reaching a daemon or an SSH session is `modules/daemon`. It reads no Vault configuration, which `modules/vault` owns.
