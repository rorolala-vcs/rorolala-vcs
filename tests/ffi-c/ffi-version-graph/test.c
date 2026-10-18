/*
 * The version graph across the ABI: a file with no history is answered with
 * nothing.
 *
 * The chain itself is read from the index by the status module and is checked
 * there, where a chain can be written without an ABI for each object. What is
 * met here is the crossing a caller actually leans on: a Workspace that records
 * no version for the path asks for its graph and gets NULL rather than an empty
 * chain.
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

  char dir[512];
  snprintf(dir, sizeof dir, "%s/rola-ffi-graph-XXXXXX", tmp);
  if (mkdtemp(dir) == NULL) {
    perror("mkdtemp");
    return 2;
  }

  RorolalaResult made = create_workspace(dir);
  want(made.tag == RorolalaResult_Ok,
       "create_workspace succeeds on a fresh directory");

  char file[512];
  snprintf(file, sizeof file, "%s/nothing.psd", dir);

  want(locate_rola_version_graph(dir, file) == NULL,
       "a file no version was recorded for has no graph");

  remove_tree(dir);

  return finish();
}
