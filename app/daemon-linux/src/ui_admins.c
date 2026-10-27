#include "ui.h"

#include <string.h>

#include "admins.h"
#include "keys.h"
#include "registry.h"

static GtkWidget *g_vault_dropdown;
static GtkWidget *g_list;
static GtkWidget *g_add;
static GtkWidget *g_remove;
static GtkWidget *g_edit;
static GtkWidget *g_note;
static RegistryList g_vaults;
static AdminList g_admins;
static char g_current_vault[PATH_CCH];
static gboolean g_updating_vaults;

typedef struct NameDialogState {
  GtkWidget *dialog;
  GtkWidget *entry;
  GMainLoop *loop;
  int response;
  char name[256];
} NameDialogState;

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
  if (index < 0 || (size_t)index >= g_admins.count) {
    return -1;
  }
  return index;
}

static void update_buttons(void) {
  gboolean has_vault = g_current_vault[0] != '\0';
  gboolean has_rola = g_app.rola_exe[0] != '\0';
  gboolean has_selection = selected_index() >= 0;

  /* A Vault with no administrator has nobody who may name one; the first is
     written by hand through Edit vault.toml. */
  gtk_widget_set_sensitive(g_add, has_vault && has_rola && g_admins.count > 0);
  gtk_widget_set_sensitive(g_remove, has_vault && has_rola && has_selection);
  gtk_widget_set_sensitive(g_edit, has_vault);
}

static const char *selected_vault_path(void) {
  guint selected = gtk_drop_down_get_selected(GTK_DROP_DOWN(g_vault_dropdown));

  if (selected == GTK_INVALID_LIST_POSITION || selected >= g_vaults.count) {
    g_current_vault[0] = '\0';
    return g_current_vault;
  }

  str_copy(g_current_vault, sizeof(g_current_vault),
           g_vaults.items[selected].path);
  return g_current_vault;
}

static void load_admins(void) {
  const char *vault = selected_vault_path();
  char error[4096];
  size_t i;

  clear_list();
  admin_list_free(&g_admins);

  if (!vault[0]) {
    gtk_label_set_text(
        GTK_LABEL(g_note),
        "Administrators may move any file in the Vault's layout.");
    update_buttons();
    app_set_status("No Vault is selected.");
    return;
  }

  gtk_label_set_text(GTK_LABEL(g_note), vault);

  if (!g_app.rola_exe[0]) {
    app_set_status(
        "rola was not found next to rola-daemon or on the program path.");
    update_buttons();
    return;
  }

  if (!admins_load(g_app.rola_exe, vault, &g_admins, error, sizeof(error))) {
    app_set_statusf("Could not read the administrators: %s", error);
    update_buttons();
    return;
  }

  for (i = 0; i < g_admins.count; ++i) {
    add_row(g_admins.items[i]);
  }
  if (g_admins.count > 0) {
    GtkListBoxRow *row = gtk_list_box_get_row_at_index(GTK_LIST_BOX(g_list), 0);

    if (row) {
      gtk_list_box_select_row(GTK_LIST_BOX(g_list), row);
    }
  }

  if (g_admins.count == 0) {
    app_set_status("No administrators. Write the first one in vault.toml.");
  } else {
    app_set_statusf("%zu administrator(s).", g_admins.count);
  }
  update_buttons();
}

void admins_page_vaults_changed(void) {
  char previous[256];
  char error[4096];
  GtkStringList *names;
  guint selected;
  guint next = GTK_INVALID_LIST_POSITION;
  size_t i;

  previous[0] = '\0';
  selected = gtk_drop_down_get_selected(GTK_DROP_DOWN(g_vault_dropdown));
  if (selected != GTK_INVALID_LIST_POSITION && selected < g_vaults.count) {
    str_copy(previous, sizeof(previous),
             g_vaults.items[selected].namespace_name);
  }

  g_updating_vaults = TRUE;
  clear_list();
  admin_list_free(&g_admins);
  registry_list_free(&g_vaults);
  g_current_vault[0] = '\0';

  if (!registry_list_load(g_app.daemon_exe, &g_vaults, error, sizeof(error))) {
    GtkStringList *empty = gtk_string_list_new(NULL);

    gtk_drop_down_set_model(GTK_DROP_DOWN(g_vault_dropdown),
                            G_LIST_MODEL(empty));
    g_object_unref(empty);
    gtk_label_set_text(GTK_LABEL(g_note),
                       "Register a Vault on the Registry tab first.");
    update_buttons();
    app_set_statusf("Could not read the registry: %s", error);
    g_updating_vaults = FALSE;
    return;
  }

  names = gtk_string_list_new(NULL);
  for (i = 0; i < g_vaults.count; ++i) {
    gtk_string_list_append(names, g_vaults.items[i].namespace_name);
  }
  gtk_drop_down_set_model(GTK_DROP_DOWN(g_vault_dropdown), G_LIST_MODEL(names));
  g_object_unref(names);

  for (i = 0; i < g_vaults.count; ++i) {
    if (previous[0] &&
        strcmp(previous, g_vaults.items[i].namespace_name) == 0) {
      next = (guint)i;
      break;
    }
  }

  if (next == GTK_INVALID_LIST_POSITION && g_vaults.count > 0) {
    next = 0;
  }
  gtk_drop_down_set_selected(GTK_DROP_DOWN(g_vault_dropdown), next);
  g_updating_vaults = FALSE;

  load_admins();
}

