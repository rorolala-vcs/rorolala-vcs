/*
 * The history operations across the ABI: fold, cut, list what was recorded, and undo it.
 *
 * A Workspace of this program's own is the subject of every call, so what an operation recorded is
 * read back from it rather than from anything another run left behind. The file is inside the
 * Workspace, so no call needs the network.
 */

#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#include "rorolala_ffi.h"

#include "harness.h"

/* The fields every write request carries, naming a Workspace and no account. */
static RolaWriteCommon common_of(const char *workspace) {
    RolaWriteCommon common;
    memset(&common, 0, sizeof common);
    common.workspace = workspace;
    common.account = "";
    common.target.tag = RolaWriteTarget_Tracked;

    return common;
}

/* Writes `text` at `path`. */
static void write_file(const char *path, const char *text) {
    FILE *file = fopen(path, "wb");
    want(file != NULL, "the file is made");
    if (file != NULL) {
        fputs(text, file);
        fclose(file);
    }
}

/* Tracks one path through the one-file export, which records no operation. */
static void track(const char *dir, const char *path, const char *message) {
    RorolalaResult tracked = rola_write_track_file(dir, path, "alice", message, "", "", "");
    want(tracked.tag == RorolalaResult_Ok, "the file is recorded");

    if (tracked.tag == RorolalaResult_Ok && tracked.payload != NULL) {
        free_rola_write_tracked_file(tracked.payload);
    } else if (tracked.tag == RorolalaResult_Err) {
        free_rola_write_error(tracked.payload);
    }
}

/* How many operations the Workspace's log holds, or -1 when it cannot be read. */
static int recorded(const char *dir) {
    RolaHistoryOperations *log = rola_history_operations(dir);
    if (log == NULL) {
        return -1;
    }

    int count = (int)rola_history_operations_count(log);
    free_rola_history_operations(log);

    return count;
}

