#include "keys.h"

#include <stdlib.h>
#include <string.h>
#include <wchar.h>

#include "process.h"

void key_list_init(KeyList *list) {
  list->items = NULL;
  list->count = 0;
}

/* Every slot below `count` is filled by append_key; the analyzer cannot see
   that across the realloc, so the warning is suppressed here rather than
   lowering /W4. */
#pragma warning(push)
#pragma warning(disable : 6001)
void key_list_free(KeyList *list) {
  size_t i;

  if (!list->items) {
    list->count = 0;
    return;
  }

  for (i = 0; i < list->count; ++i) {
    free(list->items[i].path);
  }
  free(list->items);
  key_list_init(list);
}
#pragma warning(pop)

BOOL key_is_valid_name(const WCHAR *name) {
  const WCHAR *cursor;

  if (!name || !name[0] || wcscmp(name, L".") == 0 ||
      wcscmp(name, L"..") == 0) {
    return FALSE;
  }

  for (cursor = name; *cursor; ++cursor) {
    WCHAR ch = *cursor;
    if ((ch >= L'0' && ch <= L'9') || (ch >= L'A' && ch <= L'Z') ||
        (ch >= L'a' && ch <= L'z') || ch == L'_' || ch == L'-' || ch == L'@' ||
        ch == L'.') {
      continue;
    }
    return FALSE;
  }
  return TRUE;
}

static BOOL append_key(KeyList *list, const WCHAR *path, BOOL private_key) {
  KeyEntry *grown;

  grown =
      (KeyEntry *)realloc(list->items, (list->count + 1) * sizeof(KeyEntry));
  if (!grown) {
    return FALSE;
  }
  list->items = grown;

  list->items[list->count].path = str_dup(path);
  list->items[list->count].private_key = private_key;
  if (!list->items[list->count].path) {
    return FALSE;
  }

  ++list->count;
  return TRUE;
}

static BOOL load_one(const WCHAR *rola_exe, BOOL private_key, KeyList *list,
                     WCHAR *error, size_t error_cap) {
  CmdLine args;
  ProcessResult result;
  size_t offset = 0;

  cmd_init(&args);
  if (!cmd_add(&args, L"--no-pager") || !cmd_add(&args, L"--no-color") ||
      !cmd_add(&args, L"--no-progress") || !cmd_add(&args, L"key") ||
      (private_key && !cmd_add(&args, L"--pem"))) {
    cmd_free(&args);
    if (error && error_cap) {
      str_copy(error, error_cap, L"Out of memory.");
    }
    return FALSE;
  }

  process_result_init(&result);
  if (!run_process(rola_exe, args.text.data, NULL, &result, error, error_cap)) {
    cmd_free(&args);
    return FALSE;
  }
  cmd_free(&args);

  if (result.exit_code != 0) {
    WCHAR *wide = process_output_wide(&result);
    if (error && error_cap) {
      if (wide) {
        str_trim(wide);
        str_copy(error, error_cap, wide);
      } else {
        _snwprintf_s(error, error_cap, _TRUNCATE,
                     L"rola key failed (exit code %lu).",
                     (unsigned long)result.exit_code);
      }
    }
    free(wide);
    process_result_free(&result);
    return FALSE;
  }

  while (offset < result.output_len) {
    const char *line = result.output + offset;
    const char *newline =
        (const char *)memchr(line, '\n', result.output_len - offset);
    size_t length =
        newline ? (size_t)(newline - line) : result.output_len - offset;
    WCHAR *path;

    while (length > 0 && (line[length - 1] == '\r' || line[length - 1] == ' ' ||
                          line[length - 1] == '\t')) {
      --length;
    }

    if (length > 0) {
      path = utf8_to_wide_n(line, length);
      if (!path) {
        process_result_free(&result);
        if (error && error_cap) {
          str_copy(error, error_cap, L"Out of memory.");
        }
        return FALSE;
      }
      str_trim(path);
      if (path[0] && !append_key(list, path, private_key)) {
        free(path);
        process_result_free(&result);
        if (error && error_cap) {
          str_copy(error, error_cap, L"Out of memory.");
        }
        return FALSE;
      }
      free(path);
    }

    if (!newline) {
      break;
    }
    offset = (size_t)(newline - result.output) + 1;
  }

  process_result_free(&result);
  return TRUE;
}

