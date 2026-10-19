#include "util.h"

#include <limits.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <wctype.h>

#include <knownfolders.h>
#include <shlobj.h>

static BOOL sb_grow(StrBuf *sb, size_t needed) {
  size_t cap;
  WCHAR *grown;

  if (needed <= sb->cap) {
    return TRUE;
  }

  cap = sb->cap ? sb->cap : 64;
  while (cap < needed) {
    if (cap > (size_t)-1 / 2) {
      return FALSE;
    }
    cap *= 2;
  }

  grown = (WCHAR *)realloc(sb->data, cap * sizeof(WCHAR));
  if (!grown) {
    return FALSE;
  }

  sb->data = grown;
  sb->cap = cap;
  return TRUE;
}

void sb_init(StrBuf *sb) {
  sb->data = NULL;
  sb->len = 0;
  sb->cap = 0;
}

void sb_free(StrBuf *sb) {
  free(sb->data);
  sb_init(sb);
}

void sb_clear(StrBuf *sb) {
  sb->len = 0;
  if (sb->data) {
    sb->data[0] = L'\0';
  }
}

BOOL sb_reserve(StrBuf *sb, size_t extra) {
  if (extra > (size_t)-1 - sb->len - 1) {
    return FALSE;
  }
  return sb_grow(sb, sb->len + extra + 1);
}

BOOL sb_append_n(StrBuf *sb, const WCHAR *text, size_t count) {
  if (!sb_reserve(sb, count)) {
    return FALSE;
  }

  memcpy(sb->data + sb->len, text, count * sizeof(WCHAR));
  sb->len += count;
  sb->data[sb->len] = L'\0';
  return TRUE;
}

BOOL sb_append(StrBuf *sb, const WCHAR *text) {
  return sb_append_n(sb, text, wcslen(text));
}

BOOL sb_append_char(StrBuf *sb, WCHAR ch) { return sb_append_n(sb, &ch, 1); }

BOOL sb_appendf(StrBuf *sb, const WCHAR *format, ...) {
  va_list args;
  va_list copy;
  int length;

  va_start(args, format);
  va_copy(copy, args);
  length = _vscwprintf(format, args);
  va_end(args);

  if (length < 0) {
    va_end(copy);
    return FALSE;
  }

  if (!sb_reserve(sb, (size_t)length)) {
    va_end(copy);
    return FALSE;
  }

  _vsnwprintf_s(sb->data + sb->len, sb->cap - sb->len, _TRUNCATE, format, copy);
  sb->len += (size_t)length;
  va_end(copy);
  return TRUE;
}

WCHAR *sb_detach(StrBuf *sb) {
  WCHAR *text = sb->data;

  if (!text) {
    text = (WCHAR *)calloc(1, sizeof(WCHAR));
    sb_init(sb);
    return text;
  }

  sb->data = NULL;
  sb->len = 0;
  sb->cap = 0;
  return text;
}

WCHAR *str_dup(const WCHAR *text) {
  size_t length;
  WCHAR *copy;

  if (!text) {
    return NULL;
  }

  length = wcslen(text);
  copy = (WCHAR *)malloc((length + 1) * sizeof(WCHAR));
  if (copy) {
    memcpy(copy, text, (length + 1) * sizeof(WCHAR));
  }
  return copy;
}

BOOL str_ieq(const WCHAR *left, const WCHAR *right) {
  return left && right && _wcsicmp(left, right) == 0;
}

BOOL str_iends_with(const WCHAR *text, const WCHAR *suffix) {
  size_t text_length;
  size_t suffix_length;

  if (!text || !suffix) {
    return FALSE;
  }

  text_length = wcslen(text);
  suffix_length = wcslen(suffix);
  if (suffix_length > text_length) {
    return FALSE;
  }

  return _wcsicmp(text + text_length - suffix_length, suffix) == 0;
}

WCHAR *str_trim(WCHAR *text) {
  WCHAR *end;

  if (!text) {
    return NULL;
  }

  while (*text && iswspace(*text)) {
    ++text;
  }

  end = text + wcslen(text);
  while (end > text && iswspace(end[-1])) {
    --end;
  }
  *end = L'\0';
  return text;
}

