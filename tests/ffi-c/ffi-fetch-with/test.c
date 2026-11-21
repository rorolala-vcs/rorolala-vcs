/*
 * A content fetch across the ABI: the shapes it is handed and the failure it
 * answers with.
 *
 * A request that reads as no JSON is a JSON failure, and a request naming a
 * directory no Workspace holds is the missing-Workspace failure. Both are the
 * tag-and-payload answer a C caller branches on, and the payload is released
 * the way every write failure is.
 */

#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <sys/stat.h>
#include <unistd.h>

#include "rorolala_ffi.h"

#include "harness.h"

static void release_answer(RorolalaResult answer) {
  if (answer.tag == RorolalaResult_Ok) {
    free_string(answer.payload);
  } else {
    free_rola_write_error(answer.payload);
  }
}

int main(void) {
  const char *tmp = getenv("TMPDIR");
  if (tmp == NULL) {
    tmp = "/tmp";
  }

  char dir[512];
  snprintf(dir, sizeof dir, "%s/rola-ffi-fetch-with-%ld", tmp, (long)getpid());
  remove_tree(dir);
#ifdef _WIN32
  if (mkdir(dir) != 0) {
#else
  if (mkdir(dir, 0700) != 0) {
#endif
    perror("mkdir");
    return 2;
  }

  RolaProgress *progress = rola_progress_new();
  RolaCancellation *cancellation = rola_cancellation_new();
  want(progress != NULL, "a progress handle is made");
  want(cancellation != NULL, "a cancellation handle is made");
  if (progress == NULL || cancellation == NULL) {
    free_rola_progress(progress);
    free_rola_cancellation(cancellation);
    remove_tree(dir);
    return finish();
  }

  RorolalaResult empty = rola_write_fetch_with("", progress, cancellation);
  want(empty.tag == RorolalaResult_Err, "an empty request is refused");
  want(empty.payload != NULL, "the refusal carries an error");
  if (empty.tag == RorolalaResult_Err && empty.payload != NULL) {
    RolaWriteError *error = empty.payload;
    want(error->tag == RolaWriteError_Json,
         "an empty request is a JSON failure");
  }
  release_answer(empty);

  char request[1024];
  snprintf(
      request, sizeof request,
      "{\"workspace\":\"%s\",\"account\":\"alice\",\"target\":{\"kind\":"
      "\"named\",\"name\":\"origin\"},"
      "\"layout\":\"truth@origin\",\"words\":[\"a.psd\"],\"recursive\":false}",
      dir);

  RorolalaResult refused =
      rola_write_fetch_with(request, progress, cancellation);
  want(refused.tag == RorolalaResult_Err, "a run outside a Workspace fails");
  want(refused.payload != NULL, "the failure carries an error");
  if (refused.tag == RorolalaResult_Err && refused.payload != NULL) {
    RolaWriteError *error = refused.payload;
    want(error->tag == RolaWriteError_NoWorkspace,
         "the failure is the missing Workspace");
  }
  release_answer(refused);

  free_rola_progress(progress);
  free_rola_cancellation(cancellation);
  remove_tree(dir);

  return finish();
}
