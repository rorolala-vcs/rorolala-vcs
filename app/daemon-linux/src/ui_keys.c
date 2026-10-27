#include "ui.h"

#include <string.h>

#include "keys.h"

static GtkWidget *g_list;
static GtkWidget *g_dir_label;
static KeyList g_keys;

static void clear_list(void) {
  GtkWidget *child;

  while ((child = gtk_widget_get_first_child(g_list)) != NULL) {
    gtk_list_box_remove(GTK_LIST_BOX(g_list), child);
  }
}

static void add_row(const char *text) {
  GtkWidget *row = gtk_list_box_row_new();
  GtkWidget *label = gtk_label_new(text);

  gtk_label_set_xalign(GTK_LABEL(label), 0.0f);
  gtk_label_set_ellipsize(GTK_LABEL(label), PANGO_ELLIPSIZE_END);
  gtk_widget_set_hexpand(label, TRUE);
  gtk_widget_set_margin_start(label, 4);
  gtk_widget_set_margin_end(label, 4);
  gtk_widget_set_margin_top(label, 3);
  gtk_widget_set_margin_bottom(label, 3);

  gtk_list_box_row_set_child(GTK_LIST_BOX_ROW(row), label);
  gtk_list_box_append(GTK_LIST_BOX(g_list), row);
}

static long selected_index(void) {
  GtkListBoxRow *row = gtk_list_box_get_selected_row(GTK_LIST_BOX(g_list));
  int index;

  if (!row) {
    return -1;
  }

  index = gtk_list_box_row_get_index(row);
  if (index < 0 || (size_t)index >= g_keys.count) {
    return -1;
  }
  return index;
}

void keys_page_reload(void) {
  char error[4096];
  char *directory;
  size_t i;

  directory = keys_dir();
  gtk_label_set_text(GTK_LABEL(g_dir_label), directory ? directory : "");
  g_free(directory);

  if (!g_app.rola_exe[0]) {
    clear_list();
    key_list_free(&g_keys);
    app_set_status(
        "rola was not found next to rola-daemon or on the program path.");
    return;
  }

  if (!keys_load(g_app.rola_exe, &g_keys, error, sizeof(error))) {
    clear_list();
    app_set_statusf("Could not list keys: %s", error);
    return;
  }

  clear_list();
  for (i = 0; i < g_keys.count; ++i) {
    char line[PATH_CCH + 64];

    g_snprintf(line, sizeof(line), "[%s] %s",
               g_keys.items[i].private_key ? "private" : "public",
               path_filename(g_keys.items[i].path));
    add_row(line);
  }

  if (g_keys.count == 0) {
    app_set_status("No keys found. Use Import Keys to add a .pem or .pub.");
  } else {
    app_set_statusf("%zu key(s) in the user key directory.", g_keys.count);
  }
}

static void on_row_selected(GtkListBox *box, GtkListBoxRow *row,
                            gpointer user_data) {
  long index;

  (void)box;
  (void)user_data;

  if (!row) {
    return;
  }

  index = gtk_list_box_row_get_index(row);
  if (index >= 0 && (size_t)index < g_keys.count) {
    app_set_status(g_keys.items[index].path);
  }
}

static void reveal_selected(void) {
  long index = selected_index();
  char *directory;

  if (index < 0) {
    return;
  }

  directory = path_dirname(g_keys.items[index].path);
  if (!open_path(directory)) {
    show_error(GTK_WINDOW(g_app.main),
               "The key directory could not be opened:\n%s", directory);
  }
  g_free(directory);
}

static void on_row_activated(GtkListBox *box, GtkListBoxRow *row,
                             gpointer user_data) {
  (void)box;
  (void)row;
  (void)user_data;
  reveal_selected();
}

