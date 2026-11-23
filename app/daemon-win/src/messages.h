#pragma once

#include <windows.h>

/* Carries a heap-allocated UTF-8 chunk of the daemon's output; the window frees
 * it. */
#define WM_APP_DAEMON_OUTPUT (WM_APP + 1)

/* Carries a heap-allocated WCHAR status line; the window frees it. */
#define WM_APP_SET_STATUS (WM_APP + 2)

/* Asks the Daemon page to press Start once, after the window is drawn. */
#define WM_APP_START (WM_APP + 3)
