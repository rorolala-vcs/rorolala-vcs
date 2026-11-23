/*
 * The one-shot write workflows across the ABI: a request a caller fills, a typed answer, and the
 * tagged failure a refused run leaves with the caller.
 *
 * A Workspace of this program's own is the subject of every call, so an answer is read against a
 * store and a Layout this run made rather than one another run left behind. The progress and
 * cancellation handles are made once and handed in uncancelled, because a check that reads an answer
 * would otherwise be answered with a cancellation instead.
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

/* The fields every write request carries, naming a Workspace and no account. */
static RolaWriteCommon common_of(const char *workspace, const char *account) {
    RolaWriteCommon common;
    memset(&common, 0, sizeof common);
    common.workspace = workspace;
    common.account = account;
    common.target.tag = RolaWriteTarget_Tracked;

    return common;
}

/* A list of strings as C hands one over. */
static RolaStrSpan span_of(const char **items, size_t count) {
    RolaStrSpan span;
    span.items = items;
    span.count = count;

    return span;
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

    RorolalaResult made = rola_workspace_create(dir);
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

    /* A request that names no Workspace is refused before anything is read. */
    RolaWriteBringRequest empty;
    memset(&empty, 0, sizeof empty);
    RorolalaResult refused = rola_write_bring(&empty, progress, cancellation);
    want(refused.tag == RorolalaResult_Err, "a request that names no Workspace is refused");
    want(refused.payload != NULL, "the refusal carries an error");
    if (refused.tag == RorolalaResult_Err && refused.payload != NULL) {
        RolaWriteError *error = refused.payload;
        want(error->tag == RolaWriteError_NoWorkspace, "the refusal names a missing Workspace");
    }
    free_rola_write_error(refused.payload);

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
    if (first.tag == RorolalaResult_Ok && first.payload != NULL) {
        free_rola_write_tracked_file(first.payload);
    }

    want(unlink(path) == 0, "a.txt is taken away from disk");

    RolaWriteAlignRequest aligned_request;
    memset(&aligned_request, 0, sizeof aligned_request);
    aligned_request.common = common_of(dir, "");
    aligned_request.path = "a.txt";
    aligned_request.mode.tag = RolaWriteAlignMode_Delete;

    RorolalaResult aligned = rola_write_align(&aligned_request, progress, cancellation);
    want(aligned.tag == RorolalaResult_Ok, "deleting a lost path succeeds");
    want(aligned.payload != NULL, "the deletion answers with what it settled");
    if (aligned.tag == RorolalaResult_Ok && aligned.payload != NULL) {
        RolaWriteAlign *answer = aligned.payload;
        char *kind = rola_write_align_kind(answer);
        want(kind != NULL && strcmp(kind, "paths") == 0, "the deletion is a list of paths");
        free_string(kind);
        want(rola_write_align_paths_count(answer) == 1, "the deletion names its path");
        RolaWriteAlignEntry *entry = rola_write_align_paths_at(answer, 0);
        want(entry != NULL, "the settled path is there");
        if (entry != NULL) {
            char *what = rola_write_align_entry_what(entry);
            want(what != NULL && strcmp(what, "a.txt") == 0, "the deletion names a.txt");
            free_string(what);
            free_rola_write_align_entry(entry);
        }
        free_rola_write_align(answer);
    }

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
    if (second.tag == RorolalaResult_Ok && second.payload != NULL) {
        free_rola_write_tracked_file(second.payload);
    }

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
    if (again.tag == RorolalaResult_Ok && again.payload != NULL) {
        free_rola_write_tracked_file(again.payload);
    }

    /* An empty record moves one version back, which is only a change because the second track made
       a second version to move from. */
    RolaWriteRetrackRequest retrack_request;
    memset(&retrack_request, 0, sizeof retrack_request);
    retrack_request.common = common_of(dir, "");
    retrack_request.file = "b.txt";

    RorolalaResult retracked = rola_write_retrack(&retrack_request, progress, cancellation);
    want(retracked.tag == RorolalaResult_Ok, "retracking a path succeeds");
    want(retracked.payload != NULL, "the retrack answers with the record it moved");
    if (retracked.tag == RorolalaResult_Ok && retracked.payload != NULL) {
        RolaWriteRetrack *answer = retracked.payload;
        char *kind = rola_write_retrack_kind(answer);
        want(kind != NULL && strcmp(kind, "path") == 0, "the answer is a path's record");
        free_string(kind);
        want(rola_write_retrack_changed(answer), "the record moved to another version");
        free_rola_write_retrack(answer);
    }

    /*
     * A Workspace with no account bound and no tracked Vault leaves two gates in the way. Which one
     * stops a run first is the request layer's order rather than a promise to a caller, so either
     * tag answers these checks.
     */
    RolaWriteBringRequest bring_request;
    memset(&bring_request, 0, sizeof bring_request);
    bring_request.common = common_of(dir, "");

    RorolalaResult unbrought = rola_write_bring(&bring_request, progress, cancellation);
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
    free_rola_write_error(unbrought.payload);

    RolaWriteOwnershipRequest ownership_request;
    memset(&ownership_request, 0, sizeof ownership_request);
    ownership_request.common = common_of(dir, "");
    ownership_request.kind = RolaWriteOwnershipKind_Hold;

    RorolalaResult unplanned =
        rola_write_ownership_plan(&ownership_request, progress, cancellation);
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
    free_rola_write_error(unplanned.payload);

    /* Track's plan and apply speak for an account, so the key is looked for before anything is read:
       a Workspace without one is refused the same way, and an empty array is a plan the caller can
       send rather than a shape this side invents. */
    const char *files[] = {"a.txt"};
    RolaWriteTrackRequest track_request;
    memset(&track_request, 0, sizeof track_request);
    track_request.common = common_of(dir, "");
    track_request.files = span_of(files, 1);

    RorolalaResult unplanned_track =
        rola_write_track_plan(&track_request, progress, cancellation);
    want(unplanned_track.tag == RorolalaResult_Err, "a track plan with no account is refused");
    want(unplanned_track.payload != NULL, "the track plan failure carries an error");
    if (unplanned_track.tag == RorolalaResult_Err && unplanned_track.payload != NULL) {
        RolaWriteError *error = unplanned_track.payload;
        want(
            error->tag == RolaWriteError_NoAccount,
            "the track plan names a missing account"
        );
    }
    free_rola_write_error(unplanned_track.payload);

    RorolalaResult unapplied_track = rola_write_track_apply(
        &track_request,
        NULL,
        0,
        NULL,
        0,
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
    free_rola_write_error(unapplied_track.payload);

    RolaWriteTrackRequest no_workspace;
    memset(&no_workspace, 0, sizeof no_workspace);

    RorolalaResult empty_track = rola_write_track_plan(&no_workspace, progress, cancellation);
    want(empty_track.tag == RorolalaResult_Err, "a track request with no Workspace is refused");
    if (empty_track.tag == RorolalaResult_Err && empty_track.payload != NULL) {
        RolaWriteError *error = empty_track.payload;
        want(
            error->tag == RolaWriteError_NoWorkspace,
            "the track request names a missing Workspace"
        );
    }
    free_rola_write_error(empty_track.payload);

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

    const char *named_files[] = {"c.txt"};
    track_request.common = common_of(dir, "alice");
    track_request.files = span_of(named_files, 1);

    RorolalaResult planned_track =
        rola_write_track_plan(&track_request, progress, cancellation);
    want(planned_track.tag == RorolalaResult_Ok, "a track plan with an account succeeds");
    want(planned_track.payload != NULL, "the plan names what it read");
    if (planned_track.tag == RorolalaResult_Ok && planned_track.payload != NULL) {
        RolaWriteTrackPlan *plan = planned_track.payload;
        want(rola_write_track_plan_subject_count(plan) == 1, "the plan names its subject");
        RolaWriteTrackSubject *subject = rola_write_track_plan_subject_at(plan, 0);
        want(subject != NULL, "the subject is there");
        if (subject != NULL) {
            char *named = rola_write_track_subject_path(subject);
            char *digest = rola_write_track_subject_digest(subject);
            want(named != NULL && strcmp(named, "c.txt") == 0, "the plan names c.txt");

            /* An apply reads back what the plan promised to record, so the arrays it is handed
               name each subject's path and the digest the plan read. */
            RolaWriteTrackAgreed agreed[1];
            agreed[0].path = named;
            agreed[0].digest = digest;

            RolaWriteTrackMessage words[1];
            words[0].path = "c.txt";
            words[0].text = "first";

            free_string(named);
            free_string(digest);
            free_rola_write_track_subject(subject);

            RorolalaResult applied_track = rola_write_track_apply(
                &track_request,
                agreed,
                1,
                words,
                1,
                progress,
                cancellation
            );
            want(applied_track.tag == RorolalaResult_Ok, "a track apply with a plan succeeds");
            want(applied_track.payload != NULL, "the apply answers what it recorded");
            if (applied_track.tag == RorolalaResult_Ok && applied_track.payload != NULL) {
                RolaWriteTrackApply *applied = applied_track.payload;
                want(rola_write_track_apply_item_count(applied) == 1, "one subject is recorded");
                RolaWriteTrackItem *item = rola_write_track_apply_item_at(applied, 0);
                want(item != NULL, "the recorded item is there");
                if (item != NULL) {
                    char *action = rola_write_track_item_action(item);
                    want(
                        action != NULL && strcmp(action, "added") == 0,
                        "the recording is a gain"
                    );
                    free_string(action);
                    free_rola_write_track_item(item);
                }
                free_rola_write_track_apply(applied);
            }
        }
        free_rola_write_track_plan(plan);
    }

    /* Cancelling is a flag the caller sets, so a handle cancelled before the call is answered at the
       first check rather than after the work. What the signal does is set the same flag. */
    RolaCancellation *stopped = rola_cancellation_new();
    want(stopped != NULL, "a cancellation handle is made for the stopped run");
    if (stopped != NULL) {
        rola_cancellation_cancel(stopped);
        RorolalaResult cancelled = rola_write_track_plan(&track_request, progress, stopped);
        want(cancelled.tag == RorolalaResult_Err, "a cancelled plan is refused");
        if (cancelled.tag == RorolalaResult_Err && cancelled.payload != NULL) {
            RolaWriteError *error = cancelled.payload;
            want(
                error->tag == RolaWriteError_Cancelled,
                "the cancelled plan says it was cancelled"
            );
        }
        free_rola_write_error(cancelled.payload);
        free_rola_cancellation(stopped);
    }

    /*
     * A variant merge takes the same request with a `join`, and the variant itself named as well:
     * what the words mean is the Vault's to say, so a bad one is refused rather than guessed.
     */
    memset(&bring_request, 0, sizeof bring_request);
    bring_request.common = common_of(dir, "");
    bring_request.join = "not-a-variant";
    bring_request.variant = "not-a-variant";
    bring_request.file = "c.txt";

    RorolalaResult unjoined = rola_write_bring(&bring_request, progress, cancellation);
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
    free_rola_write_error(unjoined.payload);

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
