/*
 * Addresses across the ABI: the three spellings read into one handle, and the error naming what
 * would not read.
 *
 * A caller hands over the link it was given and reads the parts back. Nothing here is fetched or
 * dialled: what is checked is the reading, which is what a window offering a Vault or a command
 * checking one does before anything is reached.
 */

#define _POSIX_C_SOURCE 200809L

#include <string.h>

#include "rorolala_ffi.h"

#include "harness.h"

int main(void) {
    /* A bare address names a daemon and a port, and reads as what was written. */
    RorolalaResult daemon = rola_vault_address_parse("127.0.0.1:9270");
    want(daemon.tag == RorolalaResult_Ok, "a bare address reads");
    if (daemon.tag == RorolalaResult_Ok) {
        const RolaVaultAddress *address = daemon.payload;
        want(address != NULL, "the address crosses as a handle");

        char *raw = rola_vault_address_raw(address);
        want(raw != NULL && strcmp(raw, "127.0.0.1:9270") == 0, "the link is what was written");
        free_string(raw);

        want(rola_vault_address_port(address) == 9270, "the port is read");

        free_rola_vault_address(daemon.payload);
    } else if (daemon.payload != NULL) {
        free_rola_addr_error(daemon.payload);
    }

    /* An SSH address names the account and the directory, and its parts read back. */
    RorolalaResult ssh = rola_vault_address_parse("alice@example.test:/srv/vault");
    want(ssh.tag == RorolalaResult_Ok, "an SSH address reads");
    if (ssh.tag == RorolalaResult_Ok) {
        const RolaVaultAddress *address = ssh.payload;

        char *user = rola_vault_address_ssh_user(address);
        want(user != NULL && strcmp(user, "alice") == 0, "the account is read");
        free_string(user);

        char *path = rola_vault_address_ssh_path(address);
        want(path != NULL && strcmp(path, "/srv/vault") == 0, "the directory is read");
        free_string(path);

        free_rola_vault_address(ssh.payload);
    } else if (ssh.payload != NULL) {
        free_rola_addr_error(ssh.payload);
    }

    /* What is not an address is a failure that names what was written. */
    RorolalaResult bad = rola_vault_address_parse("not an address");
    want(bad.tag == RorolalaResult_Err, "what is not an address is a failure");
    if (bad.tag == RorolalaResult_Err) {
        const RolaAddrError *error = bad.payload;

        char *target = rola_addr_error_target(error);
        want(target != NULL && strcmp(target, "not an address") == 0,
             "the error names what was written");
        free_string(target);

        free_rola_addr_error(bad.payload);
    }

    return finish();
}
