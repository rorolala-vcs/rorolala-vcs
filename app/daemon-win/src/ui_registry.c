#include "ui.h"

#include <shellapi.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

#include "registry.h"
#include "resource.h"

static HWND g_page;
static HWND g_list;
static HWND g_path;
static HWND g_edit;
static HWND g_delete;
static HWND g_browse;
static RegistryList g_registry;

typedef struct EntryDialog {
  WCHAR title[128];
  WCHAR namespace_name[256];
  WCHAR path[PATH_CCH];
} EntryDialog;

static long selected_index(void) {
  LRESULT selected = SendMessageW(g_list, LB_GETCURSEL, 0, 0);

  if (selected == LB_ERR || (size_t)selected >= g_registry.count) {
    return -1;
  }
  return (long)selected;
}

static void update_path_edit(void) {
  long index = selected_index();

  if (index >= 0) {
    SetWindowTextW(g_path, g_registry.items[index].path);
  } else {
    SetWindowTextW(g_path, L"");
  }

  EnableWindow(g_edit, index >= 0);
  EnableWindow(g_delete, index >= 0);
  EnableWindow(g_browse, index >= 0);
}

static void fill_list(void) {
  size_t i;

  SendMessageW(g_list, LB_RESETCONTENT, 0, 0);
  for (i = 0; i < g_registry.count; ++i) {
    WCHAR line[PATH_CCH + 320];

    if (g_registry.items[i].checked) {
      _snwprintf_s(line, ARRAY_COUNT(line), _TRUNCATE, L"%s %s  -  %s",
                   g_registry.items[i].reachable ? L"[OK]" : L"[FAIL]",
                   g_registry.items[i].namespace_name,
                   g_registry.items[i].path);
    } else {
      _snwprintf_s(line, ARRAY_COUNT(line), _TRUNCATE, L"%s  -  %s",
                   g_registry.items[i].namespace_name,
                   g_registry.items[i].path);
    }
    SendMessageW(g_list, LB_ADDSTRING, 0, (LPARAM)line);
  }
}

static void reload(BOOL report) {
  WCHAR error[4096];
  WCHAR previous[256];
  long selected;
  size_t i;
  long next = 0;

  previous[0] = L'\0';
  selected = selected_index();
  if (selected >= 0) {
    str_copy(previous, ARRAY_COUNT(previous),
             g_registry.items[selected].namespace_name);
  }

  if (!registry_list_load(g_app.daemon_exe, &g_registry, error,
                          ARRAY_COUNT(error))) {
    SendMessageW(g_list, LB_RESETCONTENT, 0, 0);
    SetWindowTextW(g_path, L"");
    update_path_edit();
    app_set_statusf(L"Could not read the registry: %s", error);
    if (report) {
      show_error(g_page, L"Could not read the registry:\r\n%s", error);
    }
    return;
  }

  fill_list();

  for (i = 0; i < g_registry.count; ++i) {
    if (previous[0] &&
        wcscmp(previous, g_registry.items[i].namespace_name) == 0) {
      next = (long)i;
      break;
    }
  }
  if (g_registry.count > 0) {
    SendMessageW(g_list, LB_SETCURSEL, (WPARAM)next, 0);
  }
  update_path_edit();

  if (g_registry.count == 0) {
    app_set_status(L"Registry is empty. Use New to serve a Vault.");
  } else {
    app_set_statusf(L"%llu namespace(s) registered.",
                    (unsigned long long)g_registry.count);
  }
}

void registry_page_reload(void) { reload(FALSE); }

