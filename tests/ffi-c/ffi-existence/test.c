/*
 * The existence reading across the ABI: whether a version, a variant or a Layout
 * entry is stored here, answered as JSON in the order asked, and a request that
 * will not read as a tagged failure.
 *
 * The reading itself is checked in Rust, where a store can be written without an
 * ABI for each object. What is met here is the crossing a caller leans on: the
 * answer is one JSON document released with `free_string`, the failure is a
 * `RolaExistenceError *` released with its own `free_*`, and both exports answer
 * even when what they were asked about is not there.
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

  RorolalaResult made = create_workspace(dir);
  want(made.tag == RorolalaResult_Ok,
       "create_workspace succeeds on a fresh directory");

  /* A whole hash the index does not hold is answered rather than refused: the
   * content is not here, which is the answer the run asked for. */
  const char *absent =
      "{\"targets\":[{\"kind\":\"version\",\"hash\":\""
      "0000000000000000000000000000000000000000000000000000000000000000"
      "\"}]}";

  RorolalaResult answered = rola_can_retrack_offline_json(dir, absent);
  want(answered.tag == RorolalaResult_Ok, "a hash nothing holds is answered");

  if (answered.tag == RorolalaResult_Ok) {
    char *json = answered.payload;
    want(json != NULL, "the answer is a JSON document");
    want(json != NULL && strstr(json, "\"offline\":false") != NULL,
         "the answer says the content is not here");
    want(json != NULL && strstr(json, "\"kind\":\"version\"") != NULL,
         "the answer names the kind it was asked about");
    free_string(json);
  }

  /* The answers keep the order the targets were written in. */
  const char *two =
      "{\"targets\":[{\"kind\":\"version\",\"hash\":\""
      "0000000000000000000000000000000000000000000000000000000000000000"
      "\"},{\"kind\":\"entry\",\"path\":\"nothing.psd\"}]}";

  RorolalaResult ordered = rola_can_retrack_offline_json(dir, two);
  want(ordered.tag == RorolalaResult_Ok, "a request of two targets is answered");

  if (ordered.tag == RorolalaResult_Ok) {
    char *json = ordered.payload;
    char *version = json == NULL ? NULL : strstr(json, "\"kind\":\"version\"");
    char *entry = json == NULL ? NULL : strstr(json, "\"kind\":\"entry\"");

    want(version != NULL && entry != NULL && version < entry,
         "the answers keep the order the targets were written in");
    free_string(json);
  }

  /* A request that does not read is a failure the caller releases by its tag. */
  RorolalaResult refused =
      rola_can_retrack_offline_json(dir, "{\"targets\":[{\"kind\":\"nope\"}]}");
  want(refused.tag == RorolalaResult_Err,
       "a request that does not read is refused");

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
  RorolalaResult searched = rola_search_source_json(dir, "{\"targets\":[]}");
  want(searched.payload != NULL, "a search carries an answer or a failure");

  if (searched.tag == RorolalaResult_Ok) {
    free_string(searched.payload);
  } else {
    free_rola_existence_error(searched.payload);
  }

  remove_tree(dir);

  return finish();
}
