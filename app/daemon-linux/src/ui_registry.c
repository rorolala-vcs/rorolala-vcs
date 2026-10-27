#include "ui.h"

#include <stdio.h>
#include <string.h>

#include "registry.h"

static GtkWidget *g_list;
static GtkWidget *g_path;
static GtkWidget *g_edit;
static GtkWidget *g_delete;
static GtkWidget *g_browse;
static RegistryList g_registry;

typedef struct EntryDialogState {
  GtkWidget *dialog;
  GtkWidget *namespace_entry;
  GtkWidget *path_entry;
  GMainLoop *loop;
  int response;
  char namespace_name[256];
  char path[PATH_CCH];
} EntryDialogState;

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
  if (index < 0 || (size_t)index >= g_registry.count) {
    return -1;
  }
  return index;
}

static void select_index(long index) {
  GtkListBoxRow *row;

  if (index < 0) {
    return;
  }

  row = gtk_list_box_get_row_at_index(GTK_LIST_BOX(g_list), (int)index);
  if (row) {
    gtk_list_box_select_row(GTK_LIST_BOX(g_list), row);
  }
}

static void update_path_edit(void) {
  long index = selected_index();

  if (index >= 0) {
    gtk_editable_set_text(GTK_EDITABLE(g_path), g_registry.items[index].path);
  } else {
    gtk_editable_set_text(GTK_EDITABLE(g_path), "");
  }

  gtk_widget_set_sensitive(g_edit, index >= 0);
  gtk_widget_set_sensitive(g_delete, index >= 0);
  gtk_widget_set_sensitive(g_browse, index >= 0);
}

static void fill_list(void) {
  size_t i;

  clear_list();
  for (i = 0; i < g_registry.count; ++i) {
    char line[PATH_CCH + 320];

    if (g_registry.items[i].checked) {
      g_snprintf(line, sizeof(line), "%s %s  -  %s",
                 g_registry.items[i].reachable ? "[OK]" : "[FAIL]",
                 g_registry.items[i].namespace_name, g_registry.items[i].path);
    } else {
      g_snprintf(line, sizeof(line), "%s  -  %s",
                 g_registry.items[i].namespace_name, g_registry.items[i].path);
    }
    add_row(line);
  }
}

static void reload(gboolean report) {
  char error[4096];
  char previous[256];
  long selected;
  size_t i;
  long next = 0;

  previous[0] = '\0';
  selected = selected_index();
  if (selected >= 0) {
    str_copy(previous, sizeof(previous),
             g_registry.items[selected].namespace_name);
  }

  if (!registry_list_load(g_app.daemon_exe, &g_registry, error,
                          sizeof(error))) {
    clear_list();
    gtk_editable_set_text(GTK_EDITABLE(g_path), "");
    update_path_edit();
    app_set_statusf("Could not read the registry: %s", error);
    if (report) {
      show_error(GTK_WINDOW(g_app.main), "Could not read the registry:\n%s",
                 error);
    }
    return;
  }

  fill_list();

  for (i = 0; i < g_registry.count; ++i) {
    if (previous[0] &&
        strcmp(previous, g_registry.items[i].namespace_name) == 0) {
      next = (long)i;
      break;
    }
  }
  if (g_registry.count > 0) {
    select_index(next);
  }
  update_path_edit();

  if (g_registry.count == 0) {
    app_set_status("Registry is empty. Use New to serve a Vault.");
  } else {
    app_set_statusf("%zu namespace(s) registered.", g_registry.count);
  }
}

void registry_page_reload(void) { reload(FALSE); }

static void close_entry(EntryDialogState *state, int response) {
  state->response = response;
  gtk_window_destroy(GTK_WINDOW(state->dialog));
  g_main_loop_quit(state->loop);
}

static void on_entry_destroy(GtkWidget *widget, gpointer user_data) {
  EntryDialogState *state = (EntryDialogState *)user_data;

  (void)widget;
  /* The main window may take the dialog down with it, which destroys it without
     a close request; the loop has to be released either way. */
  if (state->loop) {
    g_main_loop_quit(state->loop);
  }
}

