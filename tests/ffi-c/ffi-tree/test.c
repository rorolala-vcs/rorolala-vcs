/*
 * The tree's structure across the ABI: bytes as hex in, entries as typed handles out.
 *
 * What a caller leans on. The structure format is the crate's own, so a caller hands the bytes over
 * as text rather than reading them itself; text that is not hex or bytes that are not a structure
 * answer as no handle at all, and the entries of one are read by index and released one by one.
 */

#define _POSIX_C_SOURCE 200809L

#include <stdlib.h>
#include <string.h>

#include "rorolala_ffi.h"

#include "harness.h"

int main(void) {
    want(rola_tree_entries("not hex") == NULL, "text that is not hex has no entries");
    want(rola_tree_entries("abc") == NULL, "an odd number of digits has no entries");
    want(rola_tree_entries("00010203") == NULL, "bytes that are not a structure have no entries");

    /* A directory no Workspace holds has no tree to read. */
    want(rola_tree_status("/nonexistent/rola/tree") == NULL,
         "a directory outside a Workspace has no tree status");

    /* The mark is the cheap question a listing asks about every directory it draws. */
    const char *tree = getenv("ROLA_TREE");
    const char *workspace = getenv("ROLA_WORKSPACE");
    want(tree != NULL && rola_tree_is_tree(tree), "a directory with the mark is a tree");
    want(workspace != NULL, "the runner names the fixture Workspace");
    if (workspace != NULL) {
        want(!rola_tree_is_tree(workspace), "a Workspace's root is not a tree");
    }

    /* The id and the description are the data beside the mark, and both cross as fields. */
    RolaTreeMark *mark = rola_tree_mark(tree);
    want(mark != NULL, "a tree has a mark to read");
    if (mark != NULL) {
        char *id = rola_tree_mark_id(mark);
        want(id != NULL && strcmp(id, "tree-id") == 0, "a tree carries its id");
        free_string(id);

        want(rola_tree_mark_described(mark), "a staged description is marked as one");

        char *description = rola_tree_mark_description(mark);
        want(description != NULL && strcmp(description, "what the next version says") == 0,
             "the staged description crosses as data");
        free_string(description);

        free_rola_tree_mark(mark);
    }

    want(rola_tree_mark(workspace) == NULL,
         "a directory without the mark has no mark to read");

    return finish();
}
