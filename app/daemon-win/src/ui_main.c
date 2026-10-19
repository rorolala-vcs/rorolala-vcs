#include "ui.h"

#include <commctrl.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "daemon.h"
#include "messages.h"
#include "resource.h"

AppState g_app;

void app_set_status(const WCHAR *text) {
  if (!g_app.status) {
    return;
  }
  SetWindowTextW(g_app.status, text ? text : L"");
}

void app_set_statusf(const WCHAR *format, ...) {
  StrBuf text;
  va_list args;
  va_list copy;
  int length;

  sb_init(&text);
  va_start(args, format);
  va_copy(copy, args);
  length = _vscwprintf(format, args);
  va_end(args);

  if (length >= 0 && sb_reserve(&text, (size_t)length)) {
    _vsnwprintf_s(text.data, text.cap, _TRUNCATE, format, copy);
    text.len = (size_t)length;
    app_set_status(text.data);
  }
  va_end(copy);
  sb_free(&text);
}

static void fix_daemon_path(void) {
  WCHAR *module = module_dir();
  WCHAR *exe = path_join(module, L"rola-daemon.exe");

  str_copy(g_app.daemon_exe, ARRAY_COUNT(g_app.daemon_exe), exe ? exe : L"");
  free(exe);
  free(module);
}

BOOL app_daemon_file_exists(void) {
  return g_app.daemon_exe[0] && path_exists(g_app.daemon_exe) &&
         !path_is_dir(g_app.daemon_exe);
}

void app_fail_missing_daemon(void) {
  HWND main = g_app.main;

  if (!main) {
    return;
  }

  /* Take the window out of the message path first: the timer may fire again
     while the error box is up, and EndDialog below destroys the pages. */
  g_app.main = NULL;
  KillTimer(main, 1);
  daemon_stop();
  show_error(main,
             L"rola-daemon.exe is missing:\r\n%s\r\n\r\nIt must stay next to "
             L"rola-daemon-gui.exe. The program will close.",
             g_app.daemon_exe);
  EndDialog(main, 1);
}

void app_refresh_rola_path(void) {
  WCHAR *daemon_dir = path_dirname(g_app.daemon_exe);
  WCHAR *module = module_dir();
  WCHAR *candidate;
  WCHAR search[PATH_CCH];

  g_app.rola_exe[0] = L'\0';

  candidate = path_join(daemon_dir, L"rola.exe");
  if (candidate && path_exists(candidate)) {
    str_copy(g_app.rola_exe, ARRAY_COUNT(g_app.rola_exe), candidate);
    free(candidate);
    free(daemon_dir);
    free(module);
    return;
  }
  free(candidate);

  candidate = path_join(module, L"rola.exe");
  if (candidate && path_exists(candidate)) {
    str_copy(g_app.rola_exe, ARRAY_COUNT(g_app.rola_exe), candidate);
    free(candidate);
    free(daemon_dir);
    free(module);
    return;
  }
  free(candidate);

  candidate = path_join(module, L"bin\\rola.exe");
  if (candidate && path_exists(candidate)) {
    str_copy(g_app.rola_exe, ARRAY_COUNT(g_app.rola_exe), candidate);
    free(candidate);
    free(daemon_dir);
    free(module);
    return;
  }
  free(candidate);

  if (SearchPathW(NULL, L"rola.exe", NULL, (DWORD)ARRAY_COUNT(search), search,
                  NULL) > 0) {
    str_copy(g_app.rola_exe, ARRAY_COUNT(g_app.rola_exe), search);
  }

  free(daemon_dir);
  free(module);
}