int main(void) {
    const char *tmp = getenv("TMPDIR");
    if (tmp == NULL) {
        tmp = "/tmp";
    }

    char dir[512];
    snprintf(dir, sizeof dir, "%s/rola-ffi-history-%ld", tmp, (long)getpid());
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

    char path[512];
    snprintf(path, sizeof path, "%s/a.txt", dir);
    write_file(path, "one");
    track(dir, "a.txt", "first");
    write_file(path, "two");
    track(dir, "a.txt", "second");

    want(recorded(dir) == 0, "a one-file track records no operation");

    /* The handles a write reads: the progress it reports through and the flag it stops on. Both are
     * handed in uncancelled, and both are released before the program ends. */
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

    RolaWriteCollapseRequest collapse;
    memset(&collapse, 0, sizeof collapse);
    collapse.common = common_of(dir);
    collapse.file = "a.txt";

    RorolalaResult folded = rola_write_collapse(&collapse, progress, cancellation);
    want(folded.tag == RorolalaResult_Ok, "the chain is folded");
    if (folded.tag == RorolalaResult_Ok && folded.payload != NULL) {
        RolaWriteCollapse *answer = folded.payload;
        char *file = rola_write_collapse_file(answer);
        char *from = rola_write_collapse_from(answer);
        char *to = rola_write_collapse_to(answer);
        want(file != NULL && strcmp(file, "a.txt") == 0, "the answer names the path");
        want(from != NULL && from[0] != '\0', "the answer names the version before");
        want(to != NULL && to[0] != '\0', "the answer names the version after");
        want(rola_write_collapse_folded(answer) == 1, "one version is folded");
        free_string(file);
        free_string(from);
        free_string(to);
        free_rola_write_collapse(answer);
    } else if (folded.tag == RorolalaResult_Err) {
        free_rola_write_error(folded.payload);
    }

    want(recorded(dir) == 1, "the fold is recorded");

    RolaHistoryOperations *log = rola_history_operations(dir);
    want(log != NULL, "the log is listed");
    if (log != NULL) {
        want(rola_history_operations_count(log) == 1, "the log holds one operation");
        want(rola_history_operations_unreadable_count(log) == 0, "every file reads");

        RolaHistoryOperation *first = rola_history_operations_at(log, 0);
        want(first != NULL, "the operation is there");
        if (first != NULL) {
            char *command = rola_history_operation_command(first);
            char *time = rola_history_operation_time(first);
            want(command != NULL && strcmp(command, "collapse") == 0, "it names the fold");
            want(time != NULL && time[0] != '\0', "it says when it ended");
            want(rola_history_operation_number(first) == 1, "the newest is numbered first");
            free_string(command);
            free_string(time);
            free_rola_history_operation(first);
        }

        want(rola_history_operations_at(log, 1) == NULL, "nothing past the end");
        free_rola_history_operations(log);
    }

    RolaWriteUndoRequest undo;
    memset(&undo, 0, sizeof undo);
    undo.common = common_of(dir);
    undo.on_partial = RolaWriteUndoPartial_Stop;

    RorolalaResult planned = rola_write_undo_plan(&undo);
    want(planned.tag == RorolalaResult_Ok, "the stack is read");
    if (planned.tag == RorolalaResult_Ok && planned.payload != NULL) {
        RolaWriteUndoPlan *answer = planned.payload;
        want(rola_write_undo_plan_count(answer) == 1, "one operation would be undone");

        RolaWriteUndoGroup *group = rola_write_undo_plan_at(answer, 0);
        want(group != NULL, "the operation stands");
        if (group != NULL) {
            char *standing = rola_write_undo_group_standing(group);
            want(standing != NULL && strcmp(standing, "whole") == 0, "it stands whole");
            free_string(standing);
            free_rola_write_undo_group(group);
        }

        free_rola_write_undo_plan(answer);
    } else if (planned.tag == RorolalaResult_Err) {
        free_rola_write_error(planned.payload);
    }

    RorolalaResult undone = rola_write_undo(&undo);
    want(undone.tag == RorolalaResult_Ok, "the fold is undone");
    if (undone.tag == RorolalaResult_Ok && undone.payload != NULL) {
        RolaWriteUndone *answer = undone.payload;
        want(rola_write_undone_count(answer) == 1, "one operation was undone");

        char *command = rola_write_undone_command_at(answer, 0);
        want(command != NULL && strcmp(command, "collapse") == 0, "it names the fold");
        free_string(command);
        free_rola_write_undone(answer);
    } else if (undone.tag == RorolalaResult_Err) {
        free_rola_write_error(undone.payload);
    }

    want(recorded(dir) == 0, "the undone operation is off the stack");

    RolaWriteCutRequest cut;
    memset(&cut, 0, sizeof cut);
    cut.common = common_of(dir);
    cut.file = "a.txt";

    RorolalaResult cut_result = rola_write_cut(&cut, progress, cancellation);
    want(cut_result.tag == RorolalaResult_Ok, "the chain is cut");
    if (cut_result.tag == RorolalaResult_Ok && cut_result.payload != NULL) {
        RolaWriteCut *answer = cut_result.payload;
        char *uuid = rola_write_cut_uuid(answer);
        char *point = rola_write_cut_point(answer);
        want(uuid != NULL && uuid[0] != '\0', "the answer names the Uuid it records");
        want(point != NULL && point[0] != '\0', "the answer names the cut point");
        free_string(uuid);
        free_string(point);
        free_rola_write_cut(answer);
    } else if (cut_result.tag == RorolalaResult_Err) {
        free_rola_write_error(cut_result.payload);
    }

    want(recorded(dir) == 1, "the cut is recorded");

    RorolalaResult cut_undone = rola_write_undo(&undo);
    want(cut_undone.tag == RorolalaResult_Ok, "the cut is undone");
    if (cut_undone.tag == RorolalaResult_Ok && cut_undone.payload != NULL) {
        free_rola_write_undone(cut_undone.payload);
    } else if (cut_undone.tag == RorolalaResult_Err) {
        free_rola_write_error(cut_undone.payload);
    }

    want(recorded(dir) == 0, "the stack is empty again");

    RorolalaResult empty = rola_write_undo(&undo);
    want(empty.tag == RorolalaResult_Err, "an empty stack is refused");
    if (empty.tag == RorolalaResult_Err && empty.payload != NULL) {
        RolaWriteError *error = empty.payload;
        want(error->tag == RolaWriteError_NotFound, "the refusal names nothing to undo");
    }
    free_rola_write_error(empty.payload);

    /* A sheet stands on a version of the chain, and this one names a version no chain holds. The
     * content column is what keeps a working sheet out of a C caller's reach: it has to name a key
     * of the store, which no export answers. */
    RolaWriteTweakRequest tweak;
    memset(&tweak, 0, sizeof tweak);
    tweak.common = common_of(dir);
    tweak.file = "a.txt";
    tweak.sheet = ">>> LOCAL\n>>> 000000000000\n";

    RorolalaResult fitted = rola_write_tweak(&tweak, progress, cancellation);
    want(fitted.tag == RorolalaResult_Err, "a sheet naming no version of the chain is refused");
    if (fitted.tag == RorolalaResult_Err && fitted.payload != NULL) {
        RolaWriteError *error = fitted.payload;
        want(error->tag == RolaWriteError_Precondition, "the refusal names a precondition");
    }
    free_rola_write_error(fitted.payload);

    RolaWriteTweakCancelRequest cancel;
    memset(&cancel, 0, sizeof cancel);
    cancel.common = common_of(dir);
    cancel.file = "a.txt";

    RorolalaResult nothing = rola_write_tweak_cancel(&cancel);
    want(nothing.tag == RorolalaResult_Err, "cancelling a plan that is not there is refused");
    if (nothing.tag == RorolalaResult_Err && nothing.payload != NULL) {
        RolaWriteError *error = nothing.payload;
        want(error->tag == RolaWriteError_NotFound, "the refusal names a plan that is not there");
    }
    free_rola_write_error(nothing.payload);

    free_rola_progress(progress);
    free_rola_cancellation(cancellation);

    remove_tree(dir);

    return finish();
}
