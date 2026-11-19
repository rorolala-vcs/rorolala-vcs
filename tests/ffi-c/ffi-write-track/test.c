/*
 * Tracking one file across the ABI: a Workspace records a version and answers with its names.
 *
 * The file is inside the Workspace, so the run needs no network: the store, the index and the Layout
 * are all here. The answer is the JSON a caller reads, released the way every write success is.
 */

#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
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
    snprintf(dir, sizeof dir, "%s/rola-ffi-track-%ld", tmp, (long)getpid());
    remove_tree(dir);
#ifdef _WIN32
    if (mkdir(dir) != 0) {
#else
    if (mkdir(dir, 0700) != 0) {
#endif
        perror("mkdir");
        return 2;
    }

    RorolalaResult nowhere = rola_write_track_file(
        dir,
        "a.txt",
        "alice",
        "first",
        "",
        "",
        ""
    );
    want(nowhere.tag == RorolalaResult_Err, "tracking outside a Workspace fails");
    free_rola_write_error(nowhere.payload);

    RorolalaResult made = create_workspace(dir);
    want(made.tag == RorolalaResult_Ok, "a Workspace is made");

    char path[512];
    snprintf(path, sizeof path, "%s/a.txt", dir);
    FILE *file = fopen(path, "wb");
    want(file != NULL, "the file is made");
    if (file != NULL) {
        fputs("one", file);
        fclose(file);
    }

    RorolalaResult tracked = rola_write_track_file(
        dir,
        "a.txt",
        "alice",
        "first",
        "",
        "",
        ""
    );
    want(tracked.tag == RorolalaResult_Ok, "tracking a file succeeds");
    want(tracked.payload != NULL, "the answer is the version's names");
    if (tracked.tag == RorolalaResult_Ok && tracked.payload != NULL) {
        const char *json = (const char *)tracked.payload;
        want(strstr(json, "\"id\"") != NULL, "the answer names the entry");
        want(strstr(json, "\"version\"") != NULL, "the answer names the version");
    }

    free_string(tracked.payload);
    remove_tree(dir);

    return finish();
}