static INT_PTR CALLBACK EntryDialogProc(HWND dialog, UINT message,
                                        WPARAM wparam, LPARAM lparam) {
  EntryDialog *state = (EntryDialog *)GetWindowLongPtrW(dialog, DWLP_USER);

  switch (message) {
  case WM_INITDIALOG:
    state = (EntryDialog *)lparam;
    SetWindowLongPtrW(dialog, DWLP_USER, (LONG_PTR)state);
    SetWindowTextW(dialog, state->title);
    SetDlgItemTextW(dialog, IDC_ENTRY_NS, state->namespace_name);
    SetDlgItemTextW(dialog, IDC_ENTRY_PATH, state->path);
    SetFocus(GetDlgItem(dialog, IDC_ENTRY_NS));
    return TRUE;

  case WM_COMMAND:
    switch (LOWORD(wparam)) {
    case IDC_ENTRY_BROWSE:
      if (state) {
        WCHAR folder[PATH_CCH];
        if (choose_folder(dialog, L"Select the Vault folder", folder,
                          ARRAY_COUNT(folder))) {
          SetDlgItemTextW(dialog, IDC_ENTRY_PATH, folder);
          if (!GetDlgItemTextW(dialog, IDC_ENTRY_NS, state->namespace_name,
                               (int)ARRAY_COUNT(state->namespace_name)) &&
              path_filename(folder)[0]) {
            WCHAR stem[256];
            str_copy(stem, ARRAY_COUNT(stem), path_filename(folder));
            SetDlgItemTextW(dialog, IDC_ENTRY_NS, stem);
          }
        }
      }
      return TRUE;

    case IDOK:
      if (state) {
        WCHAR namespace_name[256];
        WCHAR path[PATH_CCH];
        WCHAR *trimmed;
        WCHAR *config;
        BOOL is_vault;

        GetDlgItemTextW(dialog, IDC_ENTRY_NS, namespace_name,
                        (int)ARRAY_COUNT(namespace_name));
        GetDlgItemTextW(dialog, IDC_ENTRY_PATH, path, (int)ARRAY_COUNT(path));

        trimmed = str_trim(namespace_name);
        if (!registry_is_namespace(trimmed)) {
          show_error(
              dialog,
              L"A namespace is letters, digits, '-' and '_', and starts with "
              L"a letter or a digit.");
          SetFocus(GetDlgItem(dialog, IDC_ENTRY_NS));
          return TRUE;
        }

        trimmed = str_trim(path);
        if (!trimmed[0]) {
          show_error(dialog, L"Please choose the Vault folder.");
          SetFocus(GetDlgItem(dialog, IDC_ENTRY_PATH));
          return TRUE;
        }

        config = path_join(trimmed, L"vault.toml");
        is_vault = config && path_exists(config);
        free(config);
        if (!is_vault) {
          show_error(
              dialog,
              L"That folder does not hold a Vault: vault.toml was not found.");
          SetFocus(GetDlgItem(dialog, IDC_ENTRY_PATH));
          return TRUE;
        }

        str_copy(state->namespace_name, ARRAY_COUNT(state->namespace_name),
                 trimmed);
        str_copy(state->path, ARRAY_COUNT(state->path), trimmed);
      }
      EndDialog(dialog, TRUE);
      return TRUE;

    case IDCANCEL:
      EndDialog(dialog, FALSE);
      return TRUE;

    default:
      return FALSE;
    }

  default:
    return FALSE;
  }
}

static BOOL ask_entry(HWND owner, const WCHAR *title,
                      const WCHAR *namespace_name, const WCHAR *path,
                      EntryDialog *state) {
  ZeroMemory(state, sizeof(*state));
  str_copy(state->title, ARRAY_COUNT(state->title), title);
  str_copy(state->namespace_name, ARRAY_COUNT(state->namespace_name),
           namespace_name);
  str_copy(state->path, ARRAY_COUNT(state->path), path);

  return DialogBoxParamW(g_app.instance, MAKEINTRESOURCEW(IDD_ENTRY), owner,
                         EntryDialogProc, (LPARAM)state) == TRUE;
}

static void new_entry(void) {
  EntryDialog state;
  WCHAR message[4096];

  if (!ask_entry(g_page, L"New Registry Entry", L"", L"", &state)) {
    return;
  }

  if (registry_register(g_app.daemon_exe, state.namespace_name, state.path,
                        message, ARRAY_COUNT(message))) {
    app_set_statusf(L"Registered %s.", state.namespace_name);
  } else {
    show_error(g_page, L"Could not register the Vault:\r\n%s", message);
    app_set_status(L"Registration failed.");
  }
  reload(FALSE);
}

static void edit_entry(void) {
  long index = selected_index();
  EntryDialog state;
  WCHAR message[4096];
  WCHAR old_namespace[256];

  if (index < 0) {
    return;
  }

  str_copy(old_namespace, ARRAY_COUNT(old_namespace),
           g_registry.items[index].namespace_name);

  if (!ask_entry(g_page, L"Edit Registry Entry",
                 g_registry.items[index].namespace_name,
                 g_registry.items[index].path, &state)) {
    return;
  }

  if (str_ieq(old_namespace, state.namespace_name) &&
      str_ieq(g_registry.items[index].path, state.path)) {
    return;
  }

  if (registry_update(g_app.daemon_exe, old_namespace, state.namespace_name,
                      state.path, message, ARRAY_COUNT(message))) {
    app_set_statusf(L"Updated %s.", state.namespace_name);
  } else {
    show_error(g_page, L"Could not update the entry:\r\n%s", message);
    app_set_status(L"Update failed.");
  }
  reload(FALSE);
}

static void delete_entry(void) {
  long index = selected_index();
  WCHAR message[4096];

  if (index < 0) {
    return;
  }

  if (!ask_yes_no(g_page,
                  L"Remove the registry row \"%s\"?\r\n\r\nThe Vault stays "
                  L"where it is.",
                  g_registry.items[index].namespace_name)) {
    return;
  }

  if (registry_remove(g_app.daemon_exe, g_registry.items[index].namespace_name,
                      message, ARRAY_COUNT(message))) {
    app_set_statusf(L"Removed %s.", g_registry.items[index].namespace_name);
  } else {
    show_error(g_page, L"Could not remove the entry:\r\n%s", message);
    app_set_status(L"Removal failed.");
  }
  reload(FALSE);
}

