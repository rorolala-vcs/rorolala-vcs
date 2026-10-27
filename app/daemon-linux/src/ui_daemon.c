#include "ui.h"

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "daemon.h"
#include "process.h"

static GtkWidget *g_page;
static GtkWidget *g_port;
static GtkWidget *g_start;
static GtkWidget *g_stop;
static GtkWidget *g_status;
static GtkWidget *g_log_view;
static GtkTextBuffer *g_log_buffer;
static gboolean g_last_running;

static void append_log(const char *text) {
  GtkTextIter end;

  if (!g_log_buffer || !text || !text[0]) {
    return;
  }

  gtk_text_buffer_get_end_iter(g_log_buffer, &end);
  gtk_text_buffer_insert(g_log_buffer, &end, text, -1);
  gtk_text_buffer_get_end_iter(g_log_buffer, &end);
  gtk_text_buffer_place_cursor(g_log_buffer, &end);
  gtk_text_view_scroll_mark_onscreen(GTK_TEXT_VIEW(g_log_view),
                                     gtk_text_buffer_get_insert(g_log_buffer));
}

void daemon_page_append_output(const char *text) { append_log(text); }

void daemon_page_sync_status(void) {
  gboolean running;
  int code;

  if (!g_page || !g_status) {
    return;
  }

  running = daemon_is_running();
  if (running) {
    char text[128];

    g_snprintf(text, sizeof(text), "Running (pid %d)", daemon_pid());
    gtk_label_set_text(GTK_LABEL(g_status), text);
    if (!g_last_running) {
      app_set_statusf("rola-daemon is running (pid %d).", daemon_pid());
    }
  } else {
    gtk_label_set_text(GTK_LABEL(g_status), "Stopped");
    if (g_last_running) {
      char line[128];

      code = daemon_last_exit_code();
      app_set_statusf("rola-daemon stopped (exit code %d).", code);
      g_snprintf(line, sizeof(line), "\nrola-daemon exited with code %d.\n",
                 code);
      append_log(line);
    }
  }

  g_last_running = running;
  gtk_widget_set_sensitive(g_start, !running);
  gtk_widget_set_sensitive(g_stop, running);
}

static gboolean parse_port(const char *text, char *normalized, size_t cap) {
  char buffer[64];
  char *trimmed;
  char *end = NULL;
  unsigned long value;

  normalized[0] = '\0';
  if (!str_copy(buffer, sizeof(buffer), text)) {
    return FALSE;
  }

  trimmed = str_trim(buffer);
  if (!trimmed[0]) {
    return TRUE;
  }

  errno = 0;
  value = strtoul(trimmed, &end, 10);
  if (end == trimmed || *end != '\0' || errno == ERANGE || value == 0 ||
      value > 65535) {
    return FALSE;
  }

  g_snprintf(normalized, cap, "%lu", value);
  return TRUE;
}

static void start_daemon(void) {
  const char *port_text;
  char port[32];
  char error[4096];
  CmdLine args;

  if (!app_daemon_file_exists()) {
    app_fail_missing_daemon();
    return;
  }

  app_refresh_rola_path();

  port_text = gtk_editable_get_text(GTK_EDITABLE(g_port));
  if (!parse_port(port_text, port, sizeof(port))) {
    show_error(GTK_WINDOW(g_app.main),
               "The port must be a number between 1 and 65535, or blank.");
    gtk_widget_grab_focus(g_port);
    return;
  }

  cmd_init(&args);
  if (port[0]) {
    (void)cmd_add(&args, "--port");
    (void)cmd_add(&args, port);
  }

  append_log("Starting rola-daemon...\n");
  if (!daemon_start(g_app.daemon_exe, &args, error, sizeof(error))) {
    append_log(error);
    append_log("\n");
    show_error(GTK_WINDOW(g_app.main), "%s", error);
    cmd_free(&args);
    daemon_page_sync_status();
    return;
  }

  {
    char line[256];

    g_snprintf(line, sizeof(line), "Started with pid %d.\n", daemon_pid());
    append_log(line);
  }

  cmd_free(&args);
  daemon_page_sync_status();
}

static void stop_daemon(void) {
  if (!daemon_is_running()) {
    app_set_status("rola-daemon is not running.");
    daemon_page_sync_status();
    return;
  }

  append_log("Stopping rola-daemon...\n");
  daemon_stop();
  append_log("Stopped.\n");
  app_set_status("rola-daemon stopped.");
  daemon_page_sync_status();
}

