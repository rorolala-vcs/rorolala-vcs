/*
 * The Layout across the ABI: what a place holds, what an entry says, and the names a place has no
 * Layout under.
 *
 * The fixtures are made by the runner: a Workspace whose Layout names a file under art/ and one the
 * Vault has deprecated, and a Vault whose own Layout names one file. What a caller leans on is the
 * JSON the commands already write, and the four failures a name can come to.
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
    char *layouts = rola_layouts_json(workspace);
    want(layouts != NULL, "the Layouts answer as text");
    want(layouts != NULL && strcmp(layouts, "null") != 0, "a Workspace holds a Layout");
    want(layouts != NULL && strstr(layouts, "\"name\":\"main\"") != NULL,
         "a Workspace's Layout is named main");
    want(layouts != NULL && strstr(layouts, "\"is_current\":true") != NULL,
         "the one Layout is the current one");
    free_string(layouts);

    /* A Vault keeps one of its own, under the name a Vault's Layout goes by. */
    char *vault_layouts = rola_layouts_json(vault);
    want(vault_layouts != NULL && strstr(vault_layouts, "\"name\":\"truth\"") != NULL,
         "a Vault's Layout is named truth");
    free_string(vault_layouts);

    /* A directory no place holds has no Layout, which is the JSON that says so. */
    char *nowhere = rola_layouts_json("/nonexistent/rola/layout");
    want(nowhere != NULL && strcmp(nowhere, "null") == 0,
         "a directory outside every place reads as null");
    free_string(nowhere);

    /* An unnamed Layout is the one being worked in, and its entries carry what each says. */
    RorolalaResult entries = rola_layout_entries_json(workspace, NULL);
    want(entries.tag == RorolalaResult_Ok, "the working Layout's entries answer");
    if (entries.tag == RorolalaResult_Ok) {
        const char *json = entries.payload;
        want(json != NULL, "the entries answer as text");
        want(json != NULL && strstr(json, "\"path\":\"art/hero.psd\"") != NULL,
             "the named entry is listed at its path");
        want(json != NULL && strstr(json, "\"owner\":\"alice\"") != NULL,
             "the entry carries its owner");
        want(json != NULL && strstr(json, "\"description\":\"the hero\"") != NULL,
             "the entry carries what it says about itself");
        want(json != NULL && strstr(json, "\"deprecated\":false") != NULL,
             "an entry in the work is not deprecated");
        want(json != NULL && strstr(json, "\"path\":\"@/removed/old.psd\"") != NULL,
             "a deprecated entry keeps the path the Vault moved it to");
        want(json != NULL && strstr(json, "\"deprecated\":true") != NULL,
             "the entry under @/removed/ is deprecated");
        free_string(entries.payload);
    }

    /* The Layout a Workspace works in can be named as well as left out. */
    RorolalaResult named = rola_layout_entries_json(workspace, "main");
    want(named.tag == RorolalaResult_Ok, "the working Layout answers when named");
    if (named.tag == RorolalaResult_Ok) {
        free_string(named.payload);
    }

    /* A Vault's own Layout is read from a Vault, under its own name. */
    RorolalaResult vault_entries = rola_layout_entries_json(vault, "truth");
    want(vault_entries.tag == RorolalaResult_Ok, "a Vault's own Layout answers");
    if (vault_entries.tag == RorolalaResult_Ok) {
        const char *json = vault_entries.payload;
        want(json != NULL && strstr(json, "\"path\":\"shared.psd\"") != NULL,
             "the Vault's entry is listed");
        free_string(vault_entries.payload);
    }

    /* A name the place has no Layout under is missing, not an empty Layout. */
    RorolalaResult missing = rola_layout_entries_json(workspace, "nowhere");
    want(missing.tag == RorolalaResult_Err, "a name no place has is a failure");
    if (missing.tag == RorolalaResult_Err) {
        const RolaLayoutError *error = missing.payload;
        want(error->tag == RolaLayoutError_Missing, "it is reported as Missing");
        free_rola_layout_error(missing.payload);
    }

    /* A copy that was never fetched is told apart from one that is there. */
    RorolalaResult not_cached = rola_layout_entries_json(workspace, "truth@studio");
    want(not_cached.tag == RorolalaResult_Err, "an unfetched copy is a failure");
    if (not_cached.tag == RorolalaResult_Err) {
        const RolaLayoutError *error = not_cached.payload;
        want(error->tag == RolaLayoutError_NotCached, "it is reported as NotCached");
        free_rola_layout_error(not_cached.payload);
    }

    /* A directory no place holds cannot name any Layout at all. */
    RorolalaResult no_place = rola_layout_entries_json("/nonexistent/rola/layout", NULL);
    want(no_place.tag == RorolalaResult_Err, "a directory outside every place is a failure");
    if (no_place.tag == RorolalaResult_Err) {
        const RolaLayoutError *error = no_place.payload;
        want(error->tag == RolaLayoutError_NoWorkspace, "it is reported as NoWorkspace");
        free_rola_layout_error(no_place.payload);
    }

    /* A Workspace's configuration lists the Vaults it knows and the one it reaches for. */
    char *config = rola_workspace_config_json(workspace);
    want(config != NULL && strcmp(config, "null") != 0, "a Workspace has a configuration");
    want(config != NULL && strstr(config, "\"name\":\"studio\"") != NULL,
         "a bound Vault is listed by name");
    want(config != NULL && strstr(config, "\"address\":\"tcp://127.0.0.1:9270\"") != NULL,
         "a binding carries its address");
    want(config != NULL && strstr(config, "\"is_default\":true") != NULL,
         "the Vault the Workspace reaches for is marked");
    want(config != NULL && strstr(config, "\"default\":\"studio\"") != NULL,
         "the configuration names the Vault it reaches for");
    free_string(config);

    /* A Vault's configuration says what it is and who may administer it. */
    char *vault_config = rola_vault_config_json(vault);
    want(vault_config != NULL && strcmp(vault_config, "null") != 0, "a Vault has a configuration");
    want(vault_config != NULL && strstr(vault_config, "\"description\":\"New Rola Vault\"") != NULL,
         "a Vault says what it is");
    want(vault_config != NULL && strstr(vault_config, "\"admins\":[]") != NULL,
         "a new Vault has no administrator");
    want(vault_config != NULL && strstr(vault_config, "\"max_concurrency\":") != NULL,
         "a Vault carries what its daemon may serve at once");
    free_string(vault_config);

    /* A directory no Vault holds has no configuration, which is the JSON that says so. */
    char *no_config = rola_vault_config_json("/nonexistent/rola/vault");
    want(no_config != NULL && strcmp(no_config, "null") == 0,
         "a directory outside every Vault reads as null");
    free_string(no_config);

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
