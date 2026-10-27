#include "util.h"

#include <glib/gstdio.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static gboolean sb_grow(StrBuf *sb, size_t needed) {
  size_t cap;
  char *grown;

  if (needed <= sb->cap) {
    return TRUE;
  }

  cap = sb->cap ? sb->cap : 64;
  while (cap < needed) {
    if (cap > G_MAXSIZE / 2) {
      return FALSE;
    }
    cap *= 2;
  }

  grown = g_try_realloc(sb->data, cap);
  if (!grown) {
    return FALSE;
  }

  sb->data = grown;
  sb->cap = cap;
  return TRUE;
}

void sb_init(StrBuf *sb) {
  sb->data = NULL;
  sb->len = 0;
  sb->cap = 0;
}

void sb_free(StrBuf *sb) {
  g_free(sb->data);
  sb_init(sb);
}

void sb_clear(StrBuf *sb) {
  sb->len = 0;
  if (sb->data) {
    sb->data[0] = '\0';
  }
}

gboolean sb_reserve(StrBuf *sb, size_t extra) {
  if (extra > G_MAXSIZE - sb->len - 1) {
    return FALSE;
  }
  return sb_grow(sb, sb->len + extra + 1);
}

gboolean sb_append_n(StrBuf *sb, const char *text, size_t count) {
  if (!sb_reserve(sb, count)) {
    return FALSE;
  }

  memcpy(sb->data + sb->len, text, count);
  sb->len += count;
  sb->data[sb->len] = '\0';
  return TRUE;
}

gboolean sb_append(StrBuf *sb, const char *text) {
  return sb_append_n(sb, text, strlen(text));
}

gboolean sb_append_char(StrBuf *sb, char ch) { return sb_append_n(sb, &ch, 1); }

gboolean sb_appendf(StrBuf *sb, const char *format, ...) {
  va_list args;
  char *text;
  gboolean ok;

  va_start(args, format);
  text = g_strdup_vprintf(format, args);
  va_end(args);

  ok = sb_append(sb, text);
  g_free(text);
  return ok;
}

char *sb_detach(StrBuf *sb) {
  char *text = sb->data;

  if (!text) {
    return g_strdup("");
  }

  sb->data = NULL;
  sb->len = 0;
  sb->cap = 0;
  return text;
}

char *str_dup(const char *text) { return g_strdup(text); }

gboolean str_ieq(const char *left, const char *right) {
  return left && right && g_ascii_strcasecmp(left, right) == 0;
}

gboolean str_iends_with(const char *text, const char *suffix) {
  size_t text_length;
  size_t suffix_length;

  if (!text || !suffix) {
    return FALSE;
  }

  text_length = strlen(text);
  suffix_length = strlen(suffix);
  if (suffix_length > text_length) {
    return FALSE;
  }

  return g_ascii_strcasecmp(text + text_length - suffix_length, suffix) == 0;
}

char *str_trim(char *text) {
  if (!text) {
    return NULL;
  }
  return g_strstrip(text);
}

gboolean str_copy(char *target, size_t cap, const char *source) {
  size_t length;

  if (!target || cap == 0) {
    return FALSE;
  }
  if (!source) {
    target[0] = '\0';
    return TRUE;
  }

  length = strlen(source);
  if (length >= cap) {
    target[0] = '\0';
    return FALSE;
  }

  g_strlcpy(target, source, cap);
  return TRUE;
}

char *path_join(const char *left, const char *right) {
  if (!right || !right[0]) {
    return g_strdup(left ? left : "");
  }
  if (!left || !left[0] || g_path_is_absolute(right)) {
    return g_strdup(right);
  }
  return g_build_filename(left, right, NULL);
}

char *path_dirname(const char *path) {
  if (!path || !path[0]) {
    return g_strdup(".");
  }
  return g_path_get_dirname(path);
}

const char *path_filename(const char *path) {
  const char *slash;

  if (!path) {
    return "";
  }

  slash = strrchr(path, '/');
  return slash ? slash + 1 : path;
}

char *path_stem(const char *path) {
  const char *name = path_filename(path);
  const char *dot = strrchr(name, '.');
  size_t length;

  if (dot && dot > name) {
    length = (size_t)(dot - name);
  } else {
    length = strlen(name);
  }

  return g_strndup(name, length);
}

