/*
 * The Vault across the ABI: made, located, and locked.
 *
 * The same three behaviours a C caller leans on for a Workspace, on the other side of the work:
 * making one answers in the tag-and-payload shape, locating one hands out an owned handle or
 * nothing, and locking one is a guard C holds and gives back — the Vault's own pair of locking
 * functions, so a Vault and a Workspace are met the same way and are not one type.
 */

#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include "rorolala_ffi.h"

#include "harness.h"

int main(void) {
    const char *tmp = getenv("TMPDIR");
    if (tmp == NULL) {
        tmp = "/tmp";
    }

    char dir[512];
    snprintf(dir, sizeof dir, "%s/rola-ffi-vault-XXXXXX", tmp);
    if (mkdtemp(dir) == NULL) {
        perror("mkdtemp");
        return 2;
    }

    char plain[512];
    snprintf(plain, sizeof plain, "%s/rola-ffi-plain-XXXXXX", tmp);
    if (mkdtemp(plain) == NULL) {
        perror("mkdtemp");
        return 2;
    }

    RorolalaResult made = create_vault(dir);
    want(made.tag == RorolalaResult_Ok, "create_vault succeeds on a fresh directory");
    want(made.payload == NULL, "create_vault hands back nothing on success");

    RolaVault *vault = locate_rola_vault(dir);
    want(vault != NULL, "locate_rola_vault finds the Vault just made");
    want(locate_rola_vault(plain) == NULL, "locate_rola_vault answers with nothing elsewhere");

    if (vault != NULL) {
        want(!RolaVault_is_locking(vault), "a Vault is unlocked to begin with");

        RolaVaultLocking *guard = RolaVault_get_locking_guard(vault);
        want(guard != NULL, "the lock is handed out as a guard");
        want(RolaVault_is_locking(vault), "holding the guard is holding the lock");

        RolaVault_drop_locking_guard(guard);
        want(!RolaVault_is_locking(vault), "giving the guard back gives the lock back");

        free_rola_vault(vault);
    }

    remove_tree(dir);
    remove_tree(plain);

    return finish();
}