static gboolean on_entry_close(GtkWindow *window, gpointer user_data) {
  EntryDialogState *state = (EntryDialogState *)user_data;

  (void)window;
  state->response = GTK_RESPONSE_CANCEL;
  g_main_loop_quit(state->loop);
  return FALSE;
}

static void on_entry_ok(GtkButton *button, gpointer user_data) {
  EntryDialogState *state = (EntryDialogState *)user_data;

  (void)button;

  {
    char namespace_name[256];
    char path[PATH_CCH];
    char *trimmed_namespace;
    char *trimmed_path;
    char *config;
    gboolean is_vault;

    str_copy(namespace_name, sizeof(namespace_name),
             gtk_editable_get_text(GTK_EDITABLE(state->namespace_entry)));
    str_copy(path, sizeof(path),
             gtk_editable_get_text(GTK_EDITABLE(state->path_entry)));

    trimmed_namespace = str_trim(namespace_name);
    if (!registry_is_namespace(trimmed_namespace)) {
      show_error(GTK_WINDOW(state->dialog),
                 "A namespace is letters, digits, '-' and '_', and starts with "
                 "a letter or a digit.");
      gtk_widget_grab_focus(state->namespace_entry);
      return;
    }

    trimmed_path = str_trim(path);
    if (!trimmed_path[0]) {
      show_error(GTK_WINDOW(state->dialog), "Please choose the Vault folder.");
      gtk_widget_grab_focus(state->path_entry);
      return;
    }

    config = path_join(trimmed_path, "vault.toml");
    is_vault = config && path_exists(config);
    g_free(config);
    if (!is_vault) {
      show_error(GTK_WINDOW(state->dialog),
                 "That folder does not hold a Vault: vault.toml was not "
                 "found.");
      gtk_widget_grab_focus(state->path_entry);
      return;
    }

    /* The two are kept apart: what the row is named and where it points are
       read from their own box, and a name read from the path's box would be a
       row the daemon refuses as invalid. */
    str_copy(state->namespace_name, sizeof(state->namespace_name),
             trimmed_namespace);
    str_copy(state->path, sizeof(state->path), trimmed_path);
  }

  close_entry(state, GTK_RESPONSE_OK);
}

static void on_entry_cancel(GtkButton *button, gpointer user_data) {
  (void)button;
  close_entry((EntryDialogState *)user_data, GTK_RESPONSE_CANCEL);
}

static void on_entry_browse(GtkButton *button, gpointer user_data) {
  EntryDialogState *state = (EntryDialogState *)user_data;
  char folder[PATH_CCH];

  (void)button;

  if (!choose_folder(GTK_WINDOW(state->dialog), "Select the Vault folder",
                     folder, sizeof(folder))) {
    return;
  }

  gtk_editable_set_text(GTK_EDITABLE(state->path_entry), folder);
  if (!gtk_editable_get_text(GTK_EDITABLE(state->namespace_entry))[0]) {
    const char *name = path_filename(folder);

    if (name[0]) {
      gtk_editable_set_text(GTK_EDITABLE(state->namespace_entry), name);
    }
  }
}

