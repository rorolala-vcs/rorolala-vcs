/*
 * The Layout across the ABI: what a place holds, what an entry says, and the names a place has no
 * Layout under.
 *
 * The fixtures are made by the runner: a Workspace whose Layout names a file under art/ and one the
 * Vault has deprecated, and a Vault whose own Layout names one file. What a caller leans on is the
 * typed handles the reads answer with, and the four failures a name can come to.
 */

#define _POSIX_C_SOURCE 200809L

#include <stdlib.h>
#include <string.h>

#include "rorolala_ffi.h"

#include "harness.h"

/* The fixture paths the runner names in the environment. */
static const char *fixture(const char *name) {
    const char *value = getenv(name);
    return value == NULL ? "" : value;
}

int main(void) {
    const char *workspace = fixture("ROLA_WORKSPACE");
    const char *vault = fixture("ROLA_VAULT");

    /* A Workspace holds one Layout, and it is the one being worked in. */
    RolaLayouts *layouts = rola_layouts(workspace);
    want(layouts != NULL, "a Workspace holds a Layout");
    if (layouts != NULL) {
        want(rola_layouts_count(layouts) == 1, "a Workspace holds one Layout");

        RolaLayout *one = rola_layouts_at(layouts, 0);
        want(one != NULL, "the one Layout is read");
        if (one != NULL) {
            char *name = rola_layout_name(one);
            want(name != NULL && strcmp(name, "main") == 0,
                 "a Workspace's Layout is named main");
            free_string(name);
            want(rola_layout_is_current(one), "the one Layout is the current one");
            char *tracking = rola_layout_tracking(one);
            want(tracking != NULL && tracking[0] == '\0',
                 "a Workspace's Layout tracks no Vault");
            free_string(tracking);
            free_rola_layout(one);
        }

        free_rola_layouts(layouts);
    }

    /* A Vault keeps one of its own, under the name a Vault's Layout goes by. */
    RolaLayouts *vault_layouts = rola_layouts(vault);
    want(vault_layouts != NULL, "a Vault holds a Layout");
    if (vault_layouts != NULL) {
        RolaLayout *one = rola_layouts_at(vault_layouts, 0);
        if (one != NULL) {
            char *name = rola_layout_name(one);
            want(name != NULL && strcmp(name, "truth") == 0,
                 "a Vault's Layout is named truth");
            free_string(name);
            free_rola_layout(one);
        }

        free_rola_layouts(vault_layouts);
    }

    /* A directory no place holds has no Layout, which is the null a read answers with. */
    want(rola_layouts("/nonexistent/rola/layout") == NULL,
         "a directory outside every place has no Layout");

    /* An unnamed Layout is the one being worked in, and its entries carry what each says. */
    RorolalaResult entries = rola_layout_entries(workspace, NULL);
    want(entries.tag == RorolalaResult_Ok, "the working Layout's entries answer");
    if (entries.tag == RorolalaResult_Ok) {
        RolaLayoutEntries *list = entries.payload;
        want(list != NULL, "the entries answer as a handle");
        if (list != NULL) {
            uintptr_t count = rola_layout_entries_count(list);
            want(count == 2, "the Layout lists its two entries");

            int hero = 0;
            int removed = 0;
            for (uintptr_t index = 0; index < count; index++) {
                RolaLayoutEntry *entry = rola_layout_entries_at(list, index);
                if (entry == NULL) {
                    continue;
                }

                char *path = rola_layout_entry_path(entry);
                if (path != NULL && strcmp(path, "art/hero.psd") == 0) {
                    char *owner = rola_layout_entry_owner(entry);
                    char *description = rola_layout_entry_description(entry);
                    char *version = rola_layout_entry_version(entry);
                    want(owner != NULL && strcmp(owner, "alice") == 0,
                         "the entry carries its owner");
                    want(description != NULL && strcmp(description, "the hero") == 0,
                         "the entry carries what it says about itself");
                    want(version != NULL && strlen(version) == 64,
                         "the entry carries the version it is at, as hex");
                    want(!rola_layout_entry_deprecated(entry),
                         "an entry in the work is not deprecated");
                    free_string(owner);
                    free_string(description);
                    free_string(version);
                    hero = 1;
                } else if (path != NULL && strcmp(path, "@/removed/old.psd") == 0) {
                    want(rola_layout_entry_deprecated(entry),
                         "the entry under @/removed/ is deprecated");
                    removed = 1;
                }

                free_string(path);
                free_rola_layout_entry(entry);
            }

            want(hero, "the named entry is listed at its path");
            want(removed, "a deprecated entry keeps the path the Vault moved it to");
            free_rola_layout_entries(list);
        }
    }

    /* The Layout a Workspace works in can be named as well as left out. */
    RorolalaResult named = rola_layout_entries(workspace, "main");
    want(named.tag == RorolalaResult_Ok, "the working Layout answers when named");
    if (named.tag == RorolalaResult_Ok) {
        free_rola_layout_entries(named.payload);
    }

    /* A Vault's own Layout is read from a Vault, under its own name. */
    RorolalaResult vault_entries = rola_layout_entries(vault, "truth");
    want(vault_entries.tag == RorolalaResult_Ok, "a Vault's own Layout answers");
    if (vault_entries.tag == RorolalaResult_Ok) {
        RolaLayoutEntries *list = vault_entries.payload;
        if (list != NULL) {
            int shared = 0;
            for (uintptr_t index = 0; index < rola_layout_entries_count(list); index++) {
                RolaLayoutEntry *entry = rola_layout_entries_at(list, index);
                if (entry == NULL) {
                    continue;
                }

                char *path = rola_layout_entry_path(entry);
                if (path != NULL && strcmp(path, "shared.psd") == 0) {
                    shared = 1;
                }
                free_string(path);
                free_rola_layout_entry(entry);
            }

            want(shared, "the Vault's entry is listed");
            free_rola_layout_entries(list);
        }
    }

    /* A name the place has no Layout under is missing, not an empty Layout. */
    RorolalaResult missing = rola_layout_entries(workspace, "nowhere");
    want(missing.tag == RorolalaResult_Err, "a name no place has is a failure");
    if (missing.tag == RorolalaResult_Err) {
        const RolaLayoutError *error = missing.payload;
        want(error->tag == RolaLayoutError_Missing, "it is reported as Missing");
        free_rola_layout_error(missing.payload);
    }

    /* A copy that was never fetched is told apart from one that is there. */
    RorolalaResult not_cached = rola_layout_entries(workspace, "truth@studio");
    want(not_cached.tag == RorolalaResult_Err, "an unfetched copy is a failure");
    if (not_cached.tag == RorolalaResult_Err) {
        const RolaLayoutError *error = not_cached.payload;
        want(error->tag == RolaLayoutError_NotCached, "it is reported as NotCached");
        free_rola_layout_error(not_cached.payload);
    }

    /* A directory no place holds cannot name any Layout at all. */
    RorolalaResult no_place = rola_layout_entries("/nonexistent/rola/layout", NULL);
    want(no_place.tag == RorolalaResult_Err, "a directory outside every place is a failure");
    if (no_place.tag == RorolalaResult_Err) {
        const RolaLayoutError *error = no_place.payload;
        want(error->tag == RolaLayoutError_NoWorkspace, "it is reported as NoWorkspace");
        free_rola_layout_error(no_place.payload);
    }

    /* A Workspace's configuration lists the Vaults it knows and the one it reaches for. */
    RolaWorkspaceConfig *config = rola_workspace_config_read(workspace);
    want(config != NULL, "a Workspace has a configuration");
    if (config != NULL) {
        int studio = 0;
        for (uintptr_t index = 0; index < rola_workspace_config_vault_count(config); index++) {
            RolaWorkspaceVault *binding = rola_workspace_config_vault_at(config, index);
            if (binding == NULL) {
                continue;
            }

            char *name = rola_workspace_vault_name(binding);
            if (name != NULL && strcmp(name, "studio") == 0) {
                char *address = rola_workspace_vault_address(binding);
                want(address != NULL && strcmp(address, "tcp://127.0.0.1:9270") == 0,
                     "a binding carries its address");
                want(rola_workspace_vault_is_default(binding),
                     "the Vault the Workspace reaches for is marked");
                free_string(address);
                studio = 1;
            }

            free_string(name);
            free_rola_workspace_vault(binding);
        }

        want(studio, "a bound Vault is listed by name");

        char *default_vault = rola_workspace_config_default_vault(config);
        want(default_vault != NULL && strcmp(default_vault, "studio") == 0,
             "the configuration names the Vault it reaches for");
        free_string(default_vault);

        free_rola_workspace_config(config);
    }

    /* A Vault's configuration says what it is and who may administer it. */
    RolaVaultConfig *vault_config = rola_vault_config_read(vault);
    want(vault_config != NULL, "a Vault has a configuration");
    if (vault_config != NULL) {
        char *description = rola_vault_config_description(vault_config);
        want(description != NULL && strcmp(description, "New Rola Vault") == 0,
             "a Vault says what it is");
        free_string(description);

        want(rola_vault_config_admin_count(vault_config) == 0,
             "a new Vault has no administrator");
        want(rola_vault_config_max_concurrency(vault_config) > 0,
             "a Vault carries what its daemon may serve at once");

        free_rola_vault_config(vault_config);
    }

    /* A directory no Vault holds has no configuration. */
    want(rola_vault_config_read("/nonexistent/rola/vault") == NULL,
         "a directory outside every Vault has no configuration");

    /* The two writes a fetched copy uses exist, and a Layout that is not a copy is refused as an
       argument rather than answered as one. */
    RorolalaResult moved = rola_write_layout_move_fetched(workspace, "main", "a.psd", "b.psd");
    want(moved.tag == RorolalaResult_Err, "a move in a Layout that is not a fetched copy is a failure");
    if (moved.tag == RorolalaResult_Err) {
        const RolaWriteError *error = moved.payload;
        want(error->tag == RolaWriteError_InvalidPath, "it is reported as InvalidPath");
        free_rola_write_error(moved.payload);
    }

    RorolalaResult removed = rola_write_layout_remove_fetched_entry(workspace, "truth@studio", "not-a-uuid");
    want(removed.tag == RorolalaResult_Err, "a removal of a Uuid that does not read is a failure");
    if (removed.tag == RorolalaResult_Err) {
        const RolaWriteError *error = removed.payload;
        want(error->tag == RolaWriteError_InvalidPath, "it is reported as InvalidPath");
        free_rola_write_error(removed.payload);
    }

    return finish();
}
