/*
 * The graphs across the ABI: a file with no history is answered with nothing,
 * and a Workspace whose index holds no variant draws an empty variant graph.
 *
 * The chains themselves are read from the index by the status module and are
 * checked there, where a chain can be written without an ABI for each object.
 * What is met here is the crossing a caller actually leans on: a Workspace that
 * records no version for the path asks for its graph and gets NULL rather than
 * an empty chain, and the variant graph takes a filled window and answers a
 * handle carrying its counts.
 */

#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "rorolala_ffi.h"

#include "harness.h"

int main(void) {
  const char *tmp = getenv("TMPDIR");
  if (tmp == NULL) {
    tmp = "/tmp";
  }

  char dir[512];
  snprintf(dir, sizeof dir, "%s/rola-ffi-graph-XXXXXX", tmp);
  if (mkdtemp(dir) == NULL) {
    perror("mkdtemp");
    return 2;
  }

  RorolalaResult made = rola_workspace_create(dir);
  want(made.tag == RorolalaResult_Ok,
       "rola_workspace_create succeeds on a fresh directory");

  /* `dir` may hold a full buffer, so the file name needs room for all of it and the suffix. */
  char file[sizeof dir + sizeof "/nothing.psd"];
  snprintf(file, sizeof file, "%s/nothing.psd", dir);

  want(rola_version_graph_locate(dir, file) == NULL,
       "a file no version was recorded for has no graph");

  /* The variant graph is a window the caller fills, centred on no entry: the
   * work's own entries are what it stands on, and this Workspace has none. */
  RolaVariantGraphWindow window;
  memset(&window, 0, sizeof window);
  window.back = 64;
  window.joins = true;
  window.branches = true;
  window.max_nodes = 400;

  RorolalaResult drawn = rola_variant_graph_locate(dir, &window, NULL, 0);
  want(drawn.tag == RorolalaResult_Ok, "an empty index still draws a graph");

  if (drawn.tag == RorolalaResult_Ok) {
    RolaVariantGraph *graph = drawn.payload;
    want(graph != NULL, "the variant graph is a handle");

    if (graph != NULL) {
      want(rola_variant_graph_focus_count(graph) == 0,
           "a window centred on nothing focuses nothing");
      want(rola_variant_graph_node_count(graph) == 0,
           "an empty index draws no variant");
      want(!rola_variant_graph_truncated(graph),
           "a window that was walked out is not truncated");

      free_rola_variant_graph(graph);
    }
  } else {
    free_rola_variant_graph_error(drawn.payload);
  }

  remove_tree(dir);

  return finish();
}
