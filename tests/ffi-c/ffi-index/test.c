/*
 * The index across the ABI: located, and the inverse index rebuilt over it.
 *
 * An index is reached the way a Vault and a Workspace are (an owned handle, or nothing) and the
 * inverse index of one is reached from a directory in the same way. What is worth meeting here is
 * the other shape of a fallible export: a rebuild answers with a report rather than with nothing,
 * so the payload is a struct the caller reads through accessors and frees itself.
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
    snprintf(dir, sizeof dir, "%s/rola-ffi-idx-XXXXXX", tmp);
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

    /* A Workspace keeps an index of its own, which is what there is to locate. */
    RorolalaResult made = rola_workspace_create(dir);
    want(made.tag == RorolalaResult_Ok, "rola_workspace_create succeeds on a fresh directory");

    RolaVCSIndex *index = rola_vcs_index_locate(dir);
    want(index != NULL, "rola_vcs_index_locate finds the Workspace's index");
    want(rola_vcs_index_locate(plain) == NULL, "rola_vcs_index_locate answers with nothing elsewhere");
    if (index != NULL) {
        /* An index with nothing in it lists three empty sets and has no root version yet. */
        RolaVCSIndexListing *listing = rola_vcs_index_list(index);
        want(listing != NULL, "an empty index lists");
        if (listing != NULL) {
            want(rola_vcs_index_listing_version_count(listing) == 0,
                 "an empty index lists no versions");
            want(rola_vcs_index_listing_variant_count(listing) == 0,
                 "an empty index lists no variants");
            want(rola_vcs_index_listing_string_count(listing) == 0,
                 "an empty index lists no text objects");

            char *none = rola_vcs_index_listing_string_at(listing, 0);
            want(none != NULL && none[0] == '\0', "past the end there is no text hash");
            free_string(none);

            free_rola_vcs_index_listing(listing);
        }

        char *root = rola_vcs_index_root_version(index);
        want(root != NULL && strcmp(root, "null") == 0,
             "an index with nothing in it has no root version");
        free_string(root);

        /* Naming a key nothing is stored under is a failure the caller frees. */
        RorolalaResult object = rola_vcs_index_object_view(
            index,
            "blake3:2222222222222222222222222222222222222222222222222222222222222222"
        );
        want(object.tag == RorolalaResult_Err, "a key nothing is stored under is a failure");
        if (object.tag == RorolalaResult_Err && object.payload != NULL) {
            free_rola_vcs_index_reading_error(object.payload);
        }

        free_rola_vcs_index(index);
    }

    /* The inverse index is reached the same way, and rebuilt over the objects it holds. */
    RolaInverseIndex *inverse = rola_inverse_index_locate(dir);
    want(inverse != NULL, "rola_inverse_index_locate finds the inverse index of the Workspace");

    if (inverse != NULL) {
        RorolalaResult rebuilt = rola_inverse_index_rebuild(inverse);
        want(rebuilt.tag == RorolalaResult_Ok, "an index with nothing in it rebuilds");

        if (rebuilt.tag == RorolalaResult_Ok) {
            RolaInverseIndexReport *report = rebuilt.payload;
            want(report != NULL, "the rebuild answers with a report rather than with nothing");
            if (report != NULL) {
                want(rola_inverse_index_report_objects(report) == 0, "an empty index holds no objects");

                char *fingerprint = rola_inverse_index_report_fingerprint(report);
                want(fingerprint != NULL, "the report names the records by a digest");
                free_string(fingerprint);

                free_rola_inverse_index_report(report);
            }
        } else if (rebuilt.payload != NULL) {
            free_rola_inverse_index_error(rebuilt.payload);
        }

        /* The dependents of a key are a list read by position, and an empty index has none. */
        RorolalaResult dependents = rola_inverse_index_store_dependents(
            inverse,
            "blake3:3333333333333333333333333333333333333333333333333333333333333333"
        );
        want(dependents.tag == RorolalaResult_Ok, "an empty index has an answer for any key");
        if (dependents.tag == RorolalaResult_Ok) {
            RolaHashList *hashes = dependents.payload;
            want(hashes != NULL, "the dependents cross as a list");
            if (hashes != NULL) {
                want(rola_hash_list_count(hashes) == 0, "an empty index has no dependents");

                char *first = rola_hash_list_at(hashes, 0);
                want(first != NULL && first[0] == '\0', "past the end there is no hash");
                free_string(first);

                free_rola_hash_list(hashes);
            }
        } else if (dependents.payload != NULL) {
            free_rola_inverse_index_reading_error(dependents.payload);
        }

        free_rola_inverse_index(inverse);
    }

    remove_tree(dir);
    remove_tree(plain);

    return finish();
}
