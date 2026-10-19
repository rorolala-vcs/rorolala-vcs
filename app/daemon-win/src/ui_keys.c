#include "ui.h"

#include <commdlg.h>
#include <shellapi.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strsafe.h>
#include <wchar.h>

#include "keys.h"
#include "resource.h"

static HWND g_page;
static HWND g_list;
static HWND g_dir_label;
static KeyList g_keys;

static long selected_index(void) {
  LRESULT selected = SendMessageW(g_list, LB_GETCURSEL, 0, 0);

  if (selected == LB_ERR || (size_t)selected >= g_keys.count) {
    return -1;
  }
  return (long)selected;
}

void keys_page_reload(void) {
  WCHAR error[4096];
  WCHAR *directory;
  size_t i;

  directory = keys_dir();
  SetWindowTextW(g_dir_label, directory ? directory : L"");
  free(directory);

  if (!g_app.rola_exe[0]) {
    SendMessageW(g_list, LB_RESETCONTENT, 0, 0);
    key_list_free(&g_keys);
    app_set_status(
        L"rola.exe was not found next to rola-daemon.exe or on PATH.");
    return;
  }

  if (!keys_load(g_app.rola_exe, &g_keys, error, ARRAY_COUNT(error))) {
    SendMessageW(g_list, LB_RESETCONTENT, 0, 0);
    app_set_statusf(L"Could not list keys: %s", error);
    return;
  }

  SendMessageW(g_list, LB_RESETCONTENT, 0, 0);
  for (i = 0; i < g_keys.count; ++i) {
    WCHAR line[PATH_CCH + 64];
    _snwprintf_s(line, ARRAY_COUNT(line), _TRUNCATE, L"[%s] %s",
                 g_keys.items[i].private_key ? L"private" : L"public",
                 path_filename(g_keys.items[i].path));
    SendMessageW(g_list, LB_ADDSTRING, 0, (LPARAM)line);
  }

  if (g_keys.count == 0) {
    app_set_status(L"No keys found. Use Import Keys to add a .pem or .pub.");
  } else {
    app_set_statusf(L"%llu key(s) in the user key directory.",
                    (unsigned long long)g_keys.count);
  }
}

static void add_path(WCHAR ***paths, size_t *count, const WCHAR *path) {
  WCHAR **grown = (WCHAR **)realloc(*paths, (*count + 1) * sizeof(WCHAR *));

  if (!grown) {
    return;
  }
  *paths = grown;
  (*paths)[*count] = str_dup(path);
  if ((*paths)[*count]) {
    ++*count;
  }
}

static void pick_and_import(void) {
  OPENFILENAMEW dialog;
  static WCHAR buffer[65536];
  WCHAR **paths = NULL;
  size_t count = 0;
  WCHAR message[4096];

  ZeroMemory(buffer, sizeof(buffer));
  ZeroMemory(&dialog, sizeof(dialog));

  dialog.lStructSize = sizeof(dialog);
  dialog.hwndOwner = g_page;
  dialog.lpstrFilter =
      L"Key files (*.pem;*.pub)\0*.pem;*.pub\0Private keys (*.pem)\0*.pem\0"
      L"Public keys (*.pub)\0*.pub\0All files (*.*)\0*.*\0\0";
  dialog.lpstrFile = buffer;
  dialog.nMaxFile = (DWORD)ARRAY_COUNT(buffer);
  dialog.lpstrTitle = L"Import keys";
  dialog.Flags = OFN_ALLOWMULTISELECT | OFN_EXPLORER | OFN_FILEMUSTEXIST |
                 OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;

  if (!GetOpenFileNameW(&dialog)) {
    return;
  }

  /* GetOpenFileName writes a double-NUL-terminated list here; the analyzer does
     not model that contract, so the two warnings about the end of the buffer
     are off. */
#pragma warning(push)
#pragma warning(disable : 6054 6385)
  {
    WCHAR *first = buffer;
    size_t first_length = 0;

    if (FAILED(StringCchLengthW(first, ARRAY_COUNT(buffer), &first_length))) {
      return;
    }

    if (first_length + 1 < ARRAY_COUNT(buffer) &&
        first[first_length + 1] == L'\0') {
      add_path(&paths, &count, first);
    } else {
      WCHAR *name = first + first_length + 1;
      while ((size_t)(name - buffer) < ARRAY_COUNT(buffer)) {
        size_t remaining = ARRAY_COUNT(buffer) - (size_t)(name - buffer);
        size_t name_length = 0;
        WCHAR *full;

        if (FAILED(StringCchLengthW(name, remaining, &name_length))) {
          break;
        }

        if (name_length == 0 || name_length == remaining) {
          break;
        }

        full = path_join(first, name);
        if (full) {
          add_path(&paths, &count, full);
          free(full);
        }
        name += name_length + 1;
      }
    }
  }
#pragma warning(pop)

  if (count == 0) {
    return;
  }

  if (keys_import_files(g_page, (const WCHAR **)paths, count, message,
                        ARRAY_COUNT(message))) {
    show_info(g_page, L"%s", message);
    app_set_statusf(L"Imported %llu key file(s).", (unsigned long long)count);
  } else {
    show_error(g_page, L"%s", message);
    app_set_status(L"Some key files were not imported.");
  }

  {
    size_t i;
    for (i = 0; i < count; ++i) {
      free(paths[i]);
    }
    free(paths);
  }

  keys_page_reload();
}

