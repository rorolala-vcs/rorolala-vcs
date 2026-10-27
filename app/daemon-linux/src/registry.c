#include "registry.h"

#include <string.h>

#include "process.h"
#include "util.h"

void registry_list_init(RegistryList *list) {
  list->items = NULL;
  list->count = 0;
}

void registry_list_free(RegistryList *list) {
  size_t i;

  for (i = 0; i < list->count; ++i) {
    g_free(list->items[i].namespace_name);
    g_free(list->items[i].path);
  }
  g_free(list->items);
  registry_list_init(list);
}

gboolean registry_is_namespace(const char *name) {
  const char *cursor;

  if (!name || !name[0]) {
    return FALSE;
  }

  if (!g_ascii_isalnum(name[0])) {
    return FALSE;
  }

  for (cursor = name + 1; *cursor; ++cursor) {
    if (g_ascii_isalnum(*cursor) || *cursor == '-' || *cursor == '_') {
      continue;
    }
    return FALSE;
  }
  return TRUE;
}

long registry_find(const RegistryList *list, const char *namespace_name) {
  size_t i;

  for (i = 0; i < list->count; ++i) {
    if (g_strcmp0(list->items[i].namespace_name, namespace_name) == 0) {
      return (long)i;
    }
  }
  return -1;
}

static gboolean append_entry(RegistryList *list, const char *namespace_name,
                             const char *path, gboolean checked,
                             gboolean reachable) {
  RegistryEntry *items;

  items = g_try_renew(RegistryEntry, list->items, list->count + 1);
  if (!items) {
    return FALSE;
  }
  list->items = items;

  list->items[list->count].namespace_name = g_strdup(namespace_name);
  list->items[list->count].path = g_strdup(path);
  list->items[list->count].checked = checked;
  list->items[list->count].reachable = reachable;

  ++list->count;
  return TRUE;
}

static gboolean output_is_empty(const ProcessResult *result) {
  size_t i;

  for (i = 0; i < result->output_len; ++i) {
    char ch = result->output[i];
    if (ch != ' ' && ch != '\t' && ch != '\r' && ch != '\n') {
      return FALSE;
    }
  }
  return TRUE;
}

static void message_from_result(char *message, size_t cap, const char *prefix,
                                const ProcessResult *result) {
  char *text;
  char *trimmed;

  if (!message || cap == 0) {
    return;
  }

  if (output_is_empty(result)) {
    g_snprintf(message, cap, "%s (exit code %d)", prefix, result->exit_code);
    return;
  }

  text = g_strndup(result->output, result->output_len);
  trimmed = str_trim(text);
  str_copy(message, cap, trimmed);
  g_free(text);
}

/* Split one line of the template's output. The path may itself contain '|', so
   the separator is the first one and the rest is kept whole. */
static gboolean parse_pair(const char *line, size_t length,
                           char **namespace_name, char **path) {
  const char *bar = (const char *)memchr(line, '|', length);
  size_t left_length;

  if (!bar) {
    return FALSE;
  }

  left_length = (size_t)(bar - line);
  *namespace_name = g_strndup(line, left_length);
  *path = g_strndup(bar + 1, length - left_length - 1);
  return TRUE;
}

static gboolean parse_triple(const char *line, size_t length,
                             char **namespace_name, char **path,
                             gboolean *reachable) {
  const char *bar;
  const char *last = NULL;
  size_t left_length;
  size_t i;
  char *flag;

  bar = (const char *)memchr(line, '|', length);
  if (!bar) {
    return FALSE;
  }

  /* The path may carry '|' itself, so the flag is what follows the last one. */
  for (i = 0; i < length; ++i) {
    if (line[i] == '|') {
      last = line + i;
    }
  }
  if (!last || last <= bar) {
    return FALSE;
  }

  left_length = (size_t)(bar - line);
  *namespace_name = g_strndup(line, left_length);
  *path = g_strndup(bar + 1, (size_t)(last - bar - 1));
  flag = g_strndup(last + 1, length - (size_t)(last + 1 - line));

  *reachable = g_ascii_strcasecmp(flag, "True") == 0 || strcmp(flag, "1") == 0;
  g_free(flag);
  return TRUE;
}

