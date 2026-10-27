#include "keys.h"

#include <string.h>

#include "process.h"
#include "util.h"

void key_list_init(KeyList *list) {
  list->items = NULL;
  list->count = 0;
}

void key_list_free(KeyList *list) {
  size_t i;

  for (i = 0; i < list->count; ++i) {
    g_free(list->items[i].path);
  }
  g_free(list->items);
  key_list_init(list);
}

gboolean key_is_valid_name(const char *name) {
  const char *cursor;

  if (!name || !name[0] || strcmp(name, ".") == 0 || strcmp(name, "..") == 0) {
    return FALSE;
  }

  for (cursor = name; *cursor; ++cursor) {
    char ch = *cursor;
    if (g_ascii_isalnum(ch) || ch == '_' || ch == '-' || ch == '@' ||
        ch == '.') {
      continue;
    }
    return FALSE;
  }
  return TRUE;
}

static gboolean append_key(KeyList *list, const char *path,
                           gboolean private_key) {
  KeyEntry *items;

  items = g_try_renew(KeyEntry, list->items, list->count + 1);
  if (!items) {
    return FALSE;
  }
  list->items = items;

  list->items[list->count].path = g_strdup(path);
  list->items[list->count].private_key = private_key;

  ++list->count;
  return TRUE;
}

