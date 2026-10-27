#pragma once

#include <glib.h>
#include <gtk/gtk.h>
#include <stddef.h>

#define PATH_CCH 4096
#define ARRAY_COUNT(a) (sizeof(a) / sizeof((a)[0]))

/* A growable UTF-8 string. Every function that returns gboolean leaves the
   buffer unchanged when it fails. */
typedef struct StrBuf {
  char *data;
  size_t len;
  size_t cap;
} StrBuf;

void sb_init(StrBuf *sb);
void sb_free(StrBuf *sb);
void sb_clear(StrBuf *sb);
gboolean sb_reserve(StrBuf *sb, size_t extra);
gboolean sb_append(StrBuf *sb, const char *text);
gboolean sb_append_n(StrBuf *sb, const char *text, size_t count);
gboolean sb_append_char(StrBuf *sb, char ch);
gboolean sb_appendf(StrBuf *sb, const char *format, ...) G_GNUC_PRINTF(2, 3);
char *sb_detach(StrBuf *sb);

char *str_dup(const char *text);
gboolean str_ieq(const char *left, const char *right);
gboolean str_iends_with(const char *text, const char *suffix);
char *str_trim(char *text);
gboolean str_copy(char *target, size_t cap, const char *source);
char *path_join(const char *left, const char *right);
char *path_dirname(const char *path);
const char *path_filename(const char *path);
char *path_stem(const char *path);
const char *path_extension(const char *path);
gboolean path_exists(const char *path);
gboolean path_is_dir(const char *path);
gboolean ensure_directory(const char *path);

char *module_dir(void);
char *data_home(void);
char *keys_dir(void);
char *registry_file(void);
char *find_program(const char *name);

gboolean open_path(const char *path);

void show_error(GtkWindow *owner, const char *format, ...) G_GNUC_PRINTF(2, 3);
void show_info(GtkWindow *owner, const char *format, ...) G_GNUC_PRINTF(2, 3);
gboolean ask_yes_no(GtkWindow *owner, const char *format, ...)
    G_GNUC_PRINTF(2, 3);
gboolean choose_folder(GtkWindow *owner, const char *title, char *out,
                       size_t cap);

/* The chosen files as UTF-8 paths, owned by the array; NULL when nothing was
   chosen. `filter` may be NULL, which offers every file. */
GPtrArray *choose_files(GtkWindow *owner, const char *title,
                        GtkFileFilter *filter);