static void on_vault_changed(GObject *object, GParamSpec *pspec,
                             gpointer user_data) {
  (void)object;
  (void)pspec;
  (void)user_data;

  if (g_updating_vaults) {
    return;
  }
  load_admins();
}

static void on_list_selected(GtkListBox *box, GtkListBoxRow *row,
                             gpointer user_data) {
  (void)box;
  (void)row;
  (void)user_data;
  update_buttons();
}

static void on_suggestion_changed(GObject *object, GParamSpec *pspec,
                                  gpointer user_data) {
  GtkWidget *entry = (GtkWidget *)user_data;
  GtkStringObject *item;

  (void)pspec;

  item = gtk_drop_down_get_selected_item(GTK_DROP_DOWN(object));
  if (item) {
    gtk_editable_set_text(GTK_EDITABLE(entry),
                          gtk_string_object_get_string(item));
  }
}

static void close_name(NameDialogState *state, int response) {
  state->response = response;
  gtk_window_destroy(GTK_WINDOW(state->dialog));
  g_main_loop_quit(state->loop);
}

static void on_name_destroy(GtkWidget *widget, gpointer user_data) {
  NameDialogState *state = (NameDialogState *)user_data;

  (void)widget;
  /* Destroyed with the main window rather than by a close request. */
  if (state->loop) {
    g_main_loop_quit(state->loop);
  }
}

static gboolean on_name_close(GtkWindow *window, gpointer user_data) {
  NameDialogState *state = (NameDialogState *)user_data;

  (void)window;
  state->response = GTK_RESPONSE_CANCEL;
  g_main_loop_quit(state->loop);
  return FALSE;
}

static void on_name_ok(GtkButton *button, gpointer user_data) {
  NameDialogState *state = (NameDialogState *)user_data;

  (void)button;

  {
    char name[256];
    char *trimmed;

    str_copy(name, sizeof(name),
             gtk_editable_get_text(GTK_EDITABLE(state->entry)));
    trimmed = str_trim(name);
    if (!key_is_valid_name(trimmed)) {
      show_error(GTK_WINDOW(state->dialog),
                 "Please type the name of a member key (its file stem).");
      gtk_widget_grab_focus(state->entry);
      return;
    }
    str_copy(state->name, sizeof(state->name), trimmed);
  }

  close_name(state, GTK_RESPONSE_OK);
}

static void on_name_cancel(GtkButton *button, gpointer user_data) {
  (void)button;
  close_name((NameDialogState *)user_data, GTK_RESPONSE_CANCEL);
}