static gboolean ask_entry(GtkWindow *owner, const char *title,
                          const char *namespace_name, const char *path,
                          EntryDialogState *state) {
  GtkWidget *dialog;
  GtkWidget *root;
  GtkWidget *grid;
  GtkWidget *namespace_label;
  GtkWidget *path_label;
  GtkWidget *browse;
  GtkWidget *buttons;
  GtkWidget *cancel;
  GtkWidget *ok;

  dialog = gtk_window_new();
  gtk_window_set_title(GTK_WINDOW(dialog), title);
  gtk_window_set_transient_for(GTK_WINDOW(dialog), owner);
  gtk_window_set_modal(GTK_WINDOW(dialog), TRUE);
  gtk_window_set_destroy_with_parent(GTK_WINDOW(dialog), TRUE);
  gtk_window_set_default_size(GTK_WINDOW(dialog), 560, 150);

  root = gtk_box_new(GTK_ORIENTATION_VERTICAL, 10);
  gtk_widget_set_margin_start(root, 10);
  gtk_widget_set_margin_end(root, 10);
  gtk_widget_set_margin_top(root, 10);
  gtk_widget_set_margin_bottom(root, 10);
  grid = gtk_grid_new();
  gtk_grid_set_column_spacing(GTK_GRID(grid), 8);
  gtk_grid_set_row_spacing(GTK_GRID(grid), 8);
  gtk_widget_set_margin_start(grid, 10);
  gtk_widget_set_margin_end(grid, 10);
  gtk_widget_set_margin_top(grid, 10);
  gtk_widget_set_margin_bottom(grid, 10);

  namespace_label = gtk_label_new("Namespace");
  gtk_widget_set_halign(namespace_label, GTK_ALIGN_END);
  state->namespace_entry = gtk_entry_new();
  gtk_editable_set_text(GTK_EDITABLE(state->namespace_entry), namespace_name);
  gtk_entry_set_activates_default(GTK_ENTRY(state->namespace_entry), TRUE);

  path_label = gtk_label_new("Vault path");
  gtk_widget_set_halign(path_label, GTK_ALIGN_END);
  state->path_entry = gtk_entry_new();
  gtk_editable_set_text(GTK_EDITABLE(state->path_entry), path);
  gtk_widget_set_hexpand(state->path_entry, TRUE);
  gtk_entry_set_activates_default(GTK_ENTRY(state->path_entry), TRUE);

  browse = gtk_button_new_with_label("Browse...");

  gtk_grid_attach(GTK_GRID(grid), namespace_label, 0, 0, 1, 1);
  gtk_grid_attach(GTK_GRID(grid), state->namespace_entry, 1, 0, 2, 1);
  gtk_grid_attach(GTK_GRID(grid), path_label, 0, 1, 1, 1);
  gtk_grid_attach(GTK_GRID(grid), state->path_entry, 1, 1, 1, 1);
  gtk_grid_attach(GTK_GRID(grid), browse, 2, 1, 1, 1);

  cancel = gtk_button_new_with_mnemonic("_Cancel");
  ok = gtk_button_new_with_mnemonic("_OK");
  gtk_widget_set_halign(ok, GTK_ALIGN_END);
  buttons = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
  gtk_widget_set_halign(buttons, GTK_ALIGN_END);
  gtk_box_append(GTK_BOX(buttons), cancel);
  gtk_box_append(GTK_BOX(buttons), ok);

  gtk_box_append(GTK_BOX(root), grid);
  gtk_box_append(GTK_BOX(root), buttons);
  gtk_window_set_child(GTK_WINDOW(dialog), root);
  gtk_window_set_default_widget(GTK_WINDOW(dialog), ok);

  state->dialog = dialog;
  state->response = GTK_RESPONSE_CANCEL;
  state->loop = g_main_loop_new(NULL, FALSE);

  g_signal_connect(dialog, "close-request", G_CALLBACK(on_entry_close), state);
  g_signal_connect(dialog, "destroy", G_CALLBACK(on_entry_destroy), state);
  g_signal_connect(ok, "clicked", G_CALLBACK(on_entry_ok), state);
  g_signal_connect(cancel, "clicked", G_CALLBACK(on_entry_cancel), state);
  g_signal_connect(browse, "clicked", G_CALLBACK(on_entry_browse), state);

  gtk_widget_grab_focus(state->namespace_entry);
  gtk_window_present(GTK_WINDOW(dialog));
  g_main_loop_run(state->loop);
  g_main_loop_unref(state->loop);
  state->loop = NULL;
  state->dialog = NULL;

  return state->response == GTK_RESPONSE_OK;
}

