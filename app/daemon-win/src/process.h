#pragma once

#include <windows.h>

#include "util.h"

/* A command line being built argument by argument, quoted the way CreateProcess
   and the C runtime's argv splitting expect. */
typedef struct CmdLine {
  StrBuf text;
} CmdLine;

void cmd_init(CmdLine *cmd);
void cmd_free(CmdLine *cmd);
BOOL cmd_add(CmdLine *cmd, const WCHAR *argument);

/* What a run of a command-line program left behind. `output` is the combined
   stdout and stderr as raw bytes; use `process_output_wide` to read it. */
typedef struct ProcessResult {
  DWORD exit_code;
  char *output;
  size_t output_len;
} ProcessResult;

void process_result_init(ProcessResult *result);
void process_result_free(ProcessResult *result);
WCHAR *process_output_wide(const ProcessResult *result);

/* Runs `exe_path` with `arguments` (already quoted, or empty) and waits for it.
   Returns FALSE only when the process could not be started or its output could
   not be read; a non-zero exit code is a successful run with a non-zero code.
 */
BOOL run_process(const WCHAR *exe_path, const WCHAR *arguments,
                 const WCHAR *working_dir, ProcessResult *result, WCHAR *error,
                 size_t error_cap);