static void open_key_folder(void) {
  WCHAR *directory = keys_dir();
  HINSTANCE result;

  if (!directory) {
    show_error(g_page, L"The local data directory could not be found.");
    return;
  }

  if (!ensure_directory(directory)) {
    show_error(g_page, L"The key directory could not be created:\r\n%s",
               directory);
    free(directory);
    return;
  }

  result = ShellExecuteW(g_page, L"open", directory, NULL, NULL, SW_SHOWNORMAL);
  if ((INT_PTR)result <= 32) {
    show_error(g_page, L"The key directory could not be opened:\r\n%s",
               directory);
  }
  free(directory);
}

static void reveal_selected(void) {
  long index = selected_index();
  WCHAR parameters[PATH_CCH + 32];

  if (index < 0) {
    return;
  }

  _snwprintf_s(parameters, ARRAY_COUNT(parameters), _TRUNCATE,
               L"/select,\"%s\"", g_keys.items[index].path);
  ShellExecuteW(g_page, L"open", L"explorer.exe", parameters, NULL,
                SW_SHOWNORMAL);
}

static INT_PTR CALLBACK KeysPageProcImpl(HWND dialog, UINT message,
                                         WPARAM wparam, LPARAM lparam) {
  (void)lparam;

  switch (message) {
  case WM_INITDIALOG:
    g_page = dialog;
    g_list = GetDlgItem(dialog, IDC_KEY_LIST);
    g_dir_label = GetDlgItem(dialog, IDC_KEY_DIR);
    key_list_init(&g_keys);
    return TRUE;

  case WM_COMMAND:
    switch (LOWORD(wparam)) {
    case IDC_KEY_IMPORT:
      pick_and_import();
      return TRUE;
    case IDC_KEY_REFRESH:
      keys_page_reload();
      return TRUE;
    case IDC_KEY_OPEN:
      open_key_folder();
      return TRUE;
    case IDC_KEY_LIST:
      if (HIWORD(wparam) == LBN_SELCHANGE) {
        long index = selected_index();
        if (index >= 0) {
          app_set_status(g_keys.items[index].path);
        }
        return TRUE;
      }
      if (HIWORD(wparam) == LBN_DBLCLK) {
        reveal_selected();
        return TRUE;
      }
      return FALSE;
    default:
      return FALSE;
    }

  case WM_DESTROY:
    key_list_free(&g_keys);
    g_page = NULL;
    g_list = NULL;
    g_dir_label = NULL;
    return TRUE;

  default:
    return FALSE;
  }
}

INT_PTR CALLBACK KeysPageProc(HWND dialog, UINT message, WPARAM wparam,
                              LPARAM lparam) {
  return KeysPageProcImpl(dialog, message, wparam, lparam);
}