static gboolean ask_name(GtkWindow *owner, const char *title,
                         const char *initial, GtkStringList *suggestions,
                         NameDialogState *state) {
  GtkWidget *dialog;
  GtkWidget *root;
  GtkWidget *grid;
  GtkWidget *name_label;
  GtkWidget *suggestions_label;
  GtkWidget *dropdown;
  GtkWidget *buttons;
  GtkWidget *cancel;
  GtkWidget *ok;

  dialog = gtk_window_new();
  gtk_window_set_title(GTK_WINDOW(dialog), title);
  gtk_window_set_transient_for(GTK_WINDOW(dialog), owner);
  gtk_window_set_modal(GTK_WINDOW(dialog), TRUE);
  gtk_window_set_destroy_with_parent(GTK_WINDOW(dialog), TRUE);
  gtk_window_set_default_size(GTK_WINDOW(dialog), 460, 150);

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

  name_label = gtk_label_new("Member name");
  gtk_widget_set_halign(name_label, GTK_ALIGN_END);
  state->entry = gtk_entry_new();
  gtk_editable_set_text(GTK_EDITABLE(state->entry), initial);
  gtk_widget_set_hexpand(state->entry, TRUE);
  gtk_entry_set_activates_default(GTK_ENTRY(state->entry), TRUE);

  suggestions_label = gtk_label_new("Known keys");
  gtk_widget_set_halign(suggestions_label, GTK_ALIGN_END);
  dropdown = gtk_drop_down_new(G_LIST_MODEL(suggestions), NULL);
  gtk_widget_set_hexpand(dropdown, TRUE);
  gtk_drop_down_set_selected(GTK_DROP_DOWN(dropdown),
                             GTK_INVALID_LIST_POSITION);
  gtk_widget_set_sensitive(
      dropdown, g_list_model_get_n_items(G_LIST_MODEL(suggestions)) > 0);

  gtk_grid_attach(GTK_GRID(grid), name_label, 0, 0, 1, 1);
  gtk_grid_attach(GTK_GRID(grid), state->entry, 1, 0, 1, 1);
  gtk_grid_attach(GTK_GRID(grid), suggestions_label, 0, 1, 1, 1);
  gtk_grid_attach(GTK_GRID(grid), dropdown, 1, 1, 1, 1);

  cancel = gtk_button_new_with_mnemonic("_Cancel");
  ok = gtk_button_new_with_mnemonic("_OK");
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

  g_signal_connect(dialog, "close-request", G_CALLBACK(on_name_close), state);
  g_signal_connect(dialog, "destroy", G_CALLBACK(on_name_destroy), state);
  g_signal_connect(ok, "clicked", G_CALLBACK(on_name_ok), state);
  g_signal_connect(cancel, "clicked", G_CALLBACK(on_name_cancel), state);
  g_signal_connect(dropdown, "notify::selected",
                   G_CALLBACK(on_suggestion_changed), state->entry);

  gtk_widget_grab_focus(state->entry);
  gtk_window_present(GTK_WINDOW(dialog));
  g_main_loop_run(state->loop);
  g_main_loop_unref(state->loop);
  state->loop = NULL;
  state->dialog = NULL;

  return state->response == GTK_RESPONSE_OK;
}

static void add_admin(void) {
  NameDialogState state;
  char message[4096];
  const char *vault = selected_vault_path();
  GtkStringList *suggestions;
  KeyList keys;

  if (!vault[0]) {
    return;
  }

  suggestions = gtk_string_list_new(NULL);
  key_list_init(&keys);
  if (g_app.rola_exe[0] && keys_load(g_app.rola_exe, &keys, NULL, 0)) {
    size_t i;

    for (i = 0; i < keys.count; ++i) {
      char *stem;

      if (keys.items[i].private_key) {
        continue;
      }

      stem = path_stem(keys.items[i].path);
      if (key_is_valid_name(stem)) {
        gtk_string_list_append(suggestions, stem);
      }
      g_free(stem);
    }
  }
  key_list_free(&keys);

  memset(&state, 0, sizeof(state));
  if (!ask_name(GTK_WINDOW(g_app.main), "Add Administrator", "", suggestions,
                &state)) {
    return;
  }

  if (admin_add(g_app.rola_exe, vault, state.name, message, sizeof(message))) {
    app_set_statusf("Added %s.", state.name);
  } else {
    show_error(GTK_WINDOW(g_app.main), "Could not add the administrator:\n%s",
               message);
    app_set_status("Adding the administrator failed.");
  }

  load_admins();
}

static void remove_admin(void) {
  long selected = selected_index();
  const char *vault = selected_vault_path();
  char message[4096];

  if (selected < 0 || !vault[0]) {
    return;
  }

  if (!ask_yes_no(GTK_WINDOW(g_app.main),
                  "Stop \"%s\" being an administrator of this Vault?",
                  g_admins.items[selected])) {
    return;
  }

  if (admin_remove(g_app.rola_exe, vault, g_admins.items[selected], message,
                   sizeof(message))) {
    app_set_statusf("Removed %s.", g_admins.items[selected]);
  } else {
    show_error(GTK_WINDOW(g_app.main),
               "Could not remove the administrator:\n%s", message);
    app_set_status("Removing the administrator failed.");
  }

  load_admins();
}

static void edit_vault_config(void) {
  const char *vault = selected_vault_path();
  char *file;

  if (!vault[0]) {
    return;
  }

  file = path_join(vault, "vault.toml");
  if (!file || !path_exists(file)) {
    show_error(GTK_WINDOW(g_app.main), "vault.toml was not found at:\n%s",
               file ? file : vault);
    g_free(file);
    return;
  }

  if (!open_path(file)) {
    show_error(GTK_WINDOW(g_app.main), "vault.toml could not be opened:\n%s",
               file);
  } else {
    app_set_status("vault.toml opened. Refresh after saving.");
  }

  g_free(file);
}

static void on_refresh_clicked(GtkButton *button, gpointer user_data) {
  (void)button;
  (void)user_data;
  admins_page_vaults_changed();
}

static void on_add_clicked(GtkButton *button, gpointer user_data) {
  (void)button;
  (void)user_data;
  add_admin();
}

