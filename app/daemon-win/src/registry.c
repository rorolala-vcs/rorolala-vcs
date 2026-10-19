#include "registry.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <wctype.h>

#include "process.h"

void registry_list_init(RegistryList *list) {
  list->items = NULL;
  list->count = 0;
}

/* Every slot below `count` is filled by append_entry; the analyzer cannot see
   that across the realloc, so the warning is suppressed here rather than
   lowering /W4. */
#pragma warning(push)
#pragma warning(disable : 6001)
void registry_list_free(RegistryList *list) {
  size_t i;

  if (!list->items) {
    list->count = 0;
    return;
  }

  for (i = 0; i < list->count; ++i) {
    free(list->items[i].namespace_name);
    free(list->items[i].path);
  }
  free(list->items);
  registry_list_init(list);
}
#pragma warning(pop)

BOOL registry_is_namespace(const WCHAR *name) {
  const WCHAR *cursor;

  if (!name || !name[0]) {
    return FALSE;
  }

  if (!((name[0] >= L'0' && name[0] <= L'9') ||
        (name[0] >= L'A' && name[0] <= L'Z') ||
        (name[0] >= L'a' && name[0] <= L'z'))) {
    return FALSE;
  }

  for (cursor = name + 1; *cursor; ++cursor) {
    if ((*cursor >= L'0' && *cursor <= L'9') ||
        (*cursor >= L'A' && *cursor <= L'Z') ||
        (*cursor >= L'a' && *cursor <= L'z') || *cursor == L'-' ||
        *cursor == L'_') {
      continue;
    }
    return FALSE;
  }
  return TRUE;
}

long registry_find(const RegistryList *list, const WCHAR *namespace_name) {
  size_t i;

  for (i = 0; i < list->count; ++i) {
    if (wcscmp(list->items[i].namespace_name, namespace_name) == 0) {
      return (long)i;
    }
  }
  return -1;
}

static BOOL append_entry(RegistryList *list, const WCHAR *namespace_name,
                         const WCHAR *path, BOOL checked, BOOL reachable) {
  RegistryEntry *grown;

  grown = (RegistryEntry *)realloc(list->items,
                                   (list->count + 1) * sizeof(RegistryEntry));
  if (!grown) {
    return FALSE;
  }
  list->items = grown;

  list->items[list->count].namespace_name = str_dup(namespace_name);
  list->items[list->count].path = str_dup(path);
  list->items[list->count].checked = checked;
  list->items[list->count].reachable = reachable;

  if (!list->items[list->count].namespace_name ||
      !list->items[list->count].path) {
    free(list->items[list->count].namespace_name);
    free(list->items[list->count].path);
    return FALSE;
  }

  ++list->count;
  return TRUE;
}

static BOOL output_is_empty(const ProcessResult *result) {
  size_t i;

  for (i = 0; i < result->output_len; ++i) {
    char ch = result->output[i];
    if (ch != ' ' && ch != '\t' && ch != '\r' && ch != '\n') {
      return FALSE;
    }
  }
  return TRUE;
}

static void message_from_result(WCHAR *message, size_t cap, const WCHAR *prefix,
                                const ProcessResult *result) {
  WCHAR *wide;

  if (!message || cap == 0) {
    return;
  }

  if (output_is_empty(result)) {
    _snwprintf_s(message, cap, _TRUNCATE, L"%s (exit code %lu)", prefix,
                 (unsigned long)result->exit_code);
    return;
  }

  wide = process_output_wide(result);
  if (!wide) {
    str_copy(message, cap, prefix);
    return;
  }

  str_trim(wide);
  str_copy(message, cap, wide);
  free(wide);
}

/* Split one line of the template's output. The path may itself contain '|', so
   the separator is the first one and the rest is kept whole. */
static BOOL parse_pair(const char *line, size_t length, WCHAR **namespace_name,
                       WCHAR **path) {
  const char *bar;
  size_t left_length;
  WCHAR *left;
  WCHAR *right;

  bar = (const char *)memchr(line, '|', length);
  if (!bar) {
    return FALSE;
  }

  left_length = (size_t)(bar - line);
  left = utf8_to_wide_n(line, left_length);
  right = utf8_to_wide_n(bar + 1, length - left_length - 1);
  if (!left || !right) {
    free(left);
    free(right);
    return FALSE;
  }

  *namespace_name = left;
  *path = right;
  return TRUE;
}

