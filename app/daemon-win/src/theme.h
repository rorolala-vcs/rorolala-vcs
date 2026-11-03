#pragma once

#include <windows.h>

/*
 * The window's own colours, following the desktop where the desktop can say.
 *
 * Windows 10 1809 and later have a dark mode for applications, but it is not one
 * API: the setting is a registry value, the frame is a DWM attribute, and the
 * controls are a pair of undocumented uxtheme exports. Every one of those is
 * allowed to be missing, and the answer when one is is the light look rather than
 * a half-dark window: the program is usable on an older Windows by being light.
 */

/* Reads the desktop's setting and prepares what can be prepared before a window
   exists. Safe to call more than once. */
void theme_init(void);

/*
 * Handles the messages a dialog needs in order to wear the theme, answering
 * whether it did and, when it did, what the dialog procedure has to return.
 *
 * Every dialog procedure calls this before its own switch: the colour messages,
 * the background, the system-change notifications, and the first application of
 * the theme to the controls WM_INITDIALOG has just made.
 */
BOOL theme_message(HWND dialog, UINT message, WPARAM wparam, LPARAM lparam,
                   INT_PTR *answer);

/* Applies the theme in force to a window and every control under it. */
void theme_apply(HWND window);

/* Whether the dark look is the one in force, which the tests and the menu items
   that may want it can read. */
BOOL theme_is_dark(void);