BOOL keys_load(const WCHAR *rola_exe, KeyList *list, WCHAR *error,
               size_t error_cap) {
  key_list_free(list);
  if (error && error_cap) {
    error[0] = L'\0';
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

static BOOL read_header(const WCHAR *path, char *header, size_t cap) {
  HANDLE file;
  DWORD read = 0;

  if (cap == 0) {
    return FALSE;
  }
  header[0] = '\0';

  file = CreateFileW(path, GENERIC_READ,
                     FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                     NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
  if (file == INVALID_HANDLE_VALUE) {
    return FALSE;
  }

  if (!ReadFile(file, header, (DWORD)(cap - 1), &read, NULL)) {
    CloseHandle(file);
    return FALSE;
  }
  CloseHandle(file);
  header[read] = '\0';
  return TRUE;
}

static void append_line(StrBuf *message, const WCHAR *text) {
  if (message->len > 0) {
    sb_append(message, L"\r\n");
  }
  sb_append(message, text);
}

BOOL keys_import_files(HWND owner, const WCHAR **paths, size_t count,
                       WCHAR *message, size_t message_cap) {
  WCHAR *directory;
  StrBuf report;
  size_t imported = 0;
  size_t skipped = 0;
  size_t failed = 0;
  size_t i;

  if (message && message_cap) {
    message[0] = L'\0';
  }
  if (count == 0) {
    return TRUE;
  }

  directory = keys_dir();
  if (!directory) {
    if (message && message_cap) {
      str_copy(message, message_cap,
               L"The local data directory could not be found.");
    }
    return FALSE;
  }
  if (!ensure_directory(directory)) {
    if (message && message_cap) {
      _snwprintf_s(message, message_cap, _TRUNCATE,
                   L"The key directory could not be created:\r\n%s", directory);
    }
    free(directory);
    return FALSE;
  }

  sb_init(&report);
  for (i = 0; i < count; ++i) {
    const WCHAR *source = paths[i];
    const WCHAR *extension = path_extension(source);
    WCHAR *stem;
    WCHAR *target;
    WCHAR line[PATH_CCH + 128];
    char header[256];
    BOOL private_key;

    if (_wcsicmp(extension, L".pem") == 0) {
      private_key = TRUE;
    } else if (_wcsicmp(extension, L".pub") == 0) {
      private_key = FALSE;
    } else {
      _snwprintf_s(line, ARRAY_COUNT(line), _TRUNCATE,
                   L"Skipped %s: only .pem and .pub files are imported.",
                   source);
      append_line(&report, line);
      ++skipped;
      continue;
    }

    stem = path_stem(source);
    if (!stem || !key_is_valid_name(stem)) {
      _snwprintf_s(line, ARRAY_COUNT(line), _TRUNCATE,
                   L"Skipped %s: the name is not a key name.", source);
      append_line(&report, line);
      free(stem);
      ++skipped;
      continue;
    }
    free(stem);

    if (!read_header(source, header, sizeof(header)) ||
        !strstr(header, private_key ? "PRIVATE KEY" : "PUBLIC KEY")) {
      _snwprintf_s(line, ARRAY_COUNT(line), _TRUNCATE,
                   L"Skipped %s: the file does not look like a %s key.", source,
                   private_key ? L"private" : L"public");
      append_line(&report, line);
      ++skipped;
      continue;
    }

    target = path_join(directory, path_filename(source));
    if (!target) {
      ++failed;
      append_line(&report, L"Out of memory while copying a key.");
      continue;
    }

    if (str_ieq(source, target)) {
      _snwprintf_s(line, ARRAY_COUNT(line), _TRUNCATE,
                   L"Skipped %s: it is already in the key directory.", source);
      append_line(&report, line);
      free(target);
      ++skipped;
      continue;
    }

    if (path_exists(target) &&
        !ask_yes_no(owner,
                    L"%s already exists in the key directory.\r\nOverwrite it?",
                    path_filename(target))) {
      _snwprintf_s(line, ARRAY_COUNT(line), _TRUNCATE,
                   L"Skipped %s: it already exists.", path_filename(target));
      append_line(&report, line);
      free(target);
      ++skipped;
      continue;
    }

    if (!CopyFileW(source, target, FALSE)) {
      _snwprintf_s(line, ARRAY_COUNT(line), _TRUNCATE,
                   L"Failed to copy %s (Windows error %lu).", source,
                   (unsigned long)GetLastError());
      append_line(&report, line);
      free(target);
      ++failed;
      continue;
    }

    _snwprintf_s(line, ARRAY_COUNT(line), _TRUNCATE, L"Imported %s",
                 path_filename(target));
    append_line(&report, line);
    free(target);
    ++imported;
  }

  if (message && message_cap) {
    StrBuf summary;
    sb_init(&summary);
    sb_appendf(&summary, L"Imported %llu key(s); %llu skipped; %llu failed.",
               (unsigned long long)imported, (unsigned long long)skipped,
               (unsigned long long)failed);
    if (report.len > 0) {
      sb_append(&summary, L"\r\n\r\n");
      sb_append(&summary, report.data);
    }
    str_copy(message, message_cap, summary.data ? summary.data : L"");
    sb_free(&summary);
  }

  sb_free(&report);
  free(directory);
  return failed == 0;
}
