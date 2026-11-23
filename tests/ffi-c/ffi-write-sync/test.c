/*
 * A sync run across the ABI: a directory no Workspace is in is a failure.
 *
 * The run has an account, a key and an address, but no Workspace to work in, so the answer is the
 * tag-and-payload failure rather than a typed report. The payload is released the way every write
 * failure is, and a run that succeeds answers with a handle of its own.
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

    char dir[512];
    snprintf(dir, sizeof dir, "%s/rola-ffi-sync-%ld", tmp, (long)getpid());
    remove_tree(dir);
#ifdef _WIN32
    if (mkdir(dir) != 0) {
#else
    if (mkdir(dir, 0700) != 0) {
#endif
        perror("mkdir");
        return 2;
    }

    RorolalaResult answer = rola_write_sync(
        dir,
        "vault",
        "alice",
        "alice.pem",
        "rola://127.0.0.1/",
        "both",
        false,
        true,
        true,
        true
    );
    want(answer.tag == RorolalaResult_Err, "a sync run outside a Workspace fails");
    want(answer.payload != NULL, "the failure carries an error");

    free_rola_write_error(answer.payload);

    RorolalaResult made = rola_workspace_create(dir);
    want(made.tag == RorolalaResult_Ok, "a Workspace is made");

    RorolalaResult nameless = rola_write_sync(
        dir,
        "vault",
        "alice",
        "missing.pem",
        "",
        "both",
        false,
        false,
        false,
        false
    );
    want(nameless.tag == RorolalaResult_Err, "an empty address is refused");
    free_rola_write_error(nameless.payload);

    RorolalaResult empty = rola_write_sync(
        dir,
        "vault",
        "alice",
        "missing.pem",
        "rola://127.0.0.1/",
        "both",
        false,
        false,
        false,
        false
    );
    want(empty.tag == RorolalaResult_Ok, "a run with every side left alone succeeds");
    want(empty.payload != NULL, "the run answers with its numbers");
    if (empty.tag == RorolalaResult_Ok && empty.payload != NULL) {
        RolaWriteSync *report = empty.payload;
        want(rola_write_sync_applied(report) == 0, "nothing was applied");
        want(rola_write_sync_received(report) == 0, "nothing was received");
        free_rola_write_sync(report);
    }

    remove_tree(dir);

    return finish();
}
