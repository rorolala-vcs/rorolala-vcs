/*
 * A content fetch across the ABI: the shapes it is handed and the failure it
 * answers with.
 *
 * A request that names no Workspace is the missing-Workspace failure and one
 * naming a directory no Workspace holds is the same, so both are the
 * tag-and-payload answer a C caller branches on, and the payload is released
 * the way every write failure is.
 */

#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#include "rorolala_ffi.h"

#include "harness.h"

static void release_answer(RorolalaResult answer) {
  if (answer.tag == RorolalaResult_Ok) {
    free_rola_write_fetch(answer.payload);
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

  RolaWriteFetchRequest empty;
  memset(&empty, 0, sizeof empty);

  RorolalaResult refused = rola_write_fetch(&empty, progress, cancellation);
  want(refused.tag == RorolalaResult_Err, "a request with no Workspace is refused");
  want(refused.payload != NULL, "the refusal carries an error");
  if (refused.tag == RorolalaResult_Err && refused.payload != NULL) {
    RolaWriteError *error = refused.payload;
    want(error->tag == RolaWriteError_NoWorkspace,
         "the refusal is the missing Workspace");
  }
  release_answer(refused);

  const char *words[] = {"a.psd"};
  RolaWriteFetchRequest request;
  memset(&request, 0, sizeof request);
  request.common.workspace = dir;
  request.common.account = "alice";
  request.common.target.tag = RolaWriteTarget_Named;
  request.common.target.payload.Named.name = "origin";
  request.layout = "truth@origin";
  request.words.items = words;
  request.words.count = 1;

  RorolalaResult outside = rola_write_fetch(&request, progress, cancellation);
  want(outside.tag == RorolalaResult_Err, "a run outside a Workspace fails");
  want(outside.payload != NULL, "the failure carries an error");
  if (outside.tag == RorolalaResult_Err && outside.payload != NULL) {
    RolaWriteError *error = outside.payload;
    want(error->tag == RolaWriteError_NoWorkspace,
         "the failure is the missing Workspace");
  }
  release_answer(outside);

  free_rola_progress(progress);
  free_rola_cancellation(cancellation);
  remove_tree(dir);

  return finish();
}
