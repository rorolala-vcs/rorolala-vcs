#pragma once

#include <windows.h>

#include "util.h"

BOOL daemon_start(HWND notify, const WCHAR *exe, const WCHAR *arguments,
                  WCHAR *error, size_t error_cap);
void daemon_stop(void);
BOOL daemon_is_running(void);
DWORD daemon_pid(void);
DWORD daemon_last_exit_code(void);
