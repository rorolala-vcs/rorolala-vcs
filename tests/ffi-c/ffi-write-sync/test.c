/*
 * A sync run across the ABI: a directory no Workspace is in is a failure.
 *
 * The run has an account, a key and an address, but no Workspace to work in, so the answer is the
 * tag-and-payload failure rather than a JSON report. The payload is released the way every write
 * failure is.
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
    remove_tree(dir);

    return finish();
}
