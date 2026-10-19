#include "process.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void cmd_init(CmdLine *cmd) { sb_init(&cmd->text); }

void cmd_free(CmdLine *cmd) { sb_free(&cmd->text); }

static BOOL append_quoted(StrBuf *sb, const WCHAR *argument) {
  const WCHAR *cursor;
  BOOL needs_quotes = argument[0] == L'\0';

  for (cursor = argument; *cursor; ++cursor) {
    if (*cursor == L' ' || *cursor == L'\t' || *cursor == L'"') {
      needs_quotes = TRUE;
      break;
    }
  }

  if (!needs_quotes) {
    return sb_append(sb, argument);
  }

  if (!sb_append_char(sb, L'"')) {
    return FALSE;
  }

  cursor = argument;
  while (*cursor) {
    size_t slashes = 0;
    size_t i;

    while (*cursor == L'\\') {
      ++cursor;
      ++slashes;
    }

    if (*cursor == L'"') {
      for (i = 0; i < slashes * 2 + 1; ++i) {
        if (!sb_append_char(sb, L'\\')) {
          return FALSE;
        }
      }
      if (!sb_append_char(sb, L'"')) {
        return FALSE;
      }
      ++cursor;
    } else if (*cursor == L'\0') {
      for (i = 0; i < slashes * 2; ++i) {
        if (!sb_append_char(sb, L'\\')) {
          return FALSE;
        }
      }
    } else {
      for (i = 0; i < slashes; ++i) {
        if (!sb_append_char(sb, L'\\')) {
          return FALSE;
        }
      }
      if (!sb_append_char(sb, *cursor++)) {
        return FALSE;
      }
    }
  }

  return sb_append_char(sb, L'"');
}

BOOL cmd_add(CmdLine *cmd, const WCHAR *argument) {
  if (cmd->text.len > 0 && !sb_append_char(&cmd->text, L' ')) {
    return FALSE;
  }
  return append_quoted(&cmd->text, argument ? argument : L"");
}

void process_result_init(ProcessResult *result) {
  result->exit_code = (DWORD)-1;
  result->output = NULL;
  result->output_len = 0;
}

void process_result_free(ProcessResult *result) {
  free(result->output);
  process_result_init(result);
}

WCHAR *process_output_wide(const ProcessResult *result) {
  if (!result->output) {
    return utf8_to_wide("");
  }
  return utf8_to_wide_n(result->output, result->output_len);
}

static void set_last_error_text(WCHAR *error, size_t error_cap, DWORD code) {
  WCHAR text[1024];

  if (!error || error_cap == 0) {
    return;
  }

  text[0] = L'\0';
  if (FormatMessageW(FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
                     NULL, code, 0, text, (DWORD)ARRAY_COUNT(text),
                     NULL) == 0) {
    _snwprintf_s(error, error_cap, _TRUNCATE, L"Windows error %lu",
                 (unsigned long)code);
    return;
  }

  str_trim(text);
  str_copy(error, error_cap, text);
}

