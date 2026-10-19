#include "ui.h"

#include <shellapi.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

#include "admins.h"
#include "keys.h"
#include "registry.h"
#include "resource.h"

static HWND g_page;
static HWND g_combo;
static HWND g_list;
static HWND g_add;
static HWND g_remove;
static HWND g_edit;
static HWND g_note;
static RegistryList g_vaults;
static AdminList g_admins;
static WCHAR g_current_vault[PATH_CCH];

typedef struct NameDialog {
  WCHAR title[128];
  WCHAR name[256];
  WCHAR **suggestions;
  size_t suggestion_count;
} NameDialog;

static void update_buttons(void) {
  BOOL has_vault = g_current_vault[0] != L'\0';
  BOOL has_rola = g_app.rola_exe[0] != L'\0';
  BOOL has_selection = SendMessageW(g_list, LB_GETCURSEL, 0, 0) != LB_ERR;

  /* A Vault with no administrator has nobody who may name one; the first is
     written by hand through Edit vault.toml. */
  EnableWindow(g_add, has_vault && has_rola && g_admins.count > 0);
  EnableWindow(g_remove, has_vault && has_rola && has_selection);
  EnableWindow(g_edit, has_vault);
}

static const WCHAR *selected_vault_path(void) {
  LRESULT selected = SendMessageW(g_combo, CB_GETCURSEL, 0, 0);
  LRESULT data;

  if (selected == CB_ERR) {
    g_current_vault[0] = L'\0';
    return g_current_vault;
  }

  data = SendMessageW(g_combo, CB_GETITEMDATA, (WPARAM)selected, 0);
  if (data == CB_ERR || (size_t)data >= g_vaults.count) {
    g_current_vault[0] = L'\0';
    return g_current_vault;
  }

  str_copy(g_current_vault, ARRAY_COUNT(g_current_vault),
           g_vaults.items[data].path);
  return g_current_vault;
}

static void load_admins(void) {
  const WCHAR *vault = selected_vault_path();
  WCHAR error[4096];
  size_t i;

  SendMessageW(g_list, LB_RESETCONTENT, 0, 0);
  admin_list_free(&g_admins);

  if (!vault[0]) {
    SetWindowTextW(g_note,
                   L"Administrators may move any file in the Vault's layout.");
    update_buttons();
    app_set_status(L"No Vault is selected.");
    return;
  }

  SetWindowTextW(g_note, vault);

  if (!g_app.rola_exe[0]) {
    app_set_status(
        L"rola.exe was not found next to rola-daemon.exe or on PATH.");
    update_buttons();
    return;
  }

  if (!admins_load(g_app.rola_exe, vault, &g_admins, error,
                   ARRAY_COUNT(error))) {
    app_set_statusf(L"Could not read the administrators: %s", error);
    update_buttons();
    return;
  }

  for (i = 0; i < g_admins.count; ++i) {
    SendMessageW(g_list, LB_ADDSTRING, 0, (LPARAM)g_admins.items[i]);
  }
  if (g_admins.count > 0) {
    SendMessageW(g_list, LB_SETCURSEL, 0, 0);
  }

  if (g_admins.count == 0) {
    app_set_status(L"No administrators. Write the first one in vault.toml.");
  } else {
    app_set_statusf(L"%llu administrator(s).",
                    (unsigned long long)g_admins.count);
  }
  update_buttons();
}

void admins_page_vaults_changed(void) {
  WCHAR previous[256];
  WCHAR error[4096];
  size_t i;
  long next = 0;

  previous[0] = L'\0';
  GetWindowTextW(g_combo, previous, (int)ARRAY_COUNT(previous));

  SendMessageW(g_combo, CB_RESETCONTENT, 0, 0);
  registry_list_free(&g_vaults);

  if (!registry_list_load(g_app.daemon_exe, &g_vaults, error,
                          ARRAY_COUNT(error))) {
    SendMessageW(g_list, LB_RESETCONTENT, 0, 0);
    admin_list_free(&g_admins);
    g_current_vault[0] = L'\0';
    SetWindowTextW(g_note, L"Register a Vault on the Registry tab first.");
    update_buttons();
    app_set_statusf(L"Could not read the registry: %s", error);
    return;
  }

  for (i = 0; i < g_vaults.count; ++i) {
    LRESULT index = SendMessageW(g_combo, CB_ADDSTRING, 0,
                                 (LPARAM)g_vaults.items[i].namespace_name);
    if (index != CB_ERR) {
      SendMessageW(g_combo, CB_SETITEMDATA, (WPARAM)index, (LPARAM)i);
    }
  }

  for (i = 0; i < g_vaults.count; ++i) {
    if (previous[0] &&
        wcscmp(previous, g_vaults.items[i].namespace_name) == 0) {
      next = (long)i;
      break;
    }
  }

  if (g_vaults.count > 0) {
    SendMessageW(g_combo, CB_SETCURSEL, (WPARAM)next, 0);
  }

  load_admins();
}