gboolean registry_list_load(const char *daemon_exe, RegistryList *list,
                            char *error, size_t error_cap) {
  CmdLine args;
  ProcessResult result;
  size_t offset = 0;

  registry_list_free(list);
  if (error && error_cap) {
    error[0] = '\0';
  }

  cmd_init(&args);
  (void)cmd_add(&args, "list");
  (void)cmd_add(&args, "--format");
  (void)cmd_add(&args, "{{namespaces.namespace}}|{{namespaces.path}}");

  process_result_init(&result);
  if (!run_process(daemon_exe, &args, NULL, &result, error, error_cap)) {
    cmd_free(&args);
    return FALSE;
  }
  cmd_free(&args);

  if (result.exit_code != 0) {
    message_from_result(error, error_cap, "rola-daemon list failed", &result);
    process_result_free(&result);
    return FALSE;
  }

  while (offset < result.output_len) {
    const char *line = result.output + offset;
    const char *newline =
        (const char *)memchr(line, '\n', result.output_len - offset);
    size_t length;
    char *namespace_name = NULL;
    char *path = NULL;
    char *trimmed;

    length = newline ? (size_t)(newline - line) : result.output_len - offset;
    while (length > 0 &&
           (line[length - 1] == '\r' || line[length - 1] == ' ')) {
      --length;
    }

    if (length > 0 && parse_pair(line, length, &namespace_name, &path)) {
      trimmed = str_trim(namespace_name);
      if (trimmed[0] && !append_entry(list, trimmed, path, FALSE, FALSE)) {
        g_free(namespace_name);
        g_free(path);
        process_result_free(&result);
        registry_list_free(list);
        if (error && error_cap) {
          str_copy(error, error_cap, "Out of memory.");
        }
        return FALSE;
      }
      g_free(namespace_name);
      g_free(path);
    }

    if (!newline) {
      break;
    }
    offset = (size_t)(newline - result.output) + 1;
  }

  process_result_free(&result);
  return TRUE;
}

gboolean registry_check(const char *daemon_exe, RegistryList *list, char *error,
                        size_t error_cap) {
  CmdLine args;
  ProcessResult result;
  RegistryList checked;
  size_t offset = 0;

  registry_list_init(&checked);
  if (error && error_cap) {
    error[0] = '\0';
  }

  cmd_init(&args);
  (void)cmd_add(&args, "check");
  (void)cmd_add(&args, "--format");
  (void)cmd_add(&args, "{{checks.namespace}}|{{checks.path}}|{{checks.ok}}");

  process_result_init(&result);
  if (!run_process(daemon_exe, &args, NULL, &result, error, error_cap)) {
    cmd_free(&args);
    return FALSE;
  }
  cmd_free(&args);

  while (offset < result.output_len) {
    const char *line = result.output + offset;
    const char *newline =
        (const char *)memchr(line, '\n', result.output_len - offset);
    size_t length =
        newline ? (size_t)(newline - line) : result.output_len - offset;
    char *namespace_name = NULL;
    char *path = NULL;
    gboolean reachable = FALSE;

    while (length > 0 &&
           (line[length - 1] == '\r' || line[length - 1] == ' ')) {
      --length;
    }

    if (length > 0 &&
        parse_triple(line, length, &namespace_name, &path, &reachable)) {
      if (!append_entry(&checked, namespace_name, path, TRUE, reachable)) {
        g_free(namespace_name);
        g_free(path);
        registry_list_free(&checked);
        process_result_free(&result);
        if (error && error_cap) {
          str_copy(error, error_cap, "Out of memory.");
        }
        return FALSE;
      }
      g_free(namespace_name);
      g_free(path);
    }

    if (!newline) {
      break;
    }
    offset = (size_t)(newline - result.output) + 1;
  }

  if (checked.count == 0 && !output_is_empty(&result)) {
    if (error && error_cap) {
      str_copy(error, error_cap,
               "rola-daemon check produced output that could not be read.");
    }
    process_result_free(&result);
    registry_list_free(&checked);
    return FALSE;
  }

  process_result_free(&result);
  registry_list_free(list);
  *list = checked;
  return TRUE;
}