static void on_remove_clicked(GtkButton *button, gpointer user_data) {
  (void)button;
  (void)user_data;
  remove_admin();
}

static void on_edit_clicked(GtkButton *button, gpointer user_data) {
  (void)button;
  (void)user_data;
  edit_vault_config();
}

GtkWidget *admins_page_new(void) {
  GtkWidget *page = gtk_box_new(GTK_ORIENTATION_VERTICAL, 8);
  GtkWidget *top = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
  GtkWidget *middle = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
  GtkWidget *scroller = gtk_scrolled_window_new();
  GtkWidget *buttons = gtk_box_new(GTK_ORIENTATION_VERTICAL, 6);
  GtkWidget *vault_label;
  GtkWidget *refresh;
  GtkStringList *empty;

  gtk_widget_set_margin_start(page, 10);
  gtk_widget_set_margin_end(page, 10);
  gtk_widget_set_margin_top(page, 10);
  gtk_widget_set_margin_bottom(page, 10);

  vault_label = gtk_label_new("Vault");
  empty = gtk_string_list_new(NULL);
  /* The constructor takes the model; the dropdown owns it from here. */
  g_vault_dropdown = gtk_drop_down_new(G_LIST_MODEL(empty), NULL);
  gtk_widget_set_hexpand(g_vault_dropdown, TRUE);

  refresh = gtk_button_new_with_label("Refresh");

  gtk_box_append(GTK_BOX(top), vault_label);
  gtk_box_append(GTK_BOX(top), g_vault_dropdown);
  gtk_box_append(GTK_BOX(top), refresh);

  g_list = gtk_list_box_new();
  gtk_list_box_set_selection_mode(GTK_LIST_BOX(g_list), GTK_SELECTION_SINGLE);
  gtk_list_box_set_activate_on_single_click(GTK_LIST_BOX(g_list), FALSE);
  gtk_scrolled_window_set_policy(GTK_SCROLLED_WINDOW(scroller),
                                 GTK_POLICY_AUTOMATIC, GTK_POLICY_AUTOMATIC);
  gtk_scrolled_window_set_has_frame(GTK_SCROLLED_WINDOW(scroller), TRUE);
  gtk_scrolled_window_set_child(GTK_SCROLLED_WINDOW(scroller), g_list);
  gtk_widget_set_hexpand(scroller, TRUE);
  gtk_widget_set_vexpand(scroller, TRUE);

  gtk_widget_set_size_request(buttons, 150, -1);
  gtk_widget_set_valign(buttons, GTK_ALIGN_START);

  g_add = gtk_button_new_with_label("Add...");
  g_remove = gtk_button_new_with_label("Remove");
  g_edit = gtk_button_new_with_label("Edit vault.toml...");
  gtk_widget_set_halign(g_add, GTK_ALIGN_FILL);
  gtk_widget_set_halign(g_remove, GTK_ALIGN_FILL);
  gtk_widget_set_halign(g_edit, GTK_ALIGN_FILL);
  gtk_box_append(GTK_BOX(buttons), g_add);
  gtk_box_append(GTK_BOX(buttons), g_remove);
  gtk_box_append(GTK_BOX(buttons), g_edit);

  gtk_box_append(GTK_BOX(middle), scroller);
  gtk_box_append(GTK_BOX(middle), buttons);
  gtk_widget_set_vexpand(middle, TRUE);

  g_note =
      gtk_label_new("Administrators may move any file in the Vault's layout.");
  gtk_label_set_xalign(GTK_LABEL(g_note), 0.0f);
  gtk_label_set_ellipsize(GTK_LABEL(g_note), PANGO_ELLIPSIZE_END);

  gtk_box_append(GTK_BOX(page), top);
  gtk_box_append(GTK_BOX(page), middle);
  gtk_box_append(GTK_BOX(page), g_note);

  g_signal_connect(g_vault_dropdown, "notify::selected",
                   G_CALLBACK(on_vault_changed), NULL);
  g_signal_connect(g_list, "row-selected", G_CALLBACK(on_list_selected), NULL);
  g_signal_connect(refresh, "clicked", G_CALLBACK(on_refresh_clicked), NULL);
  g_signal_connect(g_add, "clicked", G_CALLBACK(on_add_clicked), NULL);
  g_signal_connect(g_remove, "clicked", G_CALLBACK(on_remove_clicked), NULL);
  g_signal_connect(g_edit, "clicked", G_CALLBACK(on_edit_clicked), NULL);

  registry_list_init(&g_vaults);
  admin_list_init(&g_admins);
  g_current_vault[0] = '\0';
  g_updating_vaults = FALSE;
  update_buttons();
  return page;
}