BOOL run_process(const WCHAR *exe_path, const WCHAR *arguments,
                 const WCHAR *working_dir, ProcessResult *result, WCHAR *error,
                 size_t error_cap) {
  SECURITY_ATTRIBUTES security;
  STARTUPINFOW startup;
  PROCESS_INFORMATION process = {0};
  HANDLE read_pipe = INVALID_HANDLE_VALUE;
  HANDLE write_pipe = INVALID_HANDLE_VALUE;
  HANDLE null_input = INVALID_HANDLE_VALUE;
  WCHAR *command_line = NULL;
  char *output = NULL;
  size_t output_length = 0;
  size_t output_capacity = 0;
  BOOL started = FALSE;
  BOOL ok = FALSE;

  process_result_init(result);
  if (error && error_cap) {
    error[0] = L'\0';
  }

  if (!exe_path || !exe_path[0]) {
    if (error && error_cap) {
      str_copy(error, error_cap, L"No executable was named.");
    }
    return FALSE;
  }

  {
    size_t exe_length = wcslen(exe_path);
    size_t args_length = arguments ? wcslen(arguments) : 0;
    command_line =
        (WCHAR *)malloc((exe_length + args_length + 4) * sizeof(WCHAR));
    if (!command_line) {
      if (error && error_cap) {
        str_copy(error, error_cap, L"Out of memory.");
      }
      return FALSE;
    }
    _snwprintf_s(command_line, exe_length + args_length + 4, _TRUNCATE,
                 L"\"%s\"%s%s", exe_path, args_length ? L" " : L"",
                 arguments ? arguments : L"");
  }

  security.nLength = sizeof(security);
  security.lpSecurityDescriptor = NULL;
  security.bInheritHandle = TRUE;

  if (!CreatePipe(&read_pipe, &write_pipe, &security, 0)) {
    set_last_error_text(error, error_cap, GetLastError());
    goto cleanup;
  }
  if (!SetHandleInformation(read_pipe, HANDLE_FLAG_INHERIT, 0)) {
    set_last_error_text(error, error_cap, GetLastError());
    goto cleanup;
  }

  null_input =
      CreateFileW(L"NUL", GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE,
                  &security, OPEN_EXISTING, 0, NULL);

  ZeroMemory(&startup, sizeof(startup));
  startup.cb = sizeof(startup);
  startup.dwFlags = STARTF_USESTDHANDLES;
  startup.hStdInput = null_input;
  startup.hStdOutput = write_pipe;
  startup.hStdError = write_pipe;

  ZeroMemory(&process, sizeof(process));
  started = CreateProcessW(NULL, command_line, NULL, NULL, TRUE,
                           CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT, NULL,
                           (working_dir && working_dir[0]) ? working_dir : NULL,
                           &startup, &process);
  if (!started) {
    set_last_error_text(error, error_cap, GetLastError());
    goto cleanup;
  }

  CloseHandle(write_pipe);
  write_pipe = INVALID_HANDLE_VALUE;
  if (null_input != INVALID_HANDLE_VALUE) {
    CloseHandle(null_input);
    null_input = INVALID_HANDLE_VALUE;
  }

  output_capacity = 4096;
  output = (char *)malloc(output_capacity);
  if (!output) {
    if (error && error_cap) {
      str_copy(error, error_cap, L"Out of memory.");
    }
    goto cleanup;
  }

  for (;;) {
    char chunk[4096];
    DWORD read = 0;

    if (!ReadFile(read_pipe, chunk, (DWORD)sizeof(chunk), &read, NULL) ||
        read == 0) {
      break;
    }

    if (output_length + read + 1 > output_capacity) {
      size_t capacity = output_capacity;
      char *grown;
      while (capacity < output_length + read + 1) {
        capacity *= 2;
      }
      grown = (char *)realloc(output, capacity);
      if (!grown) {
        if (error && error_cap) {
          str_copy(error, error_cap, L"Out of memory.");
        }
        goto cleanup;
      }
      output = grown;
      output_capacity = capacity;
    }

    memcpy(output + output_length, chunk, read);
    output_length += read;
  }

  WaitForSingleObject(process.hProcess, INFINITE);
  GetExitCodeProcess(process.hProcess, &result->exit_code);

  output[output_length] = '\0';
  result->output = output;
  result->output_len = output_length;
  output = NULL;
  ok = TRUE;

cleanup:
  if (started) {
    CloseHandle(process.hThread);
    CloseHandle(process.hProcess);
  }
  if (read_pipe != INVALID_HANDLE_VALUE) {
    CloseHandle(read_pipe);
  }
  if (write_pipe != INVALID_HANDLE_VALUE) {
    CloseHandle(write_pipe);
  }
  if (null_input != INVALID_HANDLE_VALUE) {
    CloseHandle(null_input);
  }
  free(output);
  free(command_line);
  return ok;
}
