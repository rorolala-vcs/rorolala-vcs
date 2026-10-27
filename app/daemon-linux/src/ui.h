#pragma once

#include <gtk/gtk.h>

#include "util.h"

typedef struct AppState {
  GtkApplication *application;
  GtkWidget *main;
  GtkWidget *notebook;
  GtkWidget *status;
  char daemon_exe[PATH_CCH];
  char rola_exe[PATH_CCH];
  guint timer_id;
} AppState;

extern AppState g_app;

void app_set_status(const char *text);
void app_set_statusf(const char *format, ...) G_GNUC_PRINTF(1, 2);
void app_refresh_rola_path(void);
gboolean app_daemon_file_exists(void);
void app_fail_missing_daemon(void);

GtkWidget *daemon_page_new(void);
GtkWidget *registry_page_new(void);
GtkWidget *keys_page_new(void);
GtkWidget *admins_page_new(void);

void daemon_page_sync_status(void);
void daemon_page_append_output(const char *text);
void registry_page_reload(void);
void keys_page_reload(void);
void admins_page_vaults_changed(void);