static void center_window(HWND window) {
  RECT window_rect;
  RECT screen;
  int width;
  int height;
  int x;
  int y;

  GetWindowRect(window, &window_rect);
  screen.left = 0;
  screen.top = 0;
  screen.right = GetSystemMetrics(SM_CXSCREEN);
  screen.bottom = GetSystemMetrics(SM_CYSCREEN);
  width = window_rect.right - window_rect.left;
  height = window_rect.bottom - window_rect.top;
  x = screen.left + (screen.right - screen.left - width) / 2;
  y = screen.top + (screen.bottom - screen.top - height) / 2;
  if (x < 0) {
    x = 0;
  }
  if (y < 0) {
    y = 0;
  }
  SetWindowPos(window, NULL, x, y, 0, 0,
               SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
}

static void layout_pages(void) {
  RECT area;
  int index;

  if (!g_app.tabs) {
    return;
  }

  GetClientRect(g_app.tabs, &area);
  TabCtrl_AdjustRect(g_app.tabs, FALSE, &area);
  MapWindowPoints(g_app.tabs, g_app.main, (POINT *)&area, 2);

  for (index = 0; index < 4; ++index) {
    if (!g_app.pages[index]) {
      continue;
    }
    SetWindowPos(g_app.pages[index], HWND_TOP, area.left, area.top,
                 area.right - area.left, area.bottom - area.top,
                 SWP_NOACTIVATE);
  }
}

static void show_page(int index) {
  int i;

  if (index < 0 || index > 3) {
    return;
  }

  for (i = 0; i < 4; ++i) {
    if (g_app.pages[i]) {
      ShowWindow(g_app.pages[i], i == index ? SW_SHOW : SW_HIDE);
    }
  }

  switch (index) {
  case 0:
    daemon_page_sync_status();
    break;
  case 1:
    registry_page_reload();
    break;
  case 2:
    keys_page_reload();
    break;
  case 3:
    admins_page_vaults_changed();
    break;
  default:
    break;
  }
}

static void create_pages(HWND main) {
  TCITEMW item;
  static const WCHAR *titles[] = {L"Daemon", L"Registry", L"Keys", L"Admins"};
  int index;

  g_app.pages[0] = CreateDialogParamW(
      g_app.instance, MAKEINTRESOURCEW(IDD_DAEMON), main, DaemonPageProc, 0);
  g_app.pages[1] =
      CreateDialogParamW(g_app.instance, MAKEINTRESOURCEW(IDD_REGISTRY), main,
                         RegistryPageProc, 0);
  g_app.pages[2] = CreateDialogParamW(
      g_app.instance, MAKEINTRESOURCEW(IDD_KEYS), main, KeysPageProc, 0);
  g_app.pages[3] = CreateDialogParamW(
      g_app.instance, MAKEINTRESOURCEW(IDD_ADMINS), main, AdminsPageProc, 0);

  ZeroMemory(&item, sizeof(item));
  item.mask = TCIF_TEXT;
  for (index = 0; index < 4; ++index) {
    item.pszText = (LPWSTR)titles[index];
    TabCtrl_InsertItem(g_app.tabs, index, &item);
  }
}

static INT_PTR CALLBACK MainDialogProc(HWND dialog, UINT message, WPARAM wparam,
                                       LPARAM lparam) {
  (void)lparam;

  switch (message) {
  case WM_INITDIALOG:
    g_app.instance =
        (HINSTANCE)(ULONG_PTR)GetWindowLongPtrW(dialog, GWLP_HINSTANCE);
    g_app.main = dialog;
    g_app.tabs = GetDlgItem(dialog, IDC_TABS);
    g_app.status = GetDlgItem(dialog, IDC_MAIN_STATUS);

    center_window(dialog);

    create_pages(dialog);
    layout_pages();
    TabCtrl_SetCurSel(g_app.tabs, 0);
    show_page(0);

    SetTimer(dialog, 1, 750, NULL);
    app_set_status(L"Ready.");
    return TRUE;

  case WM_NOTIFY: {
    NMHDR *header = (NMHDR *)lparam;
    if (header->idFrom == IDC_TABS && header->code == TCN_SELCHANGE) {
      show_page(TabCtrl_GetCurSel(g_app.tabs));
      return TRUE;
    }
    return FALSE;
  }

  case WM_TIMER:
    if (wparam == 1) {
      if (!app_daemon_file_exists()) {
        app_fail_missing_daemon();
        return TRUE;
      }
      daemon_page_sync_status();
    }
    return TRUE;

  case WM_APP_DAEMON_OUTPUT: {
    char *chunk = (char *)lparam;
    if (chunk) {
      daemon_page_append_output(chunk);
      free(chunk);
    }
    return TRUE;
  }

  case WM_APP_SET_STATUS: {
    WCHAR *text = (WCHAR *)lparam;
    if (text) {
      app_set_status(text);
      free(text);
    }
    return TRUE;
  }

  case WM_COMMAND:
    if (LOWORD(wparam) == IDOK || LOWORD(wparam) == IDCANCEL) {
      SendMessageW(dialog, WM_CLOSE, 0, 0);
      return TRUE;
    }
    return FALSE;

  case WM_CLOSE:
    KillTimer(dialog, 1);
    daemon_stop();
    EndDialog(dialog, 0);
    return TRUE;

  default:
    return FALSE;
  }
}

int WINAPI wWinMain(_In_ HINSTANCE instance, _In_opt_ HINSTANCE previous,
                    _In_ LPWSTR command_line, _In_ int show) {
  INITCOMMONCONTROLSEX controls;
  INT_PTR result;

  (void)previous;
  (void)command_line;
  (void)show;

  controls.dwSize = sizeof(controls);
  controls.dwICC = ICC_TAB_CLASSES | ICC_STANDARD_CLASSES | ICC_WIN95_CLASSES;
  InitCommonControlsEx(&controls);

  if (FAILED(CoInitializeEx(NULL, COINIT_APARTMENTTHREADED |
                                      COINIT_DISABLE_OLE1DDE))) {
    return 1;
  }

  fix_daemon_path();
  if (!app_daemon_file_exists()) {
    show_error(
        NULL,
        L"rola-daemon.exe was not found next to rola-daemon-gui.exe.\r\n\r\n"
        L"Put rola-daemon.exe in the same folder and start again.");
    CoUninitialize();
    return 1;
  }
  app_refresh_rola_path();

  result = DialogBoxParamW(instance, MAKEINTRESOURCEW(IDD_MAIN), NULL,
                           MainDialogProc, 0);

  CoUninitialize();
  return result == -1 ? 1 : 0;
}