gboolean registry_register(const char *daemon_exe, const char *namespace_name,
                           const char *path, char *message,
                           size_t message_cap) {
  CmdLine args;
  ProcessResult result;
  gboolean ok;

  cmd_init(&args);
  (void)cmd_add(&args, "register");
  (void)cmd_add(&args, namespace_name);
  (void)cmd_add(&args, path);

  process_result_init(&result);
  if (!run_process(daemon_exe, &args, NULL, &result, message, message_cap)) {
    cmd_free(&args);
    return FALSE;
  }
  cmd_free(&args);

  ok = result.exit_code == 0;
  message_from_result(message, message_cap,
                      ok ? "Registered." : "rola-daemon register failed",
                      &result);
  process_result_free(&result);
  return ok;
}

gboolean registry_remove(const char *daemon_exe, const char *namespace_name,
                         char *message, size_t message_cap) {
  CmdLine args;
  ProcessResult result;
  gboolean ok;

  cmd_init(&args);
  (void)cmd_add(&args, "remove");
  (void)cmd_add(&args, namespace_name);

  process_result_init(&result);
  if (!run_process(daemon_exe, &args, NULL, &result, message, message_cap)) {
    cmd_free(&args);
    return FALSE;
  }
  cmd_free(&args);

  ok = result.exit_code == 0;
  message_from_result(message, message_cap,
                      ok ? "Removed." : "rola-daemon remove failed", &result);
  process_result_free(&result);
  return ok;
}

gboolean registry_update(const char *daemon_exe, const char *old_namespace,
                         const char *new_namespace, const char *new_path,
                         char *message, size_t message_cap) {
  char remove_message[2048];
  char register_message[2048];
  char rollback_message[2048];
  char *old_path = NULL;
  RegistryList current;
  long index;
  gboolean ok;
  gboolean restored = FALSE;

  registry_list_init(&current);
  if (registry_list_load(daemon_exe, &current, remove_message,
                         sizeof(remove_message))) {
    index = registry_find(&current, old_namespace);
    if (index >= 0) {
      old_path = g_strdup(current.items[index].path);
    }
  }

  if (!registry_remove(daemon_exe, old_namespace, remove_message,
                       sizeof(remove_message))) {
    if (message && message_cap) {
      str_copy(message, message_cap, remove_message);
    }
    g_free(old_path);
    registry_list_free(&current);
    return FALSE;
  }

  ok = registry_register(daemon_exe, new_namespace, new_path, register_message,
                         sizeof(register_message));
  if (!ok && old_path) {
    /* Put the old row back when the replacement is refused, so a mistyped path
       does not leave the namespace unserved. */
    (void)registry_remove(daemon_exe, new_namespace, rollback_message,
                          sizeof(rollback_message));
    restored = registry_register(daemon_exe, old_namespace, old_path,
                                 rollback_message, sizeof(rollback_message));
  }

  if (message && message_cap) {
    StrBuf combined;

    sb_init(&combined);
    sb_append(&combined, register_message);
    if (!ok && old_path) {
      sb_append(&combined, "\n");
      sb_append(&combined, restored
                               ? "The previous entry was restored."
                               : "The previous entry could not be restored.");
    }
    str_copy(message, message_cap, combined.data ? combined.data : "");
    sb_free(&combined);
  }

  g_free(old_path);
  registry_list_free(&current);
  return ok;
}
