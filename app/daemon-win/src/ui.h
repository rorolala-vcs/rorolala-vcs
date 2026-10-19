#pragma once

#include <windows.h>

#include "util.h"

typedef struct AppState {
  HINSTANCE instance;
  HWND main;
  HWND tabs;
  HWND status;
  HWND pages[4];
  WCHAR daemon_exe[PATH_CCH];
  WCHAR rola_exe[PATH_CCH];
} AppState;

extern AppState g_app;

void app_set_status(const WCHAR *text);
void app_set_statusf(const WCHAR *format, ...);
void app_refresh_rola_path(void);
BOOL app_daemon_file_exists(void);
void app_fail_missing_daemon(void);

INT_PTR CALLBACK DaemonPageProc(HWND dialog, UINT message, WPARAM wparam,
                                LPARAM lparam);
INT_PTR CALLBACK RegistryPageProc(HWND dialog, UINT message, WPARAM wparam,
                                  LPARAM lparam);
INT_PTR CALLBACK KeysPageProc(HWND dialog, UINT message, WPARAM wparam,
                              LPARAM lparam);
INT_PTR CALLBACK AdminsPageProc(HWND dialog, UINT message, WPARAM wparam,
                                LPARAM lparam);

void daemon_page_sync_status(void);
void daemon_page_append_output(const char *utf8);
void registry_page_reload(void);
void keys_page_reload(void);
void admins_page_vaults_changed(void);
