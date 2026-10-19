#pragma once

#include <stddef.h>
#include <windows.h>

#define PATH_CCH 2048
#define ARRAY_COUNT(a) (sizeof(a) / sizeof((a)[0]))

/* A growable UTF-16 string. Every function that returns BOOL leaves the buffer
   unchanged when it fails. */
typedef struct StrBuf {
  WCHAR *data;
  size_t len;
  size_t cap;
} StrBuf;

void sb_init(StrBuf *sb);
void sb_free(StrBuf *sb);
void sb_clear(StrBuf *sb);
BOOL sb_reserve(StrBuf *sb, size_t extra);
BOOL sb_append(StrBuf *sb, const WCHAR *text);
BOOL sb_append_n(StrBuf *sb, const WCHAR *text, size_t count);
BOOL sb_append_char(StrBuf *sb, WCHAR ch);
BOOL sb_appendf(StrBuf *sb, const WCHAR *format, ...);
WCHAR *sb_detach(StrBuf *sb);

WCHAR *str_dup(const WCHAR *text);
BOOL str_ieq(const WCHAR *left, const WCHAR *right);
BOOL str_iends_with(const WCHAR *text, const WCHAR *suffix);
WCHAR *str_trim(WCHAR *text);
BOOL str_copy(WCHAR *target, size_t cap, const WCHAR *source);
WCHAR *path_join(const WCHAR *left, const WCHAR *right);
WCHAR *path_dirname(const WCHAR *path);
const WCHAR *path_filename(const WCHAR *path);
WCHAR *path_stem(const WCHAR *path);
const WCHAR *path_extension(const WCHAR *path);
BOOL path_exists(const WCHAR *path);
BOOL path_is_dir(const WCHAR *path);
BOOL ensure_directory(const WCHAR *path);
BOOL file_is_readable(const WCHAR *path);

WCHAR *module_dir(void);
WCHAR *local_app_data_dir(void);
WCHAR *keys_dir(void);
WCHAR *registry_file(void);

void show_error(HWND owner, const WCHAR *format, ...);
void show_info(HWND owner, const WCHAR *format, ...);
BOOL ask_yes_no(HWND owner, const WCHAR *format, ...);
BOOL choose_folder(HWND owner, const WCHAR *title, WCHAR *out, size_t cap);
WCHAR *utf8_to_wide(const char *text);
WCHAR *utf8_to_wide_n(const char *text, size_t count);
char *wide_to_utf8(const WCHAR *text);