const char *path_extension(const char *path) {
  const char *name = path_filename(path);
  const char *dot = strrchr(name, '.');

  return dot ? dot : "";
}

gboolean path_exists(const char *path) {
  return path && path[0] && g_file_test(path, G_FILE_TEST_EXISTS);
}

gboolean path_is_dir(const char *path) {
  return path && path[0] && g_file_test(path, G_FILE_TEST_IS_DIR);
}

/* Create every missing component of `path`, leaving an existing one alone. */
gboolean ensure_directory(const char *path) {
  if (!path || !path[0]) {
    return FALSE;
  }
  if (path_is_dir(path)) {
    return TRUE;
  }
  return g_mkdir_with_parents(path, 0700) == 0;
}

char *module_dir(void) {
  GError *error = NULL;
  char *link = g_file_read_link("/proc/self/exe", &error);
  char *directory;

  if (!link) {
    g_clear_error(&error);
    return g_get_current_dir();
  }

  directory = g_path_get_dirname(link);
  g_free(link);
  return directory;
}

/* The twin of `local_app_data_dir` on Windows: what the Rust side's
   `data_home` answers, which `ROLA_DATA_HOME` overrides for a run that must
   keep its files to itself. */
char *data_home(void) {
  const char *named = g_getenv("ROLA_DATA_HOME");

  if (named && named[0]) {
    return g_strdup(named);
  }
  return g_strdup(g_get_user_data_dir());
}

char *keys_dir(void) {
  char *home = data_home();
  char *result = g_build_filename(home, "rola", "keys", NULL);

  g_free(home);
  return result;
}

char *registry_file(void) {
  char *home = data_home();
  char *result =
      g_build_filename(home, "rola", "daemon", "registry.toml", NULL);

  g_free(home);
  return result;
}

char *find_program(const char *name) { return g_find_program_in_path(name); }

gboolean open_path(const char *path) {
  GError *error = NULL;
  char *absolute;
  char *uri;
  gboolean ok;

  if (!path || !path[0]) {
    return FALSE;
  }

  absolute = g_canonicalize_filename(path, NULL);
  uri = g_filename_to_uri(absolute, NULL, &error);
  g_free(absolute);
  if (!uri) {
    g_clear_error(&error);
    return FALSE;
  }

  ok = g_app_info_launch_default_for_uri(uri, NULL, &error);
  g_clear_error(&error);
  g_free(uri);
  return ok;
}

typedef struct AlertWait {
  GMainLoop *loop;
  int response;
} AlertWait;

static void alert_done(GObject *source, GAsyncResult *result,
                       gpointer user_data) {
  AlertWait *wait = (AlertWait *)user_data;
  GError *error = NULL;
  int response =
      gtk_alert_dialog_choose_finish(GTK_ALERT_DIALOG(source), result, &error);

  wait->response = error ? -1 : response;
  g_clear_error(&error);
  g_main_loop_quit(wait->loop);
}

static int alert_v(GtkWindow *owner, const char *format, va_list args,
                   const char *const *buttons, int default_button,
                   int cancel_button) {
  char *text = g_strdup_vprintf(format, args);
  GtkAlertDialog *dialog = gtk_alert_dialog_new("%s", text);
  AlertWait wait;

  gtk_alert_dialog_set_buttons(dialog, buttons);
  gtk_alert_dialog_set_default_button(dialog, default_button);
  gtk_alert_dialog_set_cancel_button(dialog, cancel_button);
  gtk_alert_dialog_set_modal(dialog, TRUE);

  wait.loop = g_main_loop_new(NULL, FALSE);
  wait.response = -1;
  gtk_alert_dialog_choose(dialog, owner, NULL, alert_done, &wait);
  g_main_loop_run(wait.loop);
  g_main_loop_unref(wait.loop);

  g_object_unref(dialog);
  g_free(text);
  return wait.response;
}

static const char *const OK_BUTTONS[] = {"OK", NULL};
static const char *const YES_NO_BUTTONS[] = {"No", "Yes", NULL};

void show_error(GtkWindow *owner, const char *format, ...) {
  va_list args;

  va_start(args, format);
  (void)alert_v(owner, format, args, OK_BUTTONS, 0, 0);
  va_end(args);
}

