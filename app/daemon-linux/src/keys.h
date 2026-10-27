#pragma once

#include <glib.h>
#include <gtk/gtk.h>
#include <stddef.h>

typedef struct KeyEntry {
  char *path;
  gboolean private_key;
} KeyEntry;

typedef struct KeyList {
  KeyEntry *items;
  size_t count;
} KeyList;

void key_list_init(KeyList *list);
void key_list_free(KeyList *list);

gboolean key_is_valid_name(const char *name);
gboolean keys_load(const char *rola_exe, KeyList *list, char *error,
                   size_t error_cap);
gboolean keys_import_files(GtkWindow *owner, const char **paths, size_t count,
                           char *message, size_t message_cap);