static gboolean load_one(const char *rola_exe, gboolean private_key,
                         KeyList *list, char *error, size_t error_cap) {
  CmdLine args;
  ProcessResult result;
  size_t offset = 0;

  cmd_init(&args);
  (void)cmd_add(&args, "--no-pager");
  (void)cmd_add(&args, "--no-color");
  (void)cmd_add(&args, "--no-progress");
  (void)cmd_add(&args, "key");
  if (private_key) {
    (void)cmd_add(&args, "--pem");
  }

  process_result_init(&result);
  if (!run_process(rola_exe, &args, NULL, &result, error, error_cap)) {
    cmd_free(&args);
    return FALSE;
  }
  cmd_free(&args);

  if (result.exit_code != 0) {
    if (error && error_cap) {
      char *wide = g_strndup(result.output, result.output_len);
      char *trimmed = str_trim(wide);

      if (trimmed[0]) {
        str_copy(error, error_cap, trimmed);
      } else {
        g_snprintf(error, error_cap, "rola key failed (exit code %d).",
                   result.exit_code);
      }
      g_free(wide);
    }
    process_result_free(&result);
    return FALSE;
  }

  while (offset < result.output_len) {
    const char *line = result.output + offset;
    const char *newline =
        (const char *)memchr(line, '\n', result.output_len - offset);
    size_t length =
        newline ? (size_t)(newline - line) : result.output_len - offset;
    char *path;

    while (length > 0 && (line[length - 1] == '\r' || line[length - 1] == ' ' ||
                          line[length - 1] == '\t')) {
      --length;
    }

    if (length > 0) {
      char *trimmed;

      path = g_strndup(line, length);
      trimmed = str_trim(path);
      if (trimmed[0] && !append_key(list, trimmed, private_key)) {
        g_free(path);
        process_result_free(&result);
        if (error && error_cap) {
          str_copy(error, error_cap, "Out of memory.");
        }
        return FALSE;
      }
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

gboolean keys_load(const char *rola_exe, KeyList *list, char *error,
                   size_t error_cap) {
  key_list_free(list);
  if (error && error_cap) {
    error[0] = '\0';
  }

  if (!load_one(rola_exe, FALSE, list, error, error_cap)) {
    key_list_free(list);
    return FALSE;
  }
  if (!load_one(rola_exe, TRUE, list, error, error_cap)) {
    key_list_free(list);
    return FALSE;
  }
  return TRUE;
}

static gboolean read_header(const char *path, char *header, size_t cap) {
  char *contents = NULL;
  gsize length = 0;
  gsize copy;
  GError *error = NULL;

  if (cap == 0) {
    return FALSE;
  }
  header[0] = '\0';

  if (!g_file_get_contents(path, &contents, &length, &error)) {
    g_clear_error(&error);
    return FALSE;
  }

  copy = MIN(length, cap - 1);
  memcpy(header, contents, copy);
  header[copy] = '\0';
  g_free(contents);
  return TRUE;
}

static void append_line(StrBuf *message, const char *text) {
  if (message->len > 0) {
    sb_append(message, "\n");
  }
  sb_append(message, text);
}

gboolean keys_import_files(GtkWindow *owner, const char **paths, size_t count,
                           char *message, size_t message_cap) {
  char *directory;
  StrBuf report;
  size_t imported = 0;
  size_t skipped = 0;
  size_t failed = 0;
  size_t i;

  if (message && message_cap) {
    message[0] = '\0';
  }
  if (count == 0) {
    return TRUE;
  }

  directory = keys_dir();
  if (!directory) {
    if (message && message_cap) {
      str_copy(message, message_cap,
               "The local data directory could not be found.");
    }
    return FALSE;
  }
  if (!ensure_directory(directory)) {
    if (message && message_cap) {
      char *text = g_strdup_printf(
          "The key directory could not be created:\n%s", directory);
      str_copy(message, message_cap, text);
      g_free(text);
    }
    g_free(directory);
    return FALSE;
  }

  sb_init(&report);
  for (i = 0; i < count; ++i) {
    const char *source = paths[i];
    const char *extension = path_extension(source);
    char *stem;
    char *target;
    char line[PATH_CCH + 128];
    char header[256];
    gboolean private_key = FALSE;

    if (str_ieq(extension, ".pem")) {
      private_key = TRUE;
    } else if (str_ieq(extension, ".pub")) {
      private_key = FALSE;
    } else {
      g_snprintf(line, sizeof(line),
                 "Skipped %s: only .pem and .pub files are imported.", source);
      append_line(&report, line);
      ++skipped;
      continue;
    }

    stem = path_stem(source);
    if (!key_is_valid_name(stem)) {
      g_snprintf(line, sizeof(line), "Skipped %s: the name is not a key name.",
                 source);
      append_line(&report, line);
      g_free(stem);
      ++skipped;
      continue;
    }
    g_free(stem);

    if (!read_header(source, header, sizeof(header)) ||
        !strstr(header, private_key ? "PRIVATE KEY" : "PUBLIC KEY")) {
      g_snprintf(line, sizeof(line),
                 "Skipped %s: the file does not look like a %s key.", source,
                 private_key ? "private" : "public");
      append_line(&report, line);
      ++skipped;
      continue;
    }

    target = path_join(directory, path_filename(source));
    {
      char *canonical_source = g_canonicalize_filename(source, NULL);
      char *canonical_target = g_canonicalize_filename(target, NULL);

      if (g_strcmp0(canonical_source, canonical_target) == 0) {
        g_snprintf(line, sizeof(line),
                   "Skipped %s: it is already in the key directory.", source);
        append_line(&report, line);
        g_free(canonical_source);
        g_free(canonical_target);
        g_free(target);
        ++skipped;
        continue;
      }
      g_free(canonical_source);
      g_free(canonical_target);
    }

    if (path_exists(target) &&
        !ask_yes_no(owner,
                    "%s already exists in the key directory.\nOverwrite it?",
                    path_filename(target))) {
      g_snprintf(line, sizeof(line), "Skipped %s: it already exists.",
                 path_filename(target));
      append_line(&report, line);
      g_free(target);
      ++skipped;
      continue;
    }

    {
      char *contents = NULL;
      gsize length = 0;
      GError *error = NULL;

      if (!g_file_get_contents(source, &contents, &length, &error)) {
        g_snprintf(line, sizeof(line), "Failed to copy %s: %s", source,
                   error ? error->message : "it cannot be read.");
        append_line(&report, line);
        g_clear_error(&error);
        g_free(target);
        ++failed;
        continue;
      }

      if (!g_file_set_contents(target, contents, (gssize)length, &error)) {
        g_snprintf(line, sizeof(line), "Failed to copy %s: %s", source,
                   error ? error->message : "it cannot be written.");
        append_line(&report, line);
        g_clear_error(&error);
        g_free(contents);
        g_free(target);
        ++failed;
        continue;
      }

      g_free(contents);
    }

    g_snprintf(line, sizeof(line), "Imported %s", path_filename(target));
    append_line(&report, line);
    g_free(target);
    ++imported;
  }

  if (message && message_cap) {
    StrBuf summary;

    sb_init(&summary);
    sb_appendf(&summary, "Imported %zu key(s); %zu skipped; %zu failed.",
               imported, skipped, failed);
    if (report.len > 0) {
      sb_append(&summary, "\n\n");
      sb_append(&summary, report.data);
    }
    str_copy(message, message_cap, summary.data ? summary.data : "");
    sb_free(&summary);
  }

  sb_free(&report);
  g_free(directory);
  return failed == 0;
}