static void on_start_clicked(GtkButton *button, gpointer user_data) {
  (void)button;
  (void)user_data;
  start_daemon();
}

static void on_stop_clicked(GtkButton *button, gpointer user_data) {
  (void)button;
  (void)user_data;
  stop_daemon();
}

GtkWidget *daemon_page_new(void) {
  GtkWidget *page = gtk_box_new(GTK_ORIENTATION_VERTICAL, 8);
  GtkWidget *grid = gtk_grid_new();
  GtkWidget *port_label;
  GtkWidget *hint;
  GtkWidget *output_label;
  GtkWidget *scroller;

  gtk_widget_set_margin_start(page, 10);
  gtk_widget_set_margin_end(page, 10);
  gtk_widget_set_margin_top(page, 10);
  gtk_widget_set_margin_bottom(page, 10);

  gtk_grid_set_column_spacing(GTK_GRID(grid), 8);
  gtk_grid_set_row_spacing(GTK_GRID(grid), 6);

  port_label = gtk_label_new("Port");
  gtk_widget_set_halign(port_label, GTK_ALIGN_END);

  g_port = gtk_entry_new();
  gtk_editable_set_text(GTK_EDITABLE(g_port), "7717");
  gtk_editable_set_width_chars(GTK_EDITABLE(g_port), 7);
  gtk_entry_set_max_length(GTK_ENTRY(g_port), 5);

  hint = gtk_label_new("(1-65535; blank uses 7717 or ROLA_DEFAULT_PORT)");
  gtk_label_set_xalign(GTK_LABEL(hint), 0.0f);

  g_start = gtk_button_new_with_label("Start");
  g_stop = gtk_button_new_with_label("Stop");
  g_status = gtk_label_new("Stopped");
  gtk_label_set_xalign(GTK_LABEL(g_status), 0.0f);
  gtk_label_set_ellipsize(GTK_LABEL(g_status), PANGO_ELLIPSIZE_END);
  gtk_widget_set_hexpand(g_status, TRUE);

  gtk_grid_attach(GTK_GRID(grid), port_label, 0, 0, 1, 1);
  gtk_grid_attach(GTK_GRID(grid), g_port, 1, 0, 1, 1);
  gtk_grid_attach(GTK_GRID(grid), hint, 2, 0, 3, 1);
  gtk_grid_attach(GTK_GRID(grid), g_start, 1, 1, 1, 1);
  gtk_grid_attach(GTK_GRID(grid), g_stop, 2, 1, 1, 1);
  gtk_grid_attach(GTK_GRID(grid), g_status, 3, 1, 2, 1);

  output_label = gtk_label_new("Output");
  gtk_label_set_xalign(GTK_LABEL(output_label), 0.0f);

  scroller = gtk_scrolled_window_new();
  gtk_widget_set_vexpand(scroller, TRUE);
  gtk_scrolled_window_set_policy(GTK_SCROLLED_WINDOW(scroller),
                                 GTK_POLICY_AUTOMATIC, GTK_POLICY_AUTOMATIC);
  gtk_scrolled_window_set_has_frame(GTK_SCROLLED_WINDOW(scroller), TRUE);

  g_log_view = gtk_text_view_new();
  gtk_text_view_set_editable(GTK_TEXT_VIEW(g_log_view), FALSE);
  gtk_text_view_set_cursor_visible(GTK_TEXT_VIEW(g_log_view), FALSE);
  gtk_text_view_set_monospace(GTK_TEXT_VIEW(g_log_view), TRUE);
  gtk_text_view_set_wrap_mode(GTK_TEXT_VIEW(g_log_view), GTK_WRAP_WORD_CHAR);
  g_log_buffer = gtk_text_view_get_buffer(GTK_TEXT_VIEW(g_log_view));
  gtk_scrolled_window_set_child(GTK_SCROLLED_WINDOW(scroller), g_log_view);

  gtk_box_append(GTK_BOX(page), grid);
  gtk_box_append(GTK_BOX(page), output_label);
  gtk_box_append(GTK_BOX(page), scroller);

  g_signal_connect(g_start, "clicked", G_CALLBACK(on_start_clicked), NULL);
  g_signal_connect(g_stop, "clicked", G_CALLBACK(on_stop_clicked), NULL);

  g_page = page;
  g_last_running = FALSE;
  daemon_set_output_func(daemon_page_append_output);
  daemon_page_sync_status();
  return page;
}
