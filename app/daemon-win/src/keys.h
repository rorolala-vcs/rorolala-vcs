#pragma once

#include <windows.h>

#include "util.h"

typedef struct KeyEntry {
  WCHAR *path;
  BOOL private_key;
} KeyEntry;

typedef struct KeyList {
  KeyEntry *items;
  size_t count;
} KeyList;

void key_list_init(KeyList *list);
void key_list_free(KeyList *list);

BOOL key_is_valid_name(const WCHAR *name);
BOOL keys_load(const WCHAR *rola_exe, KeyList *list, WCHAR *error,
               size_t error_cap);
BOOL keys_import_files(HWND owner, const WCHAR **paths, size_t count,
                       WCHAR *message, size_t message_cap);
