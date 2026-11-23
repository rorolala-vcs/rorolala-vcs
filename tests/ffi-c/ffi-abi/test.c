/*
 * What the header states about itself: the version, the layout it pins, and the one call a
 * caller can make with nothing.
 *
 * The size and offset checks repeat the header's own assertions at run time, so a library built
 * against another layout is caught even when this program was compiled against the right one.
 */

#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>

#include "rorolala_ffi.h"

#include "harness.h"

int main(void) {
    /* One number in the sources, read at compile time and at run time. */
    want(ROLA_ABI_VERSION == ROROLALA_ABI_VERSION,
         "the library and the header state one ABI version");
    want(ROLA_ABI_VERSION / 10000 == ROROLALA_ABI_VERSION_MAJOR,
         "the leading digits are the major number");
    want(ROLA_ABI_VERSION / 100 % 100 == ROROLALA_ABI_VERSION_MINOR,
         "the middle two digits are the minor number");
    want(ROLA_ABI_VERSION % 100 == ROROLALA_ABI_VERSION_PATCH,
         "the last two digits are the patch number");

    /* The result type every fallible call hands back. */
    want(sizeof(RorolalaResult) == ROROLALA_ABI_SIZE_ROROLALARESULT,
         "RorolalaResult keeps the size the header states");
    want(offsetof(RorolalaResult, tag) == ROROLALA_ABI_OFFSET_ROROLALARESULT_TAG,
         "the result tag stays at the front");
    want(offsetof(RorolalaResult, payload) == ROROLALA_ABI_OFFSET_ROROLALARESULT_PAYLOAD,
         "the result payload keeps its place");

    /* Releasing nothing is allowed, and does nothing. */
    free_string(NULL);

    return finish();
}