static BOOL suggest_names(NameDialog *dialog) {
  KeyList keys;
  WCHAR error[4096];
  size_t i;
  size_t count = 0;

  if (!g_app.rola_exe[0]) {
    return TRUE;
  }

  key_list_init(&keys);
  if (!keys_load(g_app.rola_exe, &keys, error, ARRAY_COUNT(error))) {
    key_list_free(&keys);
    return TRUE;
  }

  for (i = 0; i < keys.count; ++i) {
    WCHAR *stem;
    WCHAR **grown;

    if (keys.items[i].private_key) {
      continue;
    }

    stem = path_stem(keys.items[i].path);
    if (!stem || !key_is_valid_name(stem)) {
      free(stem);
      continue;
    }

    grown =
        (WCHAR **)realloc(dialog->suggestions, (count + 1) * sizeof(WCHAR *));
    if (!grown) {
      free(stem);
      break;
    }
    dialog->suggestions = grown;
    dialog->suggestions[count++] = stem;
  }

  dialog->suggestion_count = count;
  key_list_free(&keys);
  return TRUE;
}

static void free_suggestions(NameDialog *dialog) {
  size_t i;

  for (i = 0; i < dialog->suggestion_count; ++i) {
    free(dialog->suggestions[i]);
  }
  free(dialog->suggestions);
  dialog->suggestions = NULL;
  dialog->suggestion_count = 0;
}

