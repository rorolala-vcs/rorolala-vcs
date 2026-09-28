/*
 * The Workspace across the ABI: made, located, and locked.
 *
 * Three behaviours a C caller leans on. Making one answers in the tag-and-payload shape, and a
 * directory a Workspace cannot be made at is one of the failure's. Locating one hands out an owned
 * handle, and a directory no Workspace is inside answers with nothing rather than with a handle to
 * nowhere. Locking one hands out an opaque guard that holds the lock for as long as C keeps it, so
 * the lock is read back through the handle and given back by dropping the guard.
 */

#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <sys/stat.h>
#include <unistd.h>

#include "rorolala_ffi.h"

#include "harness.h"

int main(void) {
    const char *tmp = getenv("TMPDIR");
    if (tmp == NULL) {
        tmp = "/tmp";
    }

    /* A directory of this run's own to make a Workspace in, and one that is nothing. */
    char dir[512];
    snprintf(dir, sizeof dir, "%s/rola-ffi-ws-XXXXXX", tmp);
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

    /* Making one succeeds, carrying nothing. */
    RorolalaResult made = create_workspace(dir);
    want(made.tag == RorolalaResult_Ok, "create_workspace succeeds on a fresh directory");
    want(made.payload == NULL, "create_workspace hands back nothing on success");

    /* It is a Workspace, and a directory that is not one answers with nothing. */
    RolaWorkspace *workspace = locate_rola_workspace(dir);
    want(workspace != NULL, "locate_rola_workspace finds the Workspace just made");
    want(locate_rola_workspace(plain) == NULL, "locate_rola_workspace answers with nothing elsewhere");

    if (workspace != NULL) {
        /* The guard holds the lock, and the lock is read back through the handle. */
        want(!RolaWorkspace_is_locking(workspace), "a Workspace is unlocked to begin with");

        RolaWorkspaceLocking *guard = RolaWorkspace_get_locking_guard(workspace);
        want(guard != NULL, "the lock is handed out as a guard");
        want(RolaWorkspace_is_locking(workspace), "holding the guard is holding the lock");

        RolaWorkspace_drop_locking_guard(guard);
        want(!RolaWorkspace_is_locking(workspace), "giving the guard back gives the lock back");

        free_rola_workspace(workspace);
    }

    remove_tree(dir);
    remove_tree(plain);

    return finish();
}