static void new_entry(void) {
  EntryDialogState state;
  char message[4096];

  memset(&state, 0, sizeof(state));

  if (!ask_entry(GTK_WINDOW(g_app.main), "New Registry Entry", "", "",
                 &state)) {
    return;
  }

  if (registry_register(g_app.daemon_exe, state.namespace_name, state.path,
                        message, sizeof(message))) {
    app_set_statusf("Registered %s.", state.namespace_name);
  } else {
    show_error(GTK_WINDOW(g_app.main), "Could not register the Vault:\n%s",
               message);
    app_set_status("Registration failed.");
  }
  reload(FALSE);
}

static void edit_entry(void) {
  long index = selected_index();
  EntryDialogState state;
  char message[4096];
  char old_namespace[256];

  if (index < 0) {
    return;
  }

  str_copy(old_namespace, sizeof(old_namespace),
           g_registry.items[index].namespace_name);

  memset(&state, 0, sizeof(state));
  if (!ask_entry(GTK_WINDOW(g_app.main), "Edit Registry Entry",
                 g_registry.items[index].namespace_name,
                 g_registry.items[index].path, &state)) {
    return;
  }

  if (str_ieq(old_namespace, state.namespace_name) &&
      str_ieq(g_registry.items[index].path, state.path)) {
    return;
  }

  if (registry_update(g_app.daemon_exe, old_namespace, state.namespace_name,
                      state.path, message, sizeof(message))) {
    app_set_statusf("Updated %s.", state.namespace_name);
  } else {
    show_error(GTK_WINDOW(g_app.main), "Could not update the entry:\n%s",
               message);
    app_set_status("Update failed.");
  }
  reload(FALSE);
}

static void delete_entry(void) {
  long index = selected_index();
  char message[4096];

  if (index < 0) {
    return;
  }

  if (!ask_yes_no(GTK_WINDOW(g_app.main),
                  "Remove the registry row \"%s\"?\n\nThe Vault stays where "
                  "it is.",
                  g_registry.items[index].namespace_name)) {
    return;
  }

  if (registry_remove(g_app.daemon_exe, g_registry.items[index].namespace_name,
                      message, sizeof(message))) {
    app_set_statusf("Removed %s.", g_registry.items[index].namespace_name);
  } else {
    show_error(GTK_WINDOW(g_app.main), "Could not remove the entry:\n%s",
               message);
    app_set_status("Removal failed.");
  }
  reload(FALSE);
}

static void browse_selected(void) {
  long index = selected_index();

  if (index < 0) {
    return;
  }

  if (!open_path(g_registry.items[index].path)) {
    show_error(GTK_WINDOW(g_app.main), "The folder could not be opened:\n%s",
               g_registry.items[index].path);
  }
}

static void edit_text(void) {
  char *file = registry_file();
  char *folder = path_dirname(file);
  GError *error = NULL;

  if (!ensure_directory(folder)) {
    show_error(GTK_WINDOW(g_app.main),
               "The registry folder could not be created:\n%s", folder);
    g_free(folder);
    g_free(file);
    return;
  }
  g_free(folder);

  if (!path_exists(file)) {
    (void)g_file_set_contents(file, "", 0, &error);
    g_clear_error(&error);
  }

  if (!open_path(file)) {
    show_error(GTK_WINDOW(g_app.main),
               "The registry file could not be opened:\n%s", file);
  } else {
    app_set_status(
        "Registry opened in a text editor. Use Refresh when it is saved.");
  }

  g_free(file);
}

static void check_registry(void) {
  char error[4096];
  size_t i;
  size_t reachable = 0;

  if (!registry_check(g_app.daemon_exe, &g_registry, error, sizeof(error))) {
    show_error(GTK_WINDOW(g_app.main), "Could not check the registry:\n%s",
               error);
    app_set_status("Check failed.");
    return;
  }

  fill_list();
  for (i = 0; i < g_registry.count; ++i) {
    if (g_registry.items[i].reachable) {
      ++reachable;
    }
  }
  if (g_registry.count > 0) {
    select_index(0);
  }
  update_path_edit();
  app_set_statusf("%zu of %zu namespace(s) reach a Vault.", reachable,
                  g_registry.count);
}

