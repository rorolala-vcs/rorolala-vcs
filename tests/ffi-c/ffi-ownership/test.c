/*
 * Ownership across the ABI: who holds an entry, and who the work acts as.
 *
 * What a C caller can lean on here is the absence shape, because the positive path is not reachable
 * from C: an ownership is read from the two Layouts a Workspace keeps — the one it works in and the
 * copy of the Vault's it has fetched — and no Layout is exported, so a C program can neither make
 * one nor put an entry in it. The read itself is checked on the Rust side, where a Workspace with a
 * Layout can be made.
 *
 * So what is checked here is that the symbols link and the shapes are usable from C: a directory no
 * Workspace holds answers with nothing rather than with a handle, a Workspace that works in no
 * Layout answers with nothing either, and the account the work acts as crosses as a string the
 * caller owns and releases.
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

    /* A directory that is nothing, and one made into a Workspace with no Layout in it. */
    char plain[512];
    snprintf(plain, sizeof plain, "%s/rola-ffi-owned-plain-XXXXXX", tmp);
    if (mkdtemp(plain) == NULL) {
        perror("mkdtemp");
        return 2;
    }

    char dir[512];
    snprintf(dir, sizeof dir, "%s/rola-ffi-owned-ws-XXXXXX", tmp);
    if (mkdtemp(dir) == NULL) {
        perror("mkdtemp");
        return 2;
    }

    want(
        locate_rola_ownership(plain) == NULL,
        "a directory no Workspace holds answers with nothing"
    );

    RorolalaResult made = create_workspace(dir);
    want(made.tag == RorolalaResult_Ok, "a Workspace is made for the read to be asked of");

    want(
        locate_rola_ownership(dir) == NULL,
        "a Workspace that works in no Layout answers with nothing"
    );

    /* The account is an owned string whether or not one is bound, so it is always there to release. */
    char *account = rola_current_account();
    want(account != NULL, "the account the work acts as crosses as a string");
    if (account != NULL) {
        free_string(account);
    }

    remove_tree(dir);
    remove_tree(plain);

    return finish();
}
