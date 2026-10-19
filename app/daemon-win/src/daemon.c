#include "daemon.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "messages.h"

static HANDLE g_process;
static HANDLE g_pipe_read;
static HANDLE g_reader_thread;
static HWND g_notify;
static DWORD g_pid;
static DWORD g_last_exit_code;

typedef struct ReaderStart {
  HANDLE pipe;
  HWND notify;
} ReaderStart;

static DWORD WINAPI reader_thread(LPVOID parameter) {
  char chunk[4096];
  ReaderStart *start = (ReaderStart *)parameter;
  HANDLE pipe;
  HWND notify;

  if (!start) {
    return 0;
  }
  pipe = start->pipe;
  notify = start->notify;
  free(start);

  for (;;) {
    DWORD read = 0;
    char *message;

    if (!ReadFile(pipe, chunk, (DWORD)sizeof(chunk), &read, NULL) ||
        read == 0) {
      break;
    }

    message = (char *)malloc((size_t)read + 1);
    if (!message) {
      continue;
    }
    memcpy(message, chunk, read);
    message[read] = '\0';

    if (!PostMessageW(notify, WM_APP_DAEMON_OUTPUT, 0, (LPARAM)message)) {
      free(message);
    }
  }

  CloseHandle(pipe);
  return 0;
}

BOOL daemon_start(HWND notify, const WCHAR *exe, const WCHAR *arguments,
                  WCHAR *error, size_t error_cap) {
  SECURITY_ATTRIBUTES security;
  STARTUPINFOW startup;
  PROCESS_INFORMATION process = {0};
  HANDLE write_pipe = INVALID_HANDLE_VALUE;
  HANDLE null_input = INVALID_HANDLE_VALUE;
  HANDLE thread_pipe = INVALID_HANDLE_VALUE;
  ReaderStart *reader = NULL;
  WCHAR *command_line = NULL;
  WCHAR *working_dir = NULL;
  size_t exe_length;
  size_t args_length;
  BOOL started = FALSE;

  if (error && error_cap) {
    error[0] = L'\0';
  }

  if (daemon_is_running()) {
    if (error && error_cap) {
      str_copy(error, error_cap, L"The daemon is already running.");
    }
    return FALSE;
  }

  if (!exe || !exe[0] || !path_exists(exe)) {
    if (error && error_cap) {
      str_copy(error, error_cap, L"The rola-daemon executable was not found.");
    }
    return FALSE;
  }

  exe_length = wcslen(exe);
  args_length = arguments ? wcslen(arguments) : 0;
  command_line =
      (WCHAR *)malloc((exe_length + args_length + 8) * sizeof(WCHAR));
  if (!command_line) {
    if (error && error_cap) {
      str_copy(error, error_cap, L"Out of memory.");
    }
    return FALSE;
  }
  _snwprintf_s(command_line, exe_length + args_length + 8, _TRUNCATE,
               L"\"%s\"%s%s", exe, args_length ? L" " : L"",
               arguments ? arguments : L"");

  security.nLength = sizeof(security);
  security.lpSecurityDescriptor = NULL;
  security.bInheritHandle = TRUE;

  if (!CreatePipe(&g_pipe_read, &write_pipe, &security, 0)) {
    if (error && error_cap) {
      _snwprintf_s(error, error_cap, _TRUNCATE,
                   L"Could not create a pipe (error %lu).",
                   (unsigned long)GetLastError());
    }
    goto cleanup;
  }
  SetHandleInformation(g_pipe_read, HANDLE_FLAG_INHERIT, 0);

  null_input =
      CreateFileW(L"NUL", GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE,
                  &security, OPEN_EXISTING, 0, NULL);

  ZeroMemory(&startup, sizeof(startup));
  startup.cb = sizeof(startup);
  startup.dwFlags = STARTF_USESTDHANDLES;
  startup.hStdInput = null_input;
  startup.hStdOutput = write_pipe;
  startup.hStdError = write_pipe;

  working_dir = path_dirname(exe);

  ZeroMemory(&process, sizeof(process));
  started = CreateProcessW(NULL, command_line, NULL, NULL, TRUE,
                           CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT, NULL,
                           working_dir, &startup, &process);
  if (!started) {
    if (error && error_cap) {
      WCHAR text[1024];
      DWORD code = GetLastError();
      text[0] = L'\0';
      if (FormatMessageW(
              FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS, NULL,
              code, 0, text, (DWORD)ARRAY_COUNT(text), NULL) == 0) {
        _snwprintf_s(error, error_cap, _TRUNCATE,
                     L"Could not start rola-daemon (error %lu).",
                     (unsigned long)code);
      } else {
        str_trim(text);
        _snwprintf_s(error, error_cap, _TRUNCATE,
                     L"Could not start rola-daemon: %s", text);
      }
    }
    goto cleanup;
  }

  CloseHandle(write_pipe);
  write_pipe = INVALID_HANDLE_VALUE;
  if (null_input != INVALID_HANDLE_VALUE) {
    CloseHandle(null_input);
    null_input = INVALID_HANDLE_VALUE;
  }

  CloseHandle(process.hThread);

  g_process = process.hProcess;
  g_pid = process.dwProcessId;
  g_notify = notify;
  g_last_exit_code = 0;

  /* The reader gets its own handle, so it can never be pointed at a later run's
     pipe when a stop and a start happen back to back. */
  if (DuplicateHandle(GetCurrentProcess(), g_pipe_read, GetCurrentProcess(),
                      &thread_pipe, 0, FALSE, DUPLICATE_SAME_ACCESS)) {
    reader = (ReaderStart *)malloc(sizeof(*reader));
    if (reader) {
      reader->pipe = thread_pipe;
      reader->notify = notify;
      g_reader_thread = CreateThread(NULL, 0, reader_thread, reader, 0, NULL);
      if (!g_reader_thread) {
        free(reader);
        CloseHandle(thread_pipe);
      }
    } else {
      CloseHandle(thread_pipe);
    }
  }

cleanup:
  if (write_pipe != INVALID_HANDLE_VALUE) {
    CloseHandle(write_pipe);
  }
  if (null_input != INVALID_HANDLE_VALUE) {
    CloseHandle(null_input);
  }
  if (!started && g_pipe_read) {
    CloseHandle(g_pipe_read);
    g_pipe_read = NULL;
  }
  free(command_line);
  free(working_dir);
  return started;
}