static BOOL parse_triple(const char *line, size_t length,
                         WCHAR **namespace_name, WCHAR **path,
                         BOOL *reachable) {
  const char *bar;
  const char *last = NULL;
  size_t left_length;
  size_t i;
  WCHAR *left;
  WCHAR *right;
  WCHAR *flag;
  BOOL ok;

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
  left = utf8_to_wide_n(line, left_length);
  right = utf8_to_wide_n(bar + 1, (size_t)(last - bar - 1));
  flag = utf8_to_wide_n(last + 1, length - (size_t)(last + 1 - line));
  if (!left || !right || !flag) {
    free(left);
    free(right);
    free(flag);
    return FALSE;
  }

  ok = _wcsicmp(flag, L"True") == 0 || wcscmp(flag, L"1") == 0;
  free(flag);

  *namespace_name = left;
  *path = right;
  *reachable = ok;
  return TRUE;
}

BOOL registry_list_load(const WCHAR *daemon_exe, RegistryList *list,
                        WCHAR *error, size_t error_cap) {
  CmdLine args;
  ProcessResult result;
  size_t offset = 0;
  BOOL ok = FALSE;

  registry_list_free(list);
  if (error && error_cap) {
    error[0] = L'\0';
  }

  cmd_init(&args);
  if (!cmd_add(&args, L"list") || !cmd_add(&args, L"--format") ||
      !cmd_add(&args, L"{{namespaces.namespace}}|{{namespaces.path}}")) {
    cmd_free(&args);
    if (error && error_cap) {
      str_copy(error, error_cap, L"Out of memory.");
    }
    return FALSE;
  }

  process_result_init(&result);
  if (!run_process(daemon_exe, args.text.data, NULL, &result, error,
                   error_cap)) {
    cmd_free(&args);
    return FALSE;
  }
  cmd_free(&args);

  if (result.exit_code != 0) {
    message_from_result(error, error_cap, L"rola-daemon list failed", &result);
    process_result_free(&result);
    return FALSE;
  }

  while (offset < result.output_len) {
    const char *line = result.output + offset;
    const char *newline =
        (const char *)memchr(line, '\n', result.output_len - offset);
    size_t length;
    WCHAR *namespace_name = NULL;
    WCHAR *path = NULL;
    WCHAR *trimmed;

    length = newline ? (size_t)(newline - line) : result.output_len - offset;
    while (length > 0 &&
           (line[length - 1] == '\r' || line[length - 1] == ' ')) {
      --length;
    }

    if (length > 0 && parse_pair(line, length, &namespace_name, &path)) {
      trimmed = str_trim(namespace_name);
      if (trimmed[0]) {
        if (!append_entry(list, trimmed, path, FALSE, FALSE)) {
          free(namespace_name);
          free(path);
          if (error && error_cap) {
            str_copy(error, error_cap, L"Out of memory.");
          }
          process_result_free(&result);
          return FALSE;
        }
      }
      free(namespace_name);
      free(path);
    }

    if (!newline) {
      break;
    }
    offset = (size_t)(newline - result.output) + 1;
  }

  ok = TRUE;
  process_result_free(&result);
  return ok;
}