static void browse_selected(void) {
  long index = selected_index();
  HINSTANCE result;

  if (index < 0) {
    return;
  }

  result = ShellExecuteW(g_page, L"open", g_registry.items[index].path, NULL,
                         NULL, SW_SHOWNORMAL);
  if ((INT_PTR)result <= 32) {
    show_error(g_page, L"The folder could not be opened:\r\n%s",
               g_registry.items[index].path);
  }
}

static void edit_text(void) {
  WCHAR *file = registry_file();
  WCHAR *folder = path_dirname(file);
  HINSTANCE result;
  StrBuf parameters;

  if (!file) {
    show_error(g_page, L"The local data directory could not be found.");
    return;
  }

  if (!ensure_directory(folder)) {
    show_error(g_page, L"The registry folder could not be created:\r\n%s",
               folder);
    free(folder);
    free(file);
    return;
  }
  free(folder);

  if (!path_exists(file)) {
    HANDLE created = CreateFileW(file, GENERIC_WRITE, 0, NULL, CREATE_ALWAYS,
                                 FILE_ATTRIBUTE_NORMAL, NULL);
    if (created != INVALID_HANDLE_VALUE) {
      CloseHandle(created);
    }
  }

  sb_init(&parameters);
  sb_append_char(&parameters, L'"');
  sb_append(&parameters, file);
  sb_append_char(&parameters, L'"');

  result = ShellExecuteW(g_page, L"open", L"notepad.exe", parameters.data, NULL,
                         SW_SHOWNORMAL);
  if ((INT_PTR)result <= 32) {
    result = ShellExecuteW(g_page, L"open", file, NULL, NULL, SW_SHOWNORMAL);
  }
  if ((INT_PTR)result <= 32) {
    show_error(g_page, L"The registry file could not be opened:\r\n%s", file);
  } else {
    app_set_status(
        L"Registry opened in a text editor. Use Refresh when it is saved.");
  }

  sb_free(&parameters);
  free(file);
}

static void check_registry(void) {
  WCHAR error[4096];
  size_t i;
  size_t reachable = 0;

  if (!registry_check(g_app.daemon_exe, &g_registry, error,
                      ARRAY_COUNT(error))) {
    show_error(g_page, L"Could not check the registry:\r\n%s", error);
    app_set_status(L"Check failed.");
    return;
  }

  fill_list();
  for (i = 0; i < g_registry.count; ++i) {
    if (g_registry.items[i].reachable) {
      ++reachable;
    }
  }
  if (g_registry.count > 0) {
    SendMessageW(g_list, LB_SETCURSEL, 0, 0);
  }
  update_path_edit();
  app_set_statusf(L"%llu of %llu namespace(s) reach a Vault.",
                  (unsigned long long)reachable,
                  (unsigned long long)g_registry.count);
}

static INT_PTR CALLBACK RegistryPageProcImpl(HWND dialog, UINT message,
                                             WPARAM wparam, LPARAM lparam) {
  (void)lparam;

  switch (message) {
  case WM_INITDIALOG:
    g_page = dialog;
    g_list = GetDlgItem(dialog, IDC_REG_LIST);
    g_path = GetDlgItem(dialog, IDC_REG_PATH);
    g_edit = GetDlgItem(dialog, IDC_REG_EDIT);
    g_delete = GetDlgItem(dialog, IDC_REG_DELETE);
    g_browse = GetDlgItem(dialog, IDC_REG_BROWSE);
    registry_list_init(&g_registry);
    update_path_edit();
    return TRUE;

  case WM_COMMAND:
    switch (LOWORD(wparam)) {
    case IDC_REG_NEW:
      new_entry();
      return TRUE;
    case IDC_REG_EDIT:
      edit_entry();
      return TRUE;
    case IDC_REG_DELETE:
      delete_entry();
      return TRUE;
    case IDC_REG_BROWSE:
      browse_selected();
      return TRUE;
    case IDC_REG_EDITTEXT:
      edit_text();
      return TRUE;
    case IDC_REG_REFRESH:
      reload(TRUE);
      return TRUE;
    case IDC_REG_CHECK:
      check_registry();
      return TRUE;
    case IDC_REG_LIST:
      if (HIWORD(wparam) == LBN_SELCHANGE) {
        update_path_edit();
        return TRUE;
      }
      if (HIWORD(wparam) == LBN_DBLCLK) {
        edit_entry();
        return TRUE;
      }
      return FALSE;
    default:
      return FALSE;
    }

  case WM_DESTROY:
    registry_list_free(&g_registry);
    g_page = NULL;
    g_list = NULL;
    g_path = NULL;
    g_edit = NULL;
    g_delete = NULL;
    g_browse = NULL;
    return TRUE;

  default:
    return FALSE;
  }
}

INT_PTR CALLBACK RegistryPageProc(HWND dialog, UINT message, WPARAM wparam,
                                  LPARAM lparam) {
  return RegistryPageProcImpl(dialog, message, wparam, lparam);
}
