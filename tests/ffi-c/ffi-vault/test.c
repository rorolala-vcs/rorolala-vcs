/*
 * The Vault across the ABI: made, located, and locked.
 *
 * The same three behaviours a C caller leans on for a Workspace, on the other side of the work:
 * making one answers in the tag-and-payload shape, locating one hands out an owned handle or
 * nothing, and locking one is a guard C holds and gives back, the Vault's own pair of locking
 * functions, so a Vault and a Workspace are met the same way and are not one type.
 */

#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
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

    RorolalaResult made = rola_vault_create(dir);
    want(made.tag == RorolalaResult_Ok, "rola_vault_create succeeds on a fresh directory");
    want(made.payload == NULL, "rola_vault_create hands back nothing on success");

    RolaVault *vault = rola_vault_locate(dir);
    want(vault != NULL, "rola_vault_locate finds the Vault just made");
    want(rola_vault_locate(plain) == NULL, "rola_vault_locate answers with nothing elsewhere");

    if (vault != NULL) {
        want(!rola_vault_is_locking(vault), "a Vault is unlocked to begin with");

        RolaVaultLocking *guard = rola_vault_lock_acquire(vault);
        want(guard != NULL, "the lock is handed out as a guard");
        want(rola_vault_is_locking(vault), "holding the guard is holding the lock");

        rola_vault_lock_release(guard);
        want(!rola_vault_is_locking(vault), "giving the guard back gives the lock back");

        free_rola_vault(vault);
    }

    /* What the Vault says about itself is data, and its one Layout is named truth. */
    RolaVaultConfig *config = rola_vault_config_read(dir);
    want(config != NULL, "a Vault has a configuration");
    if (config != NULL) {
        char *description = rola_vault_config_description(config);
        want(description != NULL && strcmp(description, "New Rola Vault") == 0,
             "a Vault says what it is");
        free_string(description);

        want(rola_vault_config_admin_count(config) == 0, "a new Vault has no administrator");
        want(rola_vault_config_max_concurrency(config) > 0,
             "a Vault carries what its daemon may serve at once");
        free_rola_vault_config(config);
    }

    RolaLayouts *layouts = rola_layouts(dir);
    want(layouts != NULL, "a Vault holds a Layout");
    if (layouts != NULL) {
        RolaLayout *one = rola_layouts_at(layouts, 0);
        if (one != NULL) {
            char *name = rola_layout_name(one);
            want(name != NULL && strcmp(name, "truth") == 0,
                 "a Vault's Layout is named truth");
            free_string(name);
            free_rola_layout(one);
        }

        free_rola_layouts(layouts);
    }

    remove_tree(dir);
    remove_tree(plain);

    return finish();
}
