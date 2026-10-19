#pragma once

#include <windows.h>

#include "util.h"

typedef struct RegistryEntry {
  WCHAR *namespace_name;
  WCHAR *path;
  BOOL checked;
  BOOL reachable;
} RegistryEntry;

typedef struct RegistryList {
  RegistryEntry *items;
  size_t count;
} RegistryList;

void registry_list_init(RegistryList *list);
void registry_list_free(RegistryList *list);

BOOL registry_is_namespace(const WCHAR *name);
long registry_find(const RegistryList *list, const WCHAR *namespace_name);

BOOL registry_list_load(const WCHAR *daemon_exe, RegistryList *list,
                        WCHAR *error, size_t error_cap);
BOOL registry_check(const WCHAR *daemon_exe, RegistryList *list, WCHAR *error,
                    size_t error_cap);

BOOL registry_register(const WCHAR *daemon_exe, const WCHAR *namespace_name,
                       const WCHAR *path, WCHAR *message, size_t message_cap);
BOOL registry_remove(const WCHAR *daemon_exe, const WCHAR *namespace_name,
                     WCHAR *message, size_t message_cap);
BOOL registry_update(const WCHAR *daemon_exe, const WCHAR *old_namespace,
                     const WCHAR *new_namespace, const WCHAR *new_path,
                     WCHAR *message, size_t message_cap);
