#pragma once

#include <windows.h>

#include "util.h"

typedef struct AdminList {
  WCHAR **items;
  size_t count;
} AdminList;

void admin_list_init(AdminList *list);
void admin_list_free(AdminList *list);

BOOL admins_load(const WCHAR *rola_exe, const WCHAR *vault_path,
                 AdminList *list, WCHAR *error, size_t error_cap);
BOOL admin_add(const WCHAR *rola_exe, const WCHAR *vault_path,
               const WCHAR *name, WCHAR *message, size_t message_cap);
BOOL admin_remove(const WCHAR *rola_exe, const WCHAR *vault_path,
                  const WCHAR *name, WCHAR *message, size_t message_cap);
