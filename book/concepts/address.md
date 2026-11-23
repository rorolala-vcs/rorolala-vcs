---
title: "About a Vault Address"
description: "The three spellings an address takes, what a namespace names, and what each part leaves to the other side."
keywords: address, rola://, link, namespace, host, port, ssh, scp, sub-vault, default port
---

# About a Vault Address

An address says where a Vault is reached. It names a daemon and a namespace, or a Vault reached over SSH. It does not name a directory: a directory is a Vault on the machine that holds it, and an address is how another machine asks for it.

## The three spellings

| Written | Taken as |
| --- | --- |
| `host` | That host, the default port, no namespace |
| `port` | `127.0.0.1`, that port, no namespace |
| `host:port` | That host and port, no namespace |
| `rola://host:port/ns` | The host, the port and the namespace |
| `rola://host/ns/sub` | The same, on the default port, with a Vault under the namespace |
| `host:port/ns` | The bare authority, then the namespace |
| `host/ns/sub` | The same, on the default port |
| `user@host:path` | An SSH session to that host, serving the Vault at that path |
| `user@host:port:path` | The same, on that SSH port |

The default port is 7717. A port on its own means the local machine, so `7717` is the daemon this machine runs. A host is an ip or a name, and a name is kept as it was written rather than resolved, so an address stays the address it was written as.

## The link

The `rola://` form is the one the address book writes, because it names everything:

```text
rola://<host>[:<port>]/<namespace>[/<sub-vault>]
```

The first path segment is the namespace. Anything after it names a Vault under the one the namespace was registered for. A namespace is what makes a link name a Vault rather than a daemon.

A link without a port takes the default port. A link with a trailing slash and no namespace names the daemon and no namespace, which a request answers at the root.

## The SSH spelling

An address written the way `scp` writes one names a session rather than a socket:

```text
<user>@<host>[:<port>]:<path>
```

Here the path is the Vault itself, so there is no namespace to carry. The port, when it is written, is the SSH port; leaving it out leaves the choice to SSH's own configuration.

What follows the host is a path on the far side, and this side does not read it further. It is quoted and handed to `ssh` as it was written, so it is the far side that decides whether it names a Vault.

## What an address leaves out

Every form that names a daemon fills in what it left out, because a daemon link is one thing whichever spelling was used. The namespace is empty when none was written, and the port is the default when none was given.

An SSH address is the opposite. What it leaves out is left out, because a missing port is not the default port; it is SSH's to choose. A spelling is read as what it says rather than as what it would have said with the other parts.

## Checking one

```sh
rola vault handshake rola://example.com:7420/studio
```

This speaks the handshake to the Vault the address names. It is what the launcher does when an address is checked, and what makes an entry in the address book read as reachable or without an answer.

A refusal at this point is about identity or reachability. Whether a session may change a particular file is a separate question, answered by ownership once the session is open.