static void pick_and_import(void) {
  GtkFileFilter *filter = gtk_file_filter_new();
  GPtrArray *paths;
  char message[4096];
  gboolean ok;

  gtk_file_filter_set_name(filter, "Key files (*.pem, *.pub)");
  gtk_file_filter_add_pattern(filter, "*.pem");
  gtk_file_filter_add_pattern(filter, "*.pub");

  paths = choose_files(GTK_WINDOW(g_app.main), "Import keys", filter);
  g_object_unref(filter);
  if (!paths) {
    return;
  }

  ok = keys_import_files(GTK_WINDOW(g_app.main), (const char **)paths->pdata,
                         paths->len, message, sizeof(message));
  if (ok) {
    show_info(GTK_WINDOW(g_app.main), "%s", message);
    app_set_statusf("Imported %u key file(s).", paths->len);
  } else {
    show_error(GTK_WINDOW(g_app.main), "%s", message);
    app_set_status("Some key files were not imported.");
  }

  g_ptr_array_unref(paths);
  keys_page_reload();
}

static void open_key_folder(void) {
  char *directory = keys_dir();

  if (!ensure_directory(directory)) {
    show_error(GTK_WINDOW(g_app.main),
               "The key directory could not be created:\n%s", directory);
    g_free(directory);
    return;
  }

  if (!open_path(directory)) {
    show_error(GTK_WINDOW(g_app.main),
               "The key directory could not be opened:\n%s", directory);
  }
  g_free(directory);
}

static void on_import_clicked(GtkButton *button, gpointer user_data) {
  (void)button;
  (void)user_data;
  pick_and_import();
}

static void on_refresh_clicked(GtkButton *button, gpointer user_data) {
  (void)button;
  (void)user_data;
  keys_page_reload();
}

static void on_open_clicked(GtkButton *button, gpointer user_data) {
  (void)button;
  (void)user_data;
  open_key_folder();
}

GtkWidget *keys_page_new(void) {
  GtkWidget *page = gtk_box_new(GTK_ORIENTATION_VERTICAL, 8);
  GtkWidget *scroller = gtk_scrolled_window_new();
  GtkWidget *buttons = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
  GtkWidget *import;
  GtkWidget *refresh;
  GtkWidget *open;

  gtk_widget_set_margin_start(page, 10);
  gtk_widget_set_margin_end(page, 10);
  gtk_widget_set_margin_top(page, 10);
  gtk_widget_set_margin_bottom(page, 10);

  g_list = gtk_list_box_new();
  gtk_list_box_set_selection_mode(GTK_LIST_BOX(g_list), GTK_SELECTION_SINGLE);
  gtk_list_box_set_activate_on_single_click(GTK_LIST_BOX(g_list), FALSE);
  gtk_scrolled_window_set_policy(GTK_SCROLLED_WINDOW(scroller),
                                 GTK_POLICY_AUTOMATIC, GTK_POLICY_AUTOMATIC);
  gtk_scrolled_window_set_has_frame(GTK_SCROLLED_WINDOW(scroller), TRUE);
  gtk_scrolled_window_set_child(GTK_SCROLLED_WINDOW(scroller), g_list);
  gtk_widget_set_vexpand(scroller, TRUE);
  gtk_widget_set_hexpand(scroller, TRUE);

  import = gtk_button_new_with_label("Import Keys...");
  refresh = gtk_button_new_with_label("Refresh");
  open = gtk_button_new_with_label("Open folder");
  gtk_box_append(GTK_BOX(buttons), import);
  gtk_box_append(GTK_BOX(buttons), refresh);
  gtk_box_append(GTK_BOX(buttons), open);

  g_dir_label = gtk_label_new("");
  gtk_label_set_xalign(GTK_LABEL(g_dir_label), 0.0f);
  gtk_label_set_ellipsize(GTK_LABEL(g_dir_label), PANGO_ELLIPSIZE_END);

  gtk_box_append(GTK_BOX(page), scroller);
  gtk_box_append(GTK_BOX(page), buttons);
  gtk_box_append(GTK_BOX(page), g_dir_label);

  g_signal_connect(g_list, "row-selected", G_CALLBACK(on_row_selected), NULL);
  g_signal_connect(g_list, "row-activated", G_CALLBACK(on_row_activated), NULL);
  g_signal_connect(import, "clicked", G_CALLBACK(on_import_clicked), NULL);
  g_signal_connect(refresh, "clicked", G_CALLBACK(on_refresh_clicked), NULL);
  g_signal_connect(open, "clicked", G_CALLBACK(on_open_clicked), NULL);

  key_list_init(&g_keys);
  return page;
}
