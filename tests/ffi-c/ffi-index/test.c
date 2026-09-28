/*
 * The index across the ABI: located, and the inverse index rebuilt over it.
 *
 * An index is reached the way a Vault and a Workspace are — an owned handle, or nothing — and the
 * inverse index of one is reached from a directory in the same way. What is worth meeting here is
 * the other shape of a fallible export: a rebuild answers with a report rather than with nothing,
 * so the payload is a struct the caller reads through accessors and frees itself.
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
    RorolalaResult made = create_workspace(dir);
    want(made.tag == RorolalaResult_Ok, "create_workspace succeeds on a fresh directory");

    RolaVCSIndex *index = locate_rola_vcs_index(dir);
    want(index != NULL, "locate_rola_vcs_index finds the Workspace's index");
    want(locate_rola_vcs_index(plain) == NULL, "locate_rola_vcs_index answers with nothing elsewhere");
    if (index != NULL) {
        free_rola_vcsindex(index);
    }

    /* The inverse index is reached the same way, and rebuilt over the objects it holds. */
    RolaInverseIndex *inverse = locate_rola_inverse_index(dir);
    want(inverse != NULL, "locate_rola_inverse_index finds the inverse index of the Workspace");

    if (inverse != NULL) {
        RorolalaResult rebuilt = rebuild_rola_inverse_index(inverse);
        want(rebuilt.tag == RorolalaResult_Ok, "an index with nothing in it rebuilds");

        if (rebuilt.tag == RorolalaResult_Ok) {
            RolaInverseIndexReport *report = rebuilt.payload;
            want(report != NULL, "the rebuild answers with a report rather than with nothing");
            if (report != NULL) {
                want(inverse_index_report_objects(report) == 0, "an empty index holds no objects");

                char *fingerprint = inverse_index_report_fingerprint(report);
                want(fingerprint != NULL, "the report names the records by a digest");
                free_string(fingerprint);

                free_rola_inverse_index_report(report);
            }
        } else if (rebuilt.payload != NULL) {
            free_rola_inverse_index_error(rebuilt.payload);
        }

        free_rola_inverse_index(inverse);
    }

    remove_tree(dir);
    remove_tree(plain);

    return finish();
}
