#pragma once

#include <glib.h>
#include <stddef.h>

/* A command being built argument by argument. A POSIX program is handed its
   words already split, so nothing here quotes or splits: each call is one
   `argv` entry, however it is spelled. */
typedef struct CmdLine {
  GPtrArray *args;
} CmdLine;

void cmd_init(CmdLine *cmd);
void cmd_free(CmdLine *cmd);
gboolean cmd_add(CmdLine *cmd, const char *argument);

/* What a run of a command-line program left behind. `output` is the combined
   stdout and stderr, as UTF-8. */
typedef struct ProcessResult {
  int exit_code;
  char *output;
  size_t output_len;
} ProcessResult;

void process_result_init(ProcessResult *result);
void process_result_free(ProcessResult *result);

/* Runs `exe_path` with `cmd` and waits for it. Returns FALSE only when the
   process could not be started or its output could not be read; a non-zero
   exit code is a successful run with a non-zero code. */
gboolean run_process(const char *exe_path, const CmdLine *cmd,
                     const char *working_dir, ProcessResult *result,
                     char *error, size_t error_cap);
