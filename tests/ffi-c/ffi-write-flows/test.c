/*
 * The one-shot write workflows across the ABI: a request document, a JSON answer, and the tagged
 * failure a refused run leaves with the caller.
 *
 * A Workspace of this program's own is the subject of every call, so an answer is read against a
 * store and a Layout this run made rather than one another run left behind. The progress and
 * cancellation handles are made once and handed in uncancelled, because a check that reads a JSON
 * answer would otherwise be answered with a cancellation instead.
 */

#define _POSIX_C_SOURCE 200809L

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
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
    snprintf(dir, sizeof dir, "%s/rola-ffi-write-flows-%ld", tmp, (long)getpid());
    remove_tree(dir);
#ifdef _WIN32
    if (mkdir(dir) != 0) {
#else
    if (mkdir(dir, 0700) != 0) {
#endif
        perror("mkdir");
        return 2;
    }

    RorolalaResult made = create_workspace(dir);
    want(made.tag == RorolalaResult_Ok, "a Workspace is made");

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

    RorolalaResult refused = rola_write_bring("", progress, cancellation);
    want(refused.tag == RorolalaResult_Err, "an empty request is refused");
    want(refused.payload != NULL, "the refusal carries an error");
    if (refused.tag == RorolalaResult_Err && refused.payload != NULL) {
        RolaWriteError *error = refused.payload;
        want(error->tag == RolaWriteError_Json, "an empty request is a JSON failure");
    }
    release_answer(refused);

    char path[1024];
    snprintf(path, sizeof path, "%s/a.txt", dir);
    FILE *file = fopen(path, "wb");
    want(file != NULL, "a.txt is made");
    if (file != NULL) {
        fputs("one", file);
        fclose(file);
    }

    RorolalaResult first = rola_write_track_file(
        dir,
        "a.txt",
        "alice",
        "first",
        "",
        "",
        ""
    );
    want(first.tag == RorolalaResult_Ok, "a.txt is tracked");
    release_answer(first);

    want(unlink(path) == 0, "a.txt is taken away from disk");

    char request[2048];
    snprintf(
        request,
        sizeof request,
        "{\"workspace\":\"%s\",\"path\":\"a.txt\",\"mode\":{\"kind\":\"delete\"}}",
        dir
    );
    RorolalaResult aligned = rola_write_align(request, progress, cancellation);
    want(aligned.tag == RorolalaResult_Ok, "deleting a lost path succeeds");
    want(aligned.payload != NULL, "the deletion answers with what it settled");
    if (aligned.tag == RorolalaResult_Ok && aligned.payload != NULL) {
        const char *json = aligned.payload;
        want(strstr(json, "a.txt") != NULL, "the deletion names the path");
    }
    release_answer(aligned);

    snprintf(path, sizeof path, "%s/b.txt", dir);
    file = fopen(path, "wb");
    want(file != NULL, "b.txt is made");
    if (file != NULL) {
        fputs("one", file);
        fclose(file);
    }

    RorolalaResult second = rola_write_track_file(
        dir,
        "b.txt",
        "alice",
        "first",
        "",
        "",
        ""
    );
    want(second.tag == RorolalaResult_Ok, "b.txt is tracked");
    release_answer(second);

    file = fopen(path, "wb");
    want(file != NULL, "b.txt is edited");
    if (file != NULL) {
        fputs("two", file);
        fclose(file);
    }

    RorolalaResult again = rola_write_track_file(
        dir,
        "b.txt",
        "alice",
        "second",
        "",
        "",
        ""
    );
    want(again.tag == RorolalaResult_Ok, "b.txt is tracked again");
    release_answer(again);

    /* An empty record moves one version back, which is only a change because the second track made
       a second version to move from. */
    snprintf(request, sizeof request, "{\"workspace\":\"%s\",\"file\":\"b.txt\"}", dir);
    RorolalaResult retracked = rola_write_retrack(request, progress, cancellation);
    want(retracked.tag == RorolalaResult_Ok, "retracking a path succeeds");
    want(retracked.payload != NULL, "the retrack answers with the record it moved");
    if (retracked.tag == RorolalaResult_Ok && retracked.payload != NULL) {
        const char *json = retracked.payload;
        want(strstr(json, "\"kind\":\"path\"") != NULL, "the answer is a path's record");
        want(strstr(json, "\"changed\":true") != NULL, "the record moved to another version");
    }
    release_answer(retracked);

    /*
     * A Workspace with no account bound and no tracked Vault leaves two gates in the way. Which one
     * stops a run first is the request layer's order rather than a promise to a caller, so either
     * tag answers these checks.
     */
    snprintf(request, sizeof request, "{\"workspace\":\"%s\",\"refs\":[]}", dir);
    RorolalaResult unbrought = rola_write_bring(request, progress, cancellation);
    want(
        unbrought.tag == RorolalaResult_Err,
        "a bring with neither an account nor a tracked Vault is refused"
    );
    want(unbrought.payload != NULL, "the bring failure carries an error");
    if (unbrought.tag == RorolalaResult_Err && unbrought.payload != NULL) {
        RolaWriteError *error = unbrought.payload;
        want(
            error->tag == RolaWriteError_NoAccount || error->tag == RolaWriteError_NoTrack,
            "the bring names a missing account or Vault"
        );
    }
    release_answer(unbrought);

    snprintf(
        request,
        sizeof request,
        "{\"workspace\":\"%s\",\"kind\":\"hold\",\"paths\":[]}",
        dir
    );
    RorolalaResult unplanned = rola_write_ownership_plan(request, progress, cancellation);
    want(
        unplanned.tag == RorolalaResult_Err,
        "an ownership plan with neither an account nor a tracked Vault is refused"
    );
    want(unplanned.payload != NULL, "the ownership plan failure carries an error");
    if (unplanned.tag == RorolalaResult_Err && unplanned.payload != NULL) {
        RolaWriteError *error = unplanned.payload;
        want(
            error->tag == RolaWriteError_NoAccount || error->tag == RolaWriteError_NoTrack,
            "the ownership plan names a missing account or Vault"
        );
    }
    release_answer(unplanned);

    /* Track's plan and apply speak for an account, so the key is looked for before anything is read:
       a Workspace without one is refused the same way, and an empty plan is a document the caller
       can send rather than a shape this side invents. */
    snprintf(request, sizeof request, "{\"workspace\":\"%s\",\"files\":[\"a.txt\"]}", dir);
    RorolalaResult unplanned_track = rola_write_track_plan(request, progress, cancellation);
    want(unplanned_track.tag == RorolalaResult_Err, "a track plan with no account is refused");
    want(unplanned_track.payload != NULL, "the track plan failure carries an error");
    if (unplanned_track.tag == RorolalaResult_Err && unplanned_track.payload != NULL) {
        RolaWriteError *error = unplanned_track.payload;
        want(
            error->tag == RolaWriteError_NoAccount,
            "the track plan names a missing account"
        );
    }
    release_answer(unplanned_track);

    RorolalaResult unapplied_track = rola_write_track_apply(
        request,
        "{\"subjects\":[]}",
        "{}",
        progress,
        cancellation
    );
    want(unapplied_track.tag == RorolalaResult_Err, "a track apply with no account is refused");
    want(unapplied_track.payload != NULL, "the track apply failure carries an error");
    if (unapplied_track.tag == RorolalaResult_Err && unapplied_track.payload != NULL) {
        RolaWriteError *error = unapplied_track.payload;
        want(
            error->tag == RolaWriteError_NoAccount,
            "the track apply names a missing account"
        );
    }
    release_answer(unapplied_track);

    RorolalaResult empty_track = rola_write_track_plan("", progress, cancellation);
    want(empty_track.tag == RorolalaResult_Err, "an empty track request is refused");
    if (empty_track.tag == RorolalaResult_Err && empty_track.payload != NULL) {
        RolaWriteError *error = empty_track.payload;
        want(error->tag == RolaWriteError_Json, "an empty track request is a JSON failure");
    }
    release_answer(empty_track);

    /*
     * An account is located by the name of its key file, so a file this test never signs with is
     * enough for a recording: the creator is a name, not a signature. The key comes only now, so the
     * checks above still see a Workspace without an account.
     */
    char keys[1024];
    snprintf(keys, sizeof keys, "%s/.rola/auth", dir);
#ifdef _WIN32
    want(mkdir(keys) == 0 || errno == EEXIST, "the account key directory is made");
#else
    want(mkdir(keys, 0700) == 0 || errno == EEXIST, "the account key directory is made");
#endif
    snprintf(path, sizeof path, "%s/alice.pem", keys);
    FILE *key = fopen(path, "wb");
    want(key != NULL, "the account key file is made");
    if (key != NULL) {
        fputs("not a key this test ever signs with", key);
        fclose(key);
    }

    snprintf(path, sizeof path, "%s/c.txt", dir);
    FILE *later = fopen(path, "wb");
    want(later != NULL, "c.txt is made");
    if (later != NULL) {
        fputs("one", later);
        fclose(later);
    }

    snprintf(
        request,
        sizeof request,
        "{\"workspace\":\"%s\",\"account\":\"alice\",\"files\":[\"c.txt\"]}",
        dir
    );
    RorolalaResult planned_track = rola_write_track_plan(request, progress, cancellation);
    want(planned_track.tag == RorolalaResult_Ok, "a track plan with an account succeeds");
    want(planned_track.payload != NULL, "the plan names what it read");
    if (planned_track.tag == RorolalaResult_Ok && planned_track.payload != NULL) {
        const char *plan = (const char *)planned_track.payload;
        want(strstr(plan, "\"subjects\"") != NULL, "the plan lists its subjects");
        want(strstr(plan, "\"c.txt\"") != NULL, "the plan names the file it read");

        RorolalaResult applied_track = rola_write_track_apply(
            request,
            plan,
            "{\"c.txt\":\"first\"}",
            progress,
            cancellation
        );
        want(applied_track.tag == RorolalaResult_Ok, "a track apply with a plan succeeds");
        want(applied_track.payload != NULL, "the apply answers what it recorded");
        if (applied_track.tag == RorolalaResult_Ok && applied_track.payload != NULL) {
            const char *drawn = (const char *)applied_track.payload;
            want(strstr(drawn, "\"action\":\"added\"") != NULL, "the recording is a gain");
        }
        release_answer(applied_track);
    }
    release_answer(planned_track);

    /* Cancelling is a flag the caller sets, so a handle cancelled before the call is answered at the
       first check rather than after the work. What the signal does is set the same flag. */
    RolaCancellation *stopped = rola_cancellation_new();
    want(stopped != NULL, "a cancellation handle is made for the stopped run");
    if (stopped != NULL) {
        rola_cancellation_cancel(stopped);
        RorolalaResult cancelled = rola_write_track_plan(request, progress, stopped);
        want(cancelled.tag == RorolalaResult_Err, "a cancelled plan is refused");
        if (cancelled.tag == RorolalaResult_Err && cancelled.payload != NULL) {
            RolaWriteError *error = cancelled.payload;
            want(
                error->tag == RolaWriteError_Cancelled,
                "the cancelled plan says it was cancelled"
            );
        }
        release_answer(cancelled);
        free_rola_cancellation(stopped);
    }

    /*
     * A variant merge takes the same request document with a `join`, and the variant itself named as
     * well: what the words mean is the Vault's to say, so a bad one is refused rather than guessed.
     */
    snprintf(
        request,
        sizeof request,
        "{\"workspace\":\"%s\",\"join\":\"not-a-variant\",\"variant\":\"not-a-variant\","
        "\"file\":\"c.txt\"}",
        dir
    );
    RorolalaResult unjoined = rola_write_bring(request, progress, cancellation);
    want(unjoined.tag == RorolalaResult_Err, "a variant that is not a hash is refused");
    want(unjoined.payload != NULL, "the variant failure carries an error");
    if (unjoined.tag == RorolalaResult_Err && unjoined.payload != NULL) {
        RolaWriteError *error = unjoined.payload;
        want(
            error->tag == RolaWriteError_InvalidPath
                || error->tag == RolaWriteError_Precondition
                || error->tag == RolaWriteError_NotFound,
            "the variant failure is a refused name, not a transport failure"
        );
    }
    release_answer(unjoined);

    want(
        !rola_cancellation_is_cancelled(cancellation),
        "a fresh cancellation has not been asked to stop"
    );
    rola_cancellation_cancel(cancellation);
    want(
        rola_cancellation_is_cancelled(cancellation),
        "a cancelled write is told to stop"
    );
    free_rola_cancellation(cancellation);
    free_rola_progress(progress);

    remove_tree(dir);

    return finish();
}