static INT_PTR CALLBACK NameDialogProc(HWND dialog, UINT message, WPARAM wparam,
                                       LPARAM lparam) {
  NameDialog *state = (NameDialog *)GetWindowLongPtrW(dialog, DWLP_USER);

  switch (message) {
  case WM_INITDIALOG:
    state = (NameDialog *)lparam;
    SetWindowLongPtrW(dialog, DWLP_USER, (LONG_PTR)state);
    SetWindowTextW(dialog, state->title);
    if (state) {
      size_t i;
      for (i = 0; i < state->suggestion_count; ++i) {
        SendMessageW(GetDlgItem(dialog, IDC_NAME_COMBO), CB_ADDSTRING, 0,
                     (LPARAM)state->suggestions[i]);
      }
      SetDlgItemTextW(dialog, IDC_NAME_COMBO, state->name);
    }
    SetFocus(GetDlgItem(dialog, IDC_NAME_COMBO));
    return TRUE;

  case WM_COMMAND:
    switch (LOWORD(wparam)) {
    case IDOK:
      if (state) {
        WCHAR name[256];
        WCHAR *trimmed;

        GetDlgItemTextW(dialog, IDC_NAME_COMBO, name, (int)ARRAY_COUNT(name));
        trimmed = str_trim(name);
        if (!key_is_valid_name(trimmed)) {
          show_error(dialog,
                     L"Please type the name of a member key (its file stem).");
          SetFocus(GetDlgItem(dialog, IDC_NAME_COMBO));
          return TRUE;
        }
        str_copy(state->name, ARRAY_COUNT(state->name), trimmed);
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

static void add_admin(void) {
  NameDialog dialog;
  WCHAR message[4096];
  const WCHAR *vault = selected_vault_path();

  if (!vault[0]) {
    return;
  }

  ZeroMemory(&dialog, sizeof(dialog));
  str_copy(dialog.title, ARRAY_COUNT(dialog.title), L"Add Administrator");
  (void)suggest_names(&dialog);

  if (DialogBoxParamW(g_app.instance, MAKEINTRESOURCEW(IDD_NAME), g_page,
                      NameDialogProc, (LPARAM)&dialog) != TRUE) {
    free_suggestions(&dialog);
    return;
  }

  if (admin_add(g_app.rola_exe, vault, dialog.name, message,
                ARRAY_COUNT(message))) {
    app_set_statusf(L"Added %s.", dialog.name);
  } else {
    show_error(g_page, L"Could not add the administrator:\r\n%s", message);
    app_set_status(L"Adding the administrator failed.");
  }

  free_suggestions(&dialog);
  load_admins();
}

static void remove_admin(void) {
  LRESULT selected = SendMessageW(g_list, LB_GETCURSEL, 0, 0);
  const WCHAR *vault = selected_vault_path();
  WCHAR message[4096];

  if (selected == LB_ERR || (size_t)selected >= g_admins.count || !vault[0]) {
    return;
  }

  if (!ask_yes_no(g_page, L"Stop \"%s\" being an administrator of this Vault?",
                  g_admins.items[selected])) {
    return;
  }

  if (admin_remove(g_app.rola_exe, vault, g_admins.items[selected], message,
                   ARRAY_COUNT(message))) {
    app_set_statusf(L"Removed %s.", g_admins.items[selected]);
  } else {
    show_error(g_page, L"Could not remove the administrator:\r\n%s", message);
    app_set_status(L"Removing the administrator failed.");
  }

  load_admins();
}

static void edit_vault_config(void) {
  const WCHAR *vault = selected_vault_path();
  WCHAR *file;
  StrBuf parameters;
  HINSTANCE result;

  if (!vault[0]) {
    return;
  }

  file = path_join(vault, L"vault.toml");
  if (!file || !path_exists(file)) {
    show_error(g_page, L"vault.toml was not found at:\r\n%s",
               file ? file : vault);
    free(file);
    return;
  }

  sb_init(&parameters);
  sb_append_char(&parameters, L'"');
  sb_append(&parameters, file);
  sb_append_char(&parameters, L'"');

  result = ShellExecuteW(g_page, L"open", L"notepad.exe", parameters.data, NULL,
                         SW_SHOWNORMAL);
  if ((INT_PTR)result <= 32) {
    show_error(g_page, L"vault.toml could not be opened:\r\n%s", file);
  } else {
    app_set_status(L"vault.toml opened. Refresh after saving.");
  }

  sb_free(&parameters);
  free(file);
}

static INT_PTR CALLBACK AdminsPageProcImpl(HWND dialog, UINT message,
                                           WPARAM wparam, LPARAM lparam) {
  (void)lparam;

  switch (message) {
  case WM_INITDIALOG:
    g_page = dialog;
    g_combo = GetDlgItem(dialog, IDC_ADMIN_VAULT);
    g_list = GetDlgItem(dialog, IDC_ADMIN_LIST);
    g_add = GetDlgItem(dialog, IDC_ADMIN_ADD);
    g_remove = GetDlgItem(dialog, IDC_ADMIN_REMOVE);
    g_edit = GetDlgItem(dialog, IDC_ADMIN_EDIT);
    g_note = GetDlgItem(dialog, IDC_ADMIN_NOTE);
    registry_list_init(&g_vaults);
    admin_list_init(&g_admins);
    g_current_vault[0] = L'\0';
    update_buttons();
    return TRUE;

  case WM_COMMAND:
    switch (LOWORD(wparam)) {
    case IDC_ADMIN_REFRESH:
      admins_page_vaults_changed();
      return TRUE;
    case IDC_ADMIN_ADD:
      add_admin();
      return TRUE;
    case IDC_ADMIN_REMOVE:
      remove_admin();
      return TRUE;
    case IDC_ADMIN_EDIT:
      edit_vault_config();
      return TRUE;
    case IDC_ADMIN_VAULT:
      if (HIWORD(wparam) == CBN_SELCHANGE) {
        load_admins();
        return TRUE;
      }
      return FALSE;
    case IDC_ADMIN_LIST:
      if (HIWORD(wparam) == LBN_SELCHANGE) {
        update_buttons();
        return TRUE;
      }
      return FALSE;
    default:
      return FALSE;
    }

  case WM_DESTROY:
    admin_list_free(&g_admins);
    registry_list_free(&g_vaults);
    g_page = NULL;
    g_combo = NULL;
    g_list = NULL;
    g_add = NULL;
    g_remove = NULL;
    g_edit = NULL;
    g_note = NULL;
    return TRUE;

  default:
    return FALSE;
  }
}

INT_PTR CALLBACK AdminsPageProc(HWND dialog, UINT message, WPARAM wparam,
                                LPARAM lparam) {
  return AdminsPageProcImpl(dialog, message, wparam, lparam);
}