BOOL registry_check(const WCHAR *daemon_exe, RegistryList *list, WCHAR *error,
                    size_t error_cap) {
  CmdLine args;
  ProcessResult result;
  RegistryList checked;
  size_t offset = 0;

  registry_list_init(&checked);
  if (error && error_cap) {
    error[0] = L'\0';
  }

  cmd_init(&args);
  if (!cmd_add(&args, L"check") || !cmd_add(&args, L"--format") ||
      !cmd_add(&args, L"{{checks.namespace}}|{{checks.path}}|{{checks.ok}}")) {
    cmd_free(&args);
    if (error && error_cap) {
      str_copy(error, error_cap, L"Out of memory.");
    }
    return FALSE;
  }

  process_result_init(&result);
  if (!run_process(daemon_exe, args.text.data, NULL, &result, error,
                   error_cap)) {
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
    WCHAR *namespace_name = NULL;
    WCHAR *path = NULL;
    BOOL reachable = FALSE;

    while (length > 0 &&
           (line[length - 1] == '\r' || line[length - 1] == ' ')) {
      --length;
    }

    if (length > 0 &&
        parse_triple(line, length, &namespace_name, &path, &reachable)) {
      if (!append_entry(&checked, namespace_name, path, TRUE, reachable)) {
        free(namespace_name);
        free(path);
        registry_list_free(&checked);
        process_result_free(&result);
        if (error && error_cap) {
          str_copy(error, error_cap, L"Out of memory.");
        }
        return FALSE;
      }
      free(namespace_name);
      free(path);
    }

    if (!newline) {
      break;
    }
    offset = (size_t)(newline - result.output) + 1;
  }

  if (checked.count == 0 && !output_is_empty(&result)) {
    if (error && error_cap) {
      str_copy(error, error_cap,
               L"rola-daemon check produced output that could not be read.");
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

BOOL registry_register(const WCHAR *daemon_exe, const WCHAR *namespace_name,
                       const WCHAR *path, WCHAR *message, size_t message_cap) {
  CmdLine args;
  ProcessResult result;
  BOOL ok;

  cmd_init(&args);
  if (!cmd_add(&args, L"register") || !cmd_add(&args, namespace_name) ||
      !cmd_add(&args, path)) {
    cmd_free(&args);
    if (message && message_cap) {
      str_copy(message, message_cap, L"Out of memory.");
    }
    return FALSE;
  }

  process_result_init(&result);
  if (!run_process(daemon_exe, args.text.data, NULL, &result, message,
                   message_cap)) {
    cmd_free(&args);
    return FALSE;
  }
  cmd_free(&args);

  ok = result.exit_code == 0;
  message_from_result(message, message_cap,
                      ok ? L"Registered." : L"rola-daemon register failed",
                      &result);
  process_result_free(&result);
  return ok;
}

BOOL registry_remove(const WCHAR *daemon_exe, const WCHAR *namespace_name,
                     WCHAR *message, size_t message_cap) {
  CmdLine args;
  ProcessResult result;
  BOOL ok;

  cmd_init(&args);
  if (!cmd_add(&args, L"remove") || !cmd_add(&args, namespace_name)) {
    cmd_free(&args);
    if (message && message_cap) {
      str_copy(message, message_cap, L"Out of memory.");
    }
    return FALSE;
  }

  process_result_init(&result);
  if (!run_process(daemon_exe, args.text.data, NULL, &result, message,
                   message_cap)) {
    cmd_free(&args);
    return FALSE;
  }
  cmd_free(&args);

  ok = result.exit_code == 0;
  message_from_result(message, message_cap,
                      ok ? L"Removed." : L"rola-daemon remove failed", &result);
  process_result_free(&result);
  return ok;
}

BOOL registry_update(const WCHAR *daemon_exe, const WCHAR *old_namespace,
                     const WCHAR *new_namespace, const WCHAR *new_path,
                     WCHAR *message, size_t message_cap) {
  WCHAR remove_message[2048];
  WCHAR register_message[2048];
  WCHAR rollback_message[2048];
  WCHAR *old_path = NULL;
  RegistryList current;
  long index;
  StrBuf combined;
  BOOL ok;
  BOOL restored = FALSE;

  registry_list_init(&current);
  if (registry_list_load(daemon_exe, &current, remove_message,
                         ARRAY_COUNT(remove_message))) {
    index = registry_find(&current, old_namespace);
    if (index >= 0) {
      old_path = str_dup(current.items[index].path);
    }
  }

  if (!registry_remove(daemon_exe, old_namespace, remove_message,
                       ARRAY_COUNT(remove_message))) {
    if (message && message_cap) {
      str_copy(message, message_cap, remove_message);
    }
    free(old_path);
    registry_list_free(&current);
    return FALSE;
  }

  ok = registry_register(daemon_exe, new_namespace, new_path, register_message,
                         ARRAY_COUNT(register_message));
  if (!ok && old_path) {
    /* Put the old row back when the replacement is refused, so a mistyped path
       does not leave the namespace unserved. */
    (void)registry_remove(daemon_exe, new_namespace, rollback_message,
                          ARRAY_COUNT(rollback_message));
    restored =
        registry_register(daemon_exe, old_namespace, old_path, rollback_message,
                          ARRAY_COUNT(rollback_message));
  }

  if (message && message_cap) {
    sb_init(&combined);
    sb_append(&combined, register_message);
    if (!ok && old_path) {
      sb_append(&combined, L"\r\n");
      sb_append(&combined, restored
                               ? L"The previous entry was restored."
                               : L"The previous entry could not be restored.");
    }
    str_copy(message, message_cap, combined.data ? combined.data : L"");
    sb_free(&combined);
  }

  free(old_path);
  registry_list_free(&current);
  return ok;
}