void show_info(GtkWindow *owner, const char *format, ...) {
  va_list args;

  va_start(args, format);
  (void)alert_v(owner, format, args, OK_BUTTONS, 0, 0);
  va_end(args);
}

gboolean ask_yes_no(GtkWindow *owner, const char *format, ...) {
  va_list args;
  int response;

  va_start(args, format);
  response = alert_v(owner, format, args, YES_NO_BUTTONS, 1, 0);
  va_end(args);
  return response == 1;
}

typedef struct FileWait {
  GMainLoop *loop;
  GFile *file;
  GListModel *files;
} FileWait;

static void folder_done(GObject *source, GAsyncResult *result,
                        gpointer user_data) {
  FileWait *wait = (FileWait *)user_data;
  GError *error = NULL;

  wait->file = gtk_file_dialog_select_folder_finish(GTK_FILE_DIALOG(source),
                                                    result, &error);
  g_clear_error(&error);
  g_main_loop_quit(wait->loop);
}

static void files_done(GObject *source, GAsyncResult *result,
                       gpointer user_data) {
  FileWait *wait = (FileWait *)user_data;
  GError *error = NULL;

  wait->files = gtk_file_dialog_open_multiple_finish(GTK_FILE_DIALOG(source),
                                                     result, &error);
  g_clear_error(&error);
  g_main_loop_quit(wait->loop);
}

gboolean choose_folder(GtkWindow *owner, const char *title, char *out,
                       size_t cap) {
  GtkFileDialog *dialog;
  FileWait wait;
  char *path = NULL;
  gboolean ok = FALSE;

  if (!out || cap == 0) {
    return FALSE;
  }
  out[0] = '\0';

  dialog = gtk_file_dialog_new();
  gtk_file_dialog_set_title(dialog, title);
  gtk_file_dialog_set_modal(dialog, TRUE);

  wait.loop = g_main_loop_new(NULL, FALSE);
  wait.file = NULL;
  wait.files = NULL;
  gtk_file_dialog_select_folder(dialog, owner, NULL, folder_done, &wait);
  g_main_loop_run(wait.loop);

  if (wait.file) {
    path = g_file_get_path(wait.file);
  }
  if (path && str_copy(out, cap, path)) {
    ok = TRUE;
  } else if (path) {
    out[0] = '\0';
  }

  g_free(path);
  g_clear_object(&wait.file);
  g_main_loop_unref(wait.loop);
  g_object_unref(dialog);
  return ok;
}

GPtrArray *choose_files(GtkWindow *owner, const char *title,
                        GtkFileFilter *filter) {
  GtkFileDialog *dialog;
  GtkFileFilter *all = gtk_file_filter_new();
  GListStore *filters;
  FileWait wait;
  GPtrArray *paths;

  gtk_file_filter_set_name(all, "All files");
  gtk_file_filter_add_pattern(all, "*");

  filters = g_list_store_new(GTK_TYPE_FILE_FILTER);
  if (filter) {
    g_list_store_append(filters, filter);
  }
  g_list_store_append(filters, all);

  dialog = gtk_file_dialog_new();
  gtk_file_dialog_set_title(dialog, title);
  gtk_file_dialog_set_modal(dialog, TRUE);
  gtk_file_dialog_set_filters(dialog, G_LIST_MODEL(filters));
  gtk_file_dialog_set_default_filter(dialog, filter ? filter : all);

  wait.loop = g_main_loop_new(NULL, FALSE);
  wait.file = NULL;
  wait.files = NULL;
  gtk_file_dialog_open_multiple(dialog, owner, NULL, files_done, &wait);
  g_main_loop_run(wait.loop);

  paths = g_ptr_array_new_with_free_func(g_free);
  if (wait.files) {
    guint count = g_list_model_get_n_items(wait.files);
    guint index;

    for (index = 0; index < count; ++index) {
      GFile *file = g_list_model_get_item(wait.files, index);
      char *path = g_file_get_path(file);

      if (path) {
        g_ptr_array_add(paths, path);
      }
      g_object_unref(file);
    }
  }

  g_clear_object(&wait.files);
  g_main_loop_unref(wait.loop);
  g_object_unref(dialog);
  g_object_unref(filters);
  g_object_unref(all);

  if (paths->len == 0) {
    g_ptr_array_unref(paths);
    return NULL;
  }
  return paths;
}