BOOL str_copy(WCHAR *target, size_t cap, const WCHAR *source) {
  if (!target || cap == 0) {
    return FALSE;
  }
  if (!source) {
    target[0] = L'\0';
    return TRUE;
  }
  if (wcslen(source) >= cap) {
    target[0] = L'\0';
    return FALSE;
  }
  wcscpy_s(target, cap, source);
  return TRUE;
}

static BOOL path_is_absolute(const WCHAR *path) {
  if (!path || !path[0]) {
    return FALSE;
  }
  if (path[0] == L'\\' && path[1] == L'\\') {
    return TRUE;
  }
  return path[1] == L':' && (path[2] == L'\\' || path[2] == L'/');
}

WCHAR *path_join(const WCHAR *left, const WCHAR *right) {
  size_t left_length;
  BOOL needs_separator;
  WCHAR *joined;

  if (!right || !right[0]) {
    return str_dup(left ? left : L"");
  }
  if (!left || !left[0] || path_is_absolute(right)) {
    return str_dup(right);
  }

  left_length = wcslen(left);
  needs_separator =
      left[left_length - 1] != L'\\' && left[left_length - 1] != L'/';
  joined = (WCHAR *)malloc(
      (left_length + (needs_separator ? 1 : 0) + wcslen(right) + 1) *
      sizeof(WCHAR));
  if (!joined) {
    return NULL;
  }

  wcscpy_s(joined, left_length + 1, left);
  if (needs_separator) {
    joined[left_length++] = L'\\';
  }
  wcscpy_s(joined + left_length, wcslen(right) + 1, right);
  return joined;
}

WCHAR *path_dirname(const WCHAR *path) {
  const WCHAR *slash;
  size_t length;
  WCHAR *directory;

  if (!path || !path[0]) {
    return str_dup(L".");
  }

  slash = wcsrchr(path, L'\\');
  if (!slash) {
    slash = wcsrchr(path, L'/');
  }
  if (!slash) {
    return str_dup(L".");
  }

  length = (size_t)(slash - path);
  if (length == 0) {
    length = 1;
  }

  directory = (WCHAR *)malloc((length + 1) * sizeof(WCHAR));
  if (!directory) {
    return NULL;
  }
  memcpy(directory, path, length * sizeof(WCHAR));
  directory[length] = L'\0';
  return directory;
}

const WCHAR *path_filename(const WCHAR *path) {
  const WCHAR *slash;
  const WCHAR *backslash;

  if (!path) {
    return L"";
  }

  slash = wcsrchr(path, L'/');
  backslash = wcsrchr(path, L'\\');
  if (backslash && (!slash || backslash > slash)) {
    slash = backslash;
  }
  return slash ? slash + 1 : path;
}

WCHAR *path_stem(const WCHAR *path) {
  const WCHAR *name = path_filename(path);
  const WCHAR *dot = wcsrchr(name, L'.');
  size_t length;
  WCHAR *stem;

  if (dot && dot > name) {
    length = (size_t)(dot - name);
  } else {
    length = wcslen(name);
  }

  stem = (WCHAR *)malloc((length + 1) * sizeof(WCHAR));
  if (!stem) {
    return NULL;
  }
  memcpy(stem, name, length * sizeof(WCHAR));
  stem[length] = L'\0';
  return stem;
}

const WCHAR *path_extension(const WCHAR *path) {
  const WCHAR *name = path_filename(path);
  const WCHAR *dot = wcsrchr(name, L'.');

  return dot ? dot : L"";
}

BOOL path_exists(const WCHAR *path) {
  DWORD attributes;

  if (!path || !path[0]) {
    return FALSE;
  }
  attributes = GetFileAttributesW(path);
  return attributes != INVALID_FILE_ATTRIBUTES;
}

BOOL path_is_dir(const WCHAR *path) {
  DWORD attributes;

  if (!path || !path[0]) {
    return FALSE;
  }
  attributes = GetFileAttributesW(path);
  return attributes != INVALID_FILE_ATTRIBUTES &&
         (attributes & FILE_ATTRIBUTE_DIRECTORY);
}

