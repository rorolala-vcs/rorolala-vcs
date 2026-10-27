#pragma once

#include <glib.h>
#include <stddef.h>

typedef struct RegistryEntry {
  char *namespace_name;
  char *path;
  gboolean checked;
  gboolean reachable;
} RegistryEntry;

typedef struct RegistryList {
  RegistryEntry *items;
  size_t count;
} RegistryList;

void registry_list_init(RegistryList *list);
void registry_list_free(RegistryList *list);

gboolean registry_is_namespace(const char *name);
long registry_find(const RegistryList *list, const char *namespace_name);

gboolean registry_list_load(const char *daemon_exe, RegistryList *list,
                            char *error, size_t error_cap);
gboolean registry_check(const char *daemon_exe, RegistryList *list, char *error,
                        size_t error_cap);

gboolean registry_register(const char *daemon_exe, const char *namespace_name,
                           const char *path, char *message, size_t message_cap);
gboolean registry_remove(const char *daemon_exe, const char *namespace_name,
                         char *message, size_t message_cap);
gboolean registry_update(const char *daemon_exe, const char *old_namespace,
                         const char *new_namespace, const char *new_path,
                         char *message, size_t message_cap);
