/*
 * The existence reading across the ABI: whether a version, a variant or a Layout
 * entry is stored here, answered as a typed handle in the order asked, and a
 * target that names no key answered as a tagged failure.
 *
 * The reading itself is checked in Rust, where a store can be written without an
 * ABI for each object. What is met here is the crossing a caller leans on: the
 * targets are an array the caller fills, the answer is a handle read one target
 * at a time, the failure is a `RolaExistenceError *` released with its own
 * `free_*`, and both exports answer even when what they were asked about is not
 * there.
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
  snprintf(dir, sizeof dir, "%s/rola-ffi-existence-XXXXXX", tmp);
  if (mkdtemp(dir) == NULL) {
    perror("mkdtemp");
    return 2;
  }

  RorolalaResult made = rola_workspace_create(dir);
  want(made.tag == RorolalaResult_Ok,
       "rola_workspace_create succeeds on a fresh directory");

  /* A whole hash the index does not hold is answered rather than refused: the
   * content is not here, which is the answer the run asked for. */
  char absent[] =
      "0000000000000000000000000000000000000000000000000000000000000000";

  RolaSearchTarget one[1];
  memset(one, 0, sizeof one);
  one[0].tag = RolaSearchTarget_Version;
  one[0].payload.Version.hash = absent;

  RorolalaResult answered = rola_can_retrack_offline(dir, one, 1);
  want(answered.tag == RorolalaResult_Ok, "a hash nothing holds is answered");

  if (answered.tag == RorolalaResult_Ok) {
    RolaExistenceTargets *targets = answered.payload;
    want(targets != NULL, "the answer is a handle");

    if (targets != NULL) {
      want(rola_existence_targets_count(targets) == 1,
           "one target was answered");

      RolaExistenceTarget *target = rola_existence_targets_at(targets, 0);
      want(target != NULL, "the answer names the target it was asked about");

      if (target != NULL) {
        char *kind = rola_existence_target_kind(target);
        want(kind != NULL && strcmp(kind, "version") == 0,
             "the answer names the kind it was asked about");
        free_string(kind);

        want(!rola_existence_target_offline(target),
             "the answer says the content is not here");

        free_rola_existence_target(target);
      }

      free_rola_existence_targets(targets);
    }
  }

  /* The answers keep the order the targets were written in. */
  RolaSearchTarget two[2];
  memset(two, 0, sizeof two);
  two[0].tag = RolaSearchTarget_Version;
  two[0].payload.Version.hash = absent;
  two[1].tag = RolaSearchTarget_Entry;
  two[1].payload.Entry.path = "nothing.psd";

  RorolalaResult ordered = rola_can_retrack_offline(dir, two, 2);
  want(ordered.tag == RorolalaResult_Ok, "a request of two targets is answered");

  if (ordered.tag == RorolalaResult_Ok) {
    RolaExistenceTargets *targets = ordered.payload;
    want(targets != NULL, "the answer of two targets is a handle");

    if (targets != NULL) {
      want(rola_existence_targets_count(targets) == 2,
           "both targets were answered");

      RolaExistenceTarget *first = rola_existence_targets_at(targets, 0);
      RolaExistenceTarget *second = rola_existence_targets_at(targets, 1);

      if (first != NULL && second != NULL) {
        char *kind = rola_existence_target_kind(first);
        want(kind != NULL && strcmp(kind, "version") == 0,
             "the first answer keeps its place");
        free_string(kind);

        kind = rola_existence_target_kind(second);
        want(kind != NULL && strcmp(kind, "entry") == 0,
             "the second answer keeps its place");
        free_string(kind);

        char *path = rola_existence_target_path(second);
        want(path != NULL && strcmp(path, "nothing.psd") == 0,
             "an entry answer names the path it was asked about");
        free_string(path);
      }

      if (first != NULL) {
        free_rola_existence_target(first);
      }
      if (second != NULL) {
        free_rola_existence_target(second);
      }
      free_rola_existence_targets(targets);
    }
  }

  /* A target that names no key is a failure the caller releases by its tag. */
  RolaSearchTarget denied[1];
  memset(denied, 0, sizeof denied);
  denied[0].tag = RolaSearchTarget_Version;
  denied[0].payload.Version.hash = "not-a-key";

  RorolalaResult refused = rola_can_retrack_offline(dir, denied, 1);
  want(refused.tag == RorolalaResult_Err,
       "a target that names no key is refused");

  if (refused.tag == RorolalaResult_Err) {
    RolaExistenceError *error = refused.payload;
    want(error != NULL && error->tag == RolaExistenceError_BadRequest,
         "the refusal is the bad-request failure");
    want(error != NULL && error->payload.BadRequest.reason != NULL,
         "the refusal says why it does not read");
    free_rola_existence_error(error);
  }

  /* The search crosses the same way, and what it answers is released by its
   * tag: an answer when the run can act as an account, a failure when it
   * cannot, and both are owned by the caller. */
  RorolalaResult searched = rola_search_source(dir, NULL, 0, "");
  want(searched.payload != NULL, "a search carries an answer or a failure");

  if (searched.tag == RorolalaResult_Ok) {
    free_rola_search_sources(searched.payload);
  } else {
    free_rola_existence_error(searched.payload);
  }

  remove_tree(dir);

  return finish();
}