BOOL file_is_readable(const WCHAR *path) {
  HANDLE file;
  BOOL readable;

  file = CreateFileW(path, GENERIC_READ,
                     FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                     NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
  if (file == INVALID_HANDLE_VALUE) {
    return FALSE;
  }
  readable = TRUE;
  CloseHandle(file);
  return readable;
}

/* Create every missing component of `path`, leaving an existing one alone. */
BOOL ensure_directory(const WCHAR *path) {
  WCHAR *copy;
  WCHAR *cursor;
  size_t length;

  if (!path || !path[0]) {
    return FALSE;
  }
  if (path_is_dir(path)) {
    return TRUE;
  }

  copy = str_dup(path);
  if (!copy) {
    return FALSE;
  }

  length = wcslen(copy);
  while (length > 0 &&
         (copy[length - 1] == L'\\' || copy[length - 1] == L'/')) {
    copy[--length] = L'\0';
  }

  /* Skip a drive prefix or a UNC share so the forward scan never creates "C:"
   * or "\\server". */
  cursor = copy;
  if (path_is_absolute(copy)) {
    if (copy[0] == L'\\' && copy[1] == L'\\') {
      cursor = copy + 2;
      cursor = wcschr(cursor, L'\\');
      if (cursor) {
        cursor = wcschr(cursor + 1, L'\\');
      }
      if (!cursor) {
        free(copy);
        return FALSE;
      }
      ++cursor;
    } else {
      cursor = copy + 3;
    }
  }

  for (; *cursor; ++cursor) {
    if (*cursor != L'\\' && *cursor != L'/') {
      continue;
    }
    *cursor = L'\0';
    if (!CreateDirectoryW(copy, NULL) &&
        GetLastError() != ERROR_ALREADY_EXISTS) {
      free(copy);
      return FALSE;
    }
    *cursor = L'\\';
  }

  if (!CreateDirectoryW(copy, NULL) && GetLastError() != ERROR_ALREADY_EXISTS) {
    free(copy);
    return FALSE;
  }

  free(copy);
  return TRUE;
}

WCHAR *module_dir(void) {
  WCHAR buffer[PATH_CCH];
  DWORD length = GetModuleFileNameW(NULL, buffer, (DWORD)ARRAY_COUNT(buffer));

  if (length == 0 || length >= ARRAY_COUNT(buffer)) {
    return str_dup(L".");
  }
  buffer[length] = L'\0';
  return path_dirname(buffer);
}

WCHAR *local_app_data_dir(void) {
  PWSTR known = NULL;
  WCHAR buffer[PATH_CCH];
  DWORD length;

  if (SUCCEEDED(SHGetKnownFolderPath(&FOLDERID_LocalAppData, KF_FLAG_DEFAULT,
                                     NULL, &known))) {
    WCHAR *copy = str_dup(known);
    CoTaskMemFree(known);
    return copy;
  }

  length = GetEnvironmentVariableW(L"LOCALAPPDATA", buffer,
                                   (DWORD)ARRAY_COUNT(buffer));
  if (length > 0 && length < ARRAY_COUNT(buffer)) {
    return str_dup(buffer);
  }
  return NULL;
}

WCHAR *keys_dir(void) {
  WCHAR *local = local_app_data_dir();
  WCHAR *result;

  if (!local) {
    return NULL;
  }
  result = path_join(local, L"rola\\keys");
  free(local);
  return result;
}

WCHAR *registry_file(void) {
  WCHAR *local = local_app_data_dir();
  WCHAR *result;

  if (!local) {
    return NULL;
  }
  result = path_join(local, L"rola\\daemon\\registry.toml");
  free(local);
  return result;
}

static BOOL format_into(StrBuf *sb, const WCHAR *format, va_list args) {
  va_list copy;
  int length;

  sb_clear(sb);
  va_copy(copy, args);
  length = _vscwprintf(format, args);
  if (length < 0) {
    va_end(copy);
    return FALSE;
  }
  if (!sb_reserve(sb, (size_t)length)) {
    va_end(copy);
    return FALSE;
  }
  _vsnwprintf_s(sb->data, sb->cap, _TRUNCATE, format, copy);
  sb->len = (size_t)length;
  va_end(copy);
  return TRUE;
}

static int message_box_v(HWND owner, UINT type, const WCHAR *format,
                         va_list args) {
  StrBuf text;
  int result;

  sb_init(&text);
  if (!format_into(&text, format, args)) {
    sb_free(&text);
    return 0;
  }
  result = MessageBoxW(owner, text.data, L"Rorolala Daemon", type);
  sb_free(&text);
  return result;
}

void show_error(HWND owner, const WCHAR *format, ...) {
  va_list args;

  va_start(args, format);
  (void)message_box_v(owner, MB_OK | MB_ICONERROR, format, args);
  va_end(args);
}

void show_info(HWND owner, const WCHAR *format, ...) {
  va_list args;

  va_start(args, format);
  (void)message_box_v(owner, MB_OK | MB_ICONINFORMATION, format, args);
  va_end(args);
}

BOOL ask_yes_no(HWND owner, const WCHAR *format, ...) {
  va_list args;
  int result;

  va_start(args, format);
  result = message_box_v(owner, MB_YESNO | MB_ICONQUESTION, format, args);
  va_end(args);
  return result == IDYES;
}

BOOL choose_folder(HWND owner, const WCHAR *title, WCHAR *out, size_t cap) {
  BROWSEINFOW browse;
  LPITEMIDLIST item;
  BOOL chosen = FALSE;

  if (!out || cap == 0) {
    return FALSE;
  }
  out[0] = L'\0';

  ZeroMemory(&browse, sizeof(browse));
  browse.hwndOwner = owner;
  browse.lpszTitle = title;
  browse.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_USENEWUI;

  item = SHBrowseForFolderW(&browse);
  if (item) {
    chosen = SHGetPathFromIDListW(item, out);
    CoTaskMemFree(item);
  }
  return chosen;
}

static WCHAR *decode_bytes(UINT codepage, DWORD flags, const char *text,
                           int count) {
  int length;
  WCHAR *wide;

  length = MultiByteToWideChar(codepage, flags, text, count, NULL, 0);
  if (length <= 0) {
    return NULL;
  }

  wide = (WCHAR *)malloc(((size_t)length + 1) * sizeof(WCHAR));
  if (!wide) {
    return NULL;
  }

  if (MultiByteToWideChar(codepage, flags, text, count, wide, length) !=
      length) {
    free(wide);
    return NULL;
  }
  wide[length] = L'\0';
  return wide;
}

WCHAR *utf8_to_wide_n(const char *text, size_t count) {
  WCHAR *wide;

  if (!text) {
    return NULL;
  }
  if (count > (size_t)INT_MAX) {
    return NULL;
  }
  if (count == 0) {
    wide = (WCHAR *)calloc(1, sizeof(WCHAR));
    return wide;
  }

  wide = decode_bytes(CP_UTF8, MB_ERR_INVALID_CHARS, text, (int)count);
  if (!wide) {
    wide = decode_bytes(CP_ACP, 0, text, (int)count);
  }
  return wide;
}

WCHAR *utf8_to_wide(const char *text) {
  if (!text) {
    return NULL;
  }
  return utf8_to_wide_n(text, strlen(text));
}

char *wide_to_utf8(const WCHAR *text) {
  int length;
  char *narrow;

  if (!text) {
    return NULL;
  }

  length = WideCharToMultiByte(CP_UTF8, 0, text, -1, NULL, 0, NULL, NULL);
  if (length <= 0) {
    return NULL;
  }

  narrow = (char *)malloc((size_t)length);
  if (!narrow) {
    return NULL;
  }
  if (WideCharToMultiByte(CP_UTF8, 0, text, -1, narrow, length, NULL, NULL) !=
      length) {
    free(narrow);
    return NULL;
  }
  return narrow;
}