BOOL daemon_is_running(void) {
  DWORD code;

  if (!g_process) {
    return FALSE;
  }

  if (WaitForSingleObject(g_process, 0) == WAIT_TIMEOUT) {
    return TRUE;
  }

  GetExitCodeProcess(g_process, &code);
  g_last_exit_code = code;
  CloseHandle(g_process);
  g_process = NULL;
  g_pid = 0;

  /* The process closed its write end, so the reader leaves ReadFile. Reap both
     here, since the window may be about to start the daemon again. */
  if (g_reader_thread) {
    if (WaitForSingleObject(g_reader_thread, 2000) == WAIT_TIMEOUT) {
      CancelSynchronousIo(g_reader_thread);
      WaitForSingleObject(g_reader_thread, 1000);
    }
    CloseHandle(g_reader_thread);
    g_reader_thread = NULL;
  }
  if (g_pipe_read) {
    CloseHandle(g_pipe_read);
    g_pipe_read = NULL;
  }
  return FALSE;
}

void daemon_stop(void) {
  if (g_process) {
    DWORD code = 0;

    /* rola-daemon has no stop command: terminating it is the whole of stopping
       the process this window started. */
    TerminateProcess(g_process, 0);
    WaitForSingleObject(g_process, 5000);
    GetExitCodeProcess(g_process, &code);
    g_last_exit_code = code;
    CloseHandle(g_process);
    g_process = NULL;
    g_pid = 0;
  }

  if (g_reader_thread) {
    if (WaitForSingleObject(g_reader_thread, 2000) == WAIT_TIMEOUT) {
      CancelSynchronousIo(g_reader_thread);
      WaitForSingleObject(g_reader_thread, 1000);
    }
    CloseHandle(g_reader_thread);
    g_reader_thread = NULL;
  }

  if (g_pipe_read) {
    /* The reader does not own this handle: close it after the wait so a thread
       still inside ReadFile is released rather than left holding a dead handle.
     */
    CloseHandle(g_pipe_read);
    g_pipe_read = NULL;
  }
}

DWORD daemon_pid(void) { return g_pid; }

DWORD daemon_last_exit_code(void) { return g_last_exit_code; }
