/*
 * What every `ffi-c` module's `test.c` is built on.
 *
 * A module is a C11 program of its own that uses the generated header and nothing else, so what it
 * needs said is how a check is made and how the program ends on them. The counters and the checks
 * are `static`, so a module that includes this once has them to itself.
 *
 * A `test.c` includes `_POSIX_C_SOURCE` itself before its first system header, and includes this
 * after them.
 */

#ifndef ROROLALA_FFI_C_HARNESS_H
#define ROROLALA_FFI_C_HARNESS_H

#include <stdio.h>
#include <stdlib.h>

/* How many checks have failed so far. */
static int failed = 0;

/* Counts one check, naming it when it did not hold. */
static inline void want(int held, const char *what) {
    if (!held) {
        fprintf(stderr, "FAIL: %s\n", what);
        failed++;
    }
}

/* Takes a directory away again, whole. */
static inline void remove_tree(const char *path) {
    char command[1024];
    snprintf(command, sizeof command, "rm -rf '%s'", path);

    if (system(command) != 0) {
        fprintf(stderr, "FAIL: could not remove %s\n", path);
        failed++;
    }
}

/* Ends the program on the checks made so far: nothing said, and nothing failed. */
static inline int finish(void) {
    if (failed != 0) {
        fprintf(stderr, "%d checks answered as they should not\n", failed);
        return 1;
    }

    printf("answered as it should\n");
    return 0;
}

#endif
