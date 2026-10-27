#include "ui.h"

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#include "daemon.h"

AppState g_app;

void app_set_status(const char *text) {
  if (!g_app.status) {
    return;
  }
  gtk_label_set_text(GTK_LABEL(g_app.status), text ? text : "");
}

void app_set_statusf(const char *format, ...) {
  va_list args;
  char *text;

  if (!g_app.status) {
    return;
  }

  va_start(args, format);
  text = g_strdup_vprintf(format, args);
  va_end(args);

  app_set_status(text);
  g_free(text);
}

static void fix_daemon_path(void) {
  char *module = module_dir();
  char *exe = path_join(module, "rola-daemon");

  str_copy(g_app.daemon_exe, sizeof(g_app.daemon_exe), exe ? exe : "");
  g_free(exe);
  g_free(module);
}

gboolean app_daemon_file_exists(void) {
  return g_app.daemon_exe[0] &&
         g_file_test(g_app.daemon_exe, G_FILE_TEST_IS_REGULAR);
}

static gboolean try_rola(const char *directory, const char *name) {
  char *candidate = path_join(directory, name);
  gboolean found = candidate && g_file_test(candidate, G_FILE_TEST_IS_REGULAR);

  if (found) {
    str_copy(g_app.rola_exe, sizeof(g_app.rola_exe), candidate);
  }
  g_free(candidate);
  return found;
}

void app_refresh_rola_path(void) {
  char *daemon_dir = path_dirname(g_app.daemon_exe);
  char *module = module_dir();
  char *found;

  g_app.rola_exe[0] = '\0';

  if (try_rola(daemon_dir, "rola") || try_rola(module, "rola") ||
      try_rola(module, "bin/rola")) {
    g_free(daemon_dir);
    g_free(module);
    return;
  }

  found = find_program("rola");
  if (found) {
    str_copy(g_app.rola_exe, sizeof(g_app.rola_exe), found);
  }
  g_free(found);
  g_free(daemon_dir);
  g_free(module);
}

void app_fail_missing_daemon(void) {
  GtkWidget *window = g_app.main;

  if (!window) {
    return;
  }

  /* Take the window out of the message path first: the timer may fire again
     while the error box is up, and the window is destroyed below. */
  g_app.main = NULL;
  g_app.timer_id = 0;
  daemon_stop();
  show_error(NULL,
             "rola-daemon is missing:\n%s\n\nIt must stay next to "
             "rola-daemon-gui. The program will close.",
             g_app.daemon_exe);
  gtk_window_destroy(GTK_WINDOW(window));
  g_application_quit(G_APPLICATION(g_app.application));
}

static gboolean tick(gpointer user_data) {
  (void)user_data;

  if (!app_daemon_file_exists()) {
    app_fail_missing_daemon();
    return G_SOURCE_REMOVE;
  }

  daemon_page_sync_status();
  return G_SOURCE_CONTINUE;
}

static void show_page(guint page) {
  switch (page) {
  case 0:
    daemon_page_sync_status();
    break;
  case 1:
    registry_page_reload();
    break;
  case 2:
    keys_page_reload();
    break;
  case 3:
    admins_page_vaults_changed();
    break;
  default:
    break;
  }
}

static void on_switch_page(GtkNotebook *notebook, GtkWidget *page,
                           guint page_num, gpointer user_data) {
  (void)notebook;
  (void)page;
  (void)user_data;
  show_page(page_num);
}

static gboolean on_close_request(GtkWindow *window, gpointer user_data) {
  (void)window;
  (void)user_data;

  if (g_app.timer_id) {
    g_source_remove(g_app.timer_id);
    g_app.timer_id = 0;
  }
  daemon_stop();
  return FALSE;
}

static GtkWidget *build_window(GtkApplication *application) {
  GtkWidget *window = gtk_application_window_new(application);
  GtkWidget *root = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
  GtkWidget *notebook = gtk_notebook_new();
  GtkWidget *status = gtk_label_new("Ready.");
  GtkWidget *separator;

  gtk_window_set_title(GTK_WINDOW(window), "Rorolala Daemon");
  gtk_window_set_default_size(GTK_WINDOW(window), 780, 560);

  gtk_widget_set_vexpand(notebook, TRUE);
  gtk_notebook_append_page(GTK_NOTEBOOK(notebook), daemon_page_new(),
                           gtk_label_new("Daemon"));
  gtk_notebook_append_page(GTK_NOTEBOOK(notebook), registry_page_new(),
                           gtk_label_new("Registry"));
  gtk_notebook_append_page(GTK_NOTEBOOK(notebook), keys_page_new(),
                           gtk_label_new("Keys"));
  gtk_notebook_append_page(GTK_NOTEBOOK(notebook), admins_page_new(),
                           gtk_label_new("Admins"));

  gtk_label_set_xalign(GTK_LABEL(status), 0.0f);
  gtk_label_set_ellipsize(GTK_LABEL(status), PANGO_ELLIPSIZE_END);
  gtk_widget_set_margin_start(status, 8);
  gtk_widget_set_margin_end(status, 8);
  gtk_widget_set_margin_top(status, 4);
  gtk_widget_set_margin_bottom(status, 4);

  separator = gtk_separator_new(GTK_ORIENTATION_HORIZONTAL);

  gtk_box_append(GTK_BOX(root), notebook);
  gtk_box_append(GTK_BOX(root), separator);
  gtk_box_append(GTK_BOX(root), status);
  gtk_window_set_child(GTK_WINDOW(window), root);

  g_app.main = window;
  g_app.notebook = notebook;
  g_app.status = status;

  g_signal_connect(notebook, "switch-page", G_CALLBACK(on_switch_page), NULL);
  g_signal_connect(window, "close-request", G_CALLBACK(on_close_request), NULL);

  return window;
}

static void activate(GtkApplication *application, gpointer user_data) {
  GtkWidget *window;

  (void)user_data;
  g_app.application = application;

  if (g_app.main) {
    gtk_window_present(GTK_WINDOW(g_app.main));
    return;
  }

  if (!app_daemon_file_exists()) {
    show_error(NULL, "rola-daemon was not found next to rola-daemon-gui.\n\n"
                     "Put rola-daemon in the same folder and start again.");
    g_application_quit(G_APPLICATION(application));
    return;
  }
  app_refresh_rola_path();

  window = build_window(application);
  gtk_window_present(GTK_WINDOW(window));

  show_page(0);
  g_app.timer_id = g_timeout_add(750, tick, NULL);
  app_set_status("Ready.");
}

int main(int argc, char **argv) {
  GtkApplication *application;
  int status;

  fix_daemon_path();

  application = gtk_application_new("org.rorolala.DaemonGui",
                                    G_APPLICATION_DEFAULT_FLAGS);
  g_signal_connect(application, "activate", G_CALLBACK(activate), NULL);

  status = g_application_run(G_APPLICATION(application), argc, argv);

  g_object_unref(application);
  return status;
}
