#pragma once

#include <glib.h>
#include <stddef.h>

typedef struct AdminList {
  char **items;
  size_t count;
} AdminList;

void admin_list_init(AdminList *list);
void admin_list_free(AdminList *list);

gboolean admins_load(const char *rola_exe, const char *vault_path,
                     AdminList *list, char *error, size_t error_cap);
gboolean admin_add(const char *rola_exe, const char *vault_path,
                   const char *name, char *message, size_t message_cap);
gboolean admin_remove(const char *rola_exe, const char *vault_path,
                      const char *name, char *message, size_t message_cap);
