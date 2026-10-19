#include "ui.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

#include "daemon.h"
#include "process.h"
#include "resource.h"

static HWND g_page;
static HWND g_port;
static HWND g_start;
static HWND g_stop;
static HWND g_status;
static HWND g_log;
static BOOL g_last_running;

static void append_log(const WCHAR *text) {
  int length;

  if (!g_log || !text || !text[0]) {
    return;
  }

  length = GetWindowTextLengthW(g_log);
  SendMessageW(g_log, EM_SETSEL, (WPARAM)length, (LPARAM)length);
  SendMessageW(g_log, EM_REPLACESEL, FALSE, (LPARAM)text);
  SendMessageW(g_log, EM_SCROLLCARET, 0, 0);
}

void daemon_page_append_output(const char *utf8) {
  WCHAR *wide;

  if (!g_log || !utf8) {
    return;
  }

  wide = utf8_to_wide(utf8);
  if (!wide) {
    return;
  }
  append_log(wide);
  free(wide);
}

void daemon_page_sync_status(void) {
  BOOL running;
  DWORD code;

  if (!g_page || !g_status) {
    return;
  }

  running = daemon_is_running();
  if (running) {
    WCHAR text[128];
    _snwprintf_s(text, ARRAY_COUNT(text), _TRUNCATE, L"Running (pid %lu)",
                 (unsigned long)daemon_pid());
    SetWindowTextW(g_status, text);
    if (!g_last_running) {
      app_set_statusf(L"rola-daemon is running (pid %lu).",
                      (unsigned long)daemon_pid());
    }
  } else {
    SetWindowTextW(g_status, L"Stopped");
    if (g_last_running) {
      code = daemon_last_exit_code();
      app_set_statusf(L"rola-daemon stopped (exit code %lu).",
                      (unsigned long)code);
      {
        WCHAR line[128];
        _snwprintf_s(line, ARRAY_COUNT(line), _TRUNCATE,
                     L"\r\nrola-daemon exited with code %lu.\r\n",
                     (unsigned long)code);
        append_log(line);
      }
    }
  }

  g_last_running = running;
  EnableWindow(g_start, !running);
  EnableWindow(g_stop, running);
}

static BOOL parse_port(WCHAR *text, WCHAR *normalized, size_t cap) {
  WCHAR *end = NULL;
  unsigned long value;

  normalized[0] = L'\0';
  text = str_trim(text);
  if (!text[0]) {
    return TRUE;
  }

  value = wcstoul(text, &end, 10);
  if (end == text || (end && *end != L'\0') || value == 0 || value > 65535) {
    return FALSE;
  }

  _snwprintf_s(normalized, cap, _TRUNCATE, L"%lu", value);
  return TRUE;
}

static void start_daemon(void) {
  WCHAR port_text[32];
  WCHAR port[32];
  WCHAR error[4096];
  CmdLine args;

  if (!app_daemon_file_exists()) {
    app_fail_missing_daemon();
    return;
  }

  app_refresh_rola_path();

  GetWindowTextW(g_port, port_text, (int)ARRAY_COUNT(port_text));
  if (!parse_port(port_text, port, ARRAY_COUNT(port))) {
    show_error(g_page,
               L"The port must be a number between 1 and 65535, or blank.");
    SetFocus(g_port);
    return;
  }

  cmd_init(&args);
  if (port[0]) {
    (void)cmd_add(&args, L"--port");
    (void)cmd_add(&args, port);
  }

  append_log(L"Starting rola-daemon...\r\n");
  if (!daemon_start(g_app.main, g_app.daemon_exe, args.text.data, error,
                    ARRAY_COUNT(error))) {
    append_log(error);
    append_log(L"\r\n");
    show_error(g_page, L"%s", error);
    cmd_free(&args);
    daemon_page_sync_status();
    return;
  }

  {
    WCHAR line[256];
    _snwprintf_s(line, ARRAY_COUNT(line), _TRUNCATE,
                 L"Started with pid %lu.\r\n", (unsigned long)daemon_pid());
    append_log(line);
  }
  cmd_free(&args);
  daemon_page_sync_status();
}

static void stop_daemon(void) {
  if (!daemon_is_running()) {
    app_set_status(L"rola-daemon is not running.");
    daemon_page_sync_status();
    return;
  }

  append_log(L"Stopping rola-daemon...\r\n");
  daemon_stop();
  append_log(L"Stopped.\r\n");
  app_set_status(L"rola-daemon stopped.");
  daemon_page_sync_status();
}

static INT_PTR CALLBACK DaemonPageProcImpl(HWND dialog, UINT message,
                                           WPARAM wparam, LPARAM lparam) {
  (void)lparam;

  switch (message) {
  case WM_INITDIALOG:
    g_page = dialog;
    g_port = GetDlgItem(dialog, IDC_DAEMON_PORT);
    g_start = GetDlgItem(dialog, IDC_DAEMON_START);
    g_stop = GetDlgItem(dialog, IDC_DAEMON_STOP);
    g_status = GetDlgItem(dialog, IDC_DAEMON_STATUS);
    g_log = GetDlgItem(dialog, IDC_DAEMON_LOG);

    SetWindowTextW(g_port, L"7717");
    SetWindowTextW(g_status, L"Stopped");
    g_last_running = FALSE;
    daemon_page_sync_status();
    return TRUE;

  case WM_COMMAND:
    switch (LOWORD(wparam)) {
    case IDC_DAEMON_START:
      start_daemon();
      return TRUE;
    case IDC_DAEMON_STOP:
      stop_daemon();
      return TRUE;
    default:
      return FALSE;
    }

  case WM_DESTROY:
    g_page = NULL;
    g_port = NULL;
    g_start = NULL;
    g_stop = NULL;
    g_status = NULL;
    g_log = NULL;
    return TRUE;

  default:
    return FALSE;
  }
}

INT_PTR CALLBACK DaemonPageProc(HWND dialog, UINT message, WPARAM wparam,
                                LPARAM lparam) {
  return DaemonPageProcImpl(dialog, message, wparam, lparam);
}
