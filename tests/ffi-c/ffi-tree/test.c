/*
 * The tree's structure across the ABI: bytes as hex in, entries as JSON out.
 *
 * What a caller leans on. The structure format is the crate's own, so a caller hands the bytes over
 * as text rather than reading them itself; text that is not hex or bytes that are not a structure
 * answer as JSON `null`, and the answer is always text the caller frees.
 */

#define _POSIX_C_SOURCE 200809L

#include <string.h>

#include "rorolala_ffi.h"

#include "harness.h"

int main(void) {
    char *not_hex = rola_tree_entries_json("not hex");
    want(not_hex != NULL, "an answer comes back as text");
    want(strcmp(not_hex, "null") == 0, "text that is not hex reads as null");
    free_string(not_hex);

    char *odd = rola_tree_entries_json("abc");
    want(strcmp(odd, "null") == 0, "an odd number of digits reads as null");
    free_string(odd);

    char *not_a_structure = rola_tree_entries_json("00010203");
    want(strcmp(not_a_structure, "null") == 0, "bytes that are not a structure read as null");
    free_string(not_a_structure);

    return finish();
}