static void on_row_selected(GtkListBox *box, GtkListBoxRow *row,
                            gpointer user_data) {
  (void)box;
  (void)row;
  (void)user_data;
  update_path_edit();
}

static void on_row_activated(GtkListBox *box, GtkListBoxRow *row,
                             gpointer user_data) {
  (void)box;
  (void)row;
  (void)user_data;
  edit_entry();
}

static void on_new_clicked(GtkButton *button, gpointer user_data) {
  (void)button;
  (void)user_data;
  new_entry();
}

static void on_edit_clicked(GtkButton *button, gpointer user_data) {
  (void)button;
  (void)user_data;
  edit_entry();
}

static void on_delete_clicked(GtkButton *button, gpointer user_data) {
  (void)button;
  (void)user_data;
  delete_entry();
}

static void on_browse_clicked(GtkButton *button, gpointer user_data) {
  (void)button;
  (void)user_data;
  browse_selected();
}

static void on_edit_text_clicked(GtkButton *button, gpointer user_data) {
  (void)button;
  (void)user_data;
  edit_text();
}

static void on_refresh_clicked(GtkButton *button, gpointer user_data) {
  (void)button;
  (void)user_data;
  reload(TRUE);
}

static void on_check_clicked(GtkButton *button, gpointer user_data) {
  (void)button;
  (void)user_data;
  check_registry();
}

static GtkWidget *make_button(const char *label, GCallback callback,
                              GtkWidget *box) {
  GtkWidget *button = gtk_button_new_with_label(label);

  gtk_widget_set_halign(button, GTK_ALIGN_FILL);
  g_signal_connect(button, "clicked", callback, NULL);
  gtk_box_append(GTK_BOX(box), button);
  return button;
}

GtkWidget *registry_page_new(void) {
  GtkWidget *page = gtk_box_new(GTK_ORIENTATION_VERTICAL, 8);
  GtkWidget *top = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
  GtkWidget *scroller = gtk_scrolled_window_new();
  GtkWidget *buttons = gtk_box_new(GTK_ORIENTATION_VERTICAL, 6);
  GtkWidget *note;

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
  gtk_widget_set_hexpand(scroller, TRUE);
  gtk_widget_set_vexpand(scroller, TRUE);

  gtk_widget_set_size_request(buttons, 130, -1);
  gtk_widget_set_valign(buttons, GTK_ALIGN_START);
  make_button("New", G_CALLBACK(on_new_clicked), buttons);
  g_edit = make_button("Edit", G_CALLBACK(on_edit_clicked), buttons);
  g_delete = make_button("Delete", G_CALLBACK(on_delete_clicked), buttons);
  g_browse = make_button("Browse...", G_CALLBACK(on_browse_clicked), buttons);
  make_button("Edit text...", G_CALLBACK(on_edit_text_clicked), buttons);
  make_button("Refresh", G_CALLBACK(on_refresh_clicked), buttons);
  make_button("Check", G_CALLBACK(on_check_clicked), buttons);

  gtk_box_append(GTK_BOX(top), scroller);
  gtk_box_append(GTK_BOX(top), buttons);
  gtk_widget_set_vexpand(top, TRUE);

  g_path = gtk_entry_new();
  gtk_editable_set_editable(GTK_EDITABLE(g_path), FALSE);

  note = gtk_label_new(
      "The table is sorted by namespace; Move Up/Down has no meaning here.");
  gtk_label_set_xalign(GTK_LABEL(note), 0.0f);
  gtk_label_set_ellipsize(GTK_LABEL(note), PANGO_ELLIPSIZE_END);

  gtk_box_append(GTK_BOX(page), top);
  gtk_box_append(GTK_BOX(page), g_path);
  gtk_box_append(GTK_BOX(page), note);

  g_signal_connect(g_list, "row-selected", G_CALLBACK(on_row_selected), NULL);
  g_signal_connect(g_list, "row-activated", G_CALLBACK(on_row_activated), NULL);

  registry_list_init(&g_registry);
  update_path_edit();
  return page;
}
