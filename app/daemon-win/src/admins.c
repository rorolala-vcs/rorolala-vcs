#include "admins.h"

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "process.h"

void admin_list_init(AdminList *list) {
  list->items = NULL;
  list->count = 0;
}

void admin_list_free(AdminList *list) {
  size_t i;

  if (!list->items) {
    list->count = 0;
    return;
  }

  for (i = 0; i < list->count; ++i) {
    free(list->items[i]);
  }
  free(list->items);
  admin_list_init(list);
}

static BOOL append_admin(AdminList *list, WCHAR *name) {
  WCHAR **grown;

  grown = (WCHAR **)realloc(list->items, (list->count + 1) * sizeof(WCHAR *));
  if (!grown) {
    return FALSE;
  }
  list->items = grown;
  list->items[list->count++] = name;
  return TRUE;
}

static void append_base(CmdLine *args, const WCHAR *vault_path) {
  (void)cmd_add(args, L"--no-pager");
  (void)cmd_add(args, L"--no-color");
  (void)cmd_add(args, L"--no-progress");
  (void)cmd_add(args, L"--lang");
  (void)cmd_add(args, L"en");
  (void)cmd_add(args, L"--vault-dir");
  (void)cmd_add(args, vault_path);
}

static int hex_digit(char ch) {
  if (ch >= '0' && ch <= '9') {
    return ch - '0';
  }
  if (ch >= 'a' && ch <= 'f') {
    return ch - 'a' + 10;
  }
  if (ch >= 'A' && ch <= 'F') {
    return ch - 'A' + 10;
  }
  return -1;
}

static BOOL append_codepoint(StrBuf *text, unsigned int codepoint) {
  if (codepoint <= 0xFFFF) {
    return sb_append_char(text, (WCHAR)codepoint);
  }

  codepoint -= 0x10000;
  return sb_append_char(text, (WCHAR)(0xD800 + (codepoint >> 10))) &&
         sb_append_char(text, (WCHAR)(0xDC00 + (codepoint & 0x3FF)));
}

static BOOL decode_utf8(const char **cursor, unsigned int *codepoint) {
  const unsigned char *p = (const unsigned char *)*cursor;
  unsigned int value;
  int remaining;

  if (p[0] < 0x80) {
    *codepoint = p[0];
    *cursor += 1;
    return TRUE;
  }
  if ((p[0] & 0xE0) == 0xC0) {
    value = p[0] & 0x1F;
    remaining = 1;
  } else if ((p[0] & 0xF0) == 0xE0) {
    value = p[0] & 0x0F;
    remaining = 2;
  } else if ((p[0] & 0xF8) == 0xF0) {
    value = p[0] & 0x07;
    remaining = 3;
  } else {
    return FALSE;
  }

  {
    int i;
    for (i = 1; i <= remaining; ++i) {
      if (p[i] == '\0' || (p[i] & 0xC0) != 0x80) {
        return FALSE;
      }
      value = (value << 6) | (p[i] & 0x3F);
    }
  }

  *codepoint = value;
  *cursor += remaining + 1;
  return TRUE;
}

/* Read the JSON string starting at the opening quote as UTF-16 text. */
static BOOL parse_json_string(const char **cursor, StrBuf *text) {
  const char *p = *cursor;

  if (*p != '"') {
    return FALSE;
  }
  ++p;

  while (*p && *p != '"') {
    unsigned char ch = (unsigned char)*p;

    if (ch != '\\') {
      unsigned int codepoint;
      if (!decode_utf8(&p, &codepoint) || !append_codepoint(text, codepoint)) {
        return FALSE;
      }
      continue;
    }

    ++p;
    switch (*p++) {
    case '"':
      ch = '"';
      break;
    case '\\':
      ch = '\\';
      break;
    case '/':
      ch = '/';
      break;
    case 'b':
      ch = '\b';
      break;
    case 'f':
      ch = '\f';
      break;
    case 'n':
      ch = '\n';
      break;
    case 'r':
      ch = '\r';
      break;
    case 't':
      ch = '\t';
      break;
    case 'u': {
      unsigned int codepoint = 0;
      int digit;
      int i;
      for (i = 0; i < 4; ++i) {
        digit = hex_digit(*p);
        if (digit < 0) {
          return FALSE;
        }
        codepoint = (codepoint << 4) | (unsigned int)digit;
        ++p;
      }

      /* A surrogate pair arrives as two \u escapes; combine it when the next
         one is present, otherwise write the replacement character. */
      if (codepoint >= 0xD800 && codepoint <= 0xDBFF && p[0] == '\\' &&
          p[1] == 'u') {
        unsigned int low = 0;
        const char *save = p + 2;
        BOOL valid = TRUE;
        for (i = 0; i < 4; ++i) {
          digit = hex_digit(save[i]);
          if (digit < 0) {
            valid = FALSE;
            break;
          }
          low = (low << 4) | (unsigned int)digit;
        }
        if (valid && low >= 0xDC00 && low <= 0xDFFF) {
          codepoint = 0x10000 + ((codepoint - 0xD800) << 10) + (low - 0xDC00);
          p = save + 4;
        } else {
          codepoint = 0xFFFD;
        }
      } else if (codepoint >= 0xD800 && codepoint <= 0xDFFF) {
        codepoint = 0xFFFD;
      }

      if (!append_codepoint(text, codepoint)) {
        return FALSE;
      }
      continue;
    }
    default:
      return FALSE;
    }

    if (!sb_append_char(text, (WCHAR)ch)) {
      return FALSE;
    }
  }

  if (*p != '"') {
    return FALSE;
  }
  ++p;
  *cursor = p;
  return TRUE;
}

static BOOL json_string_array(const char *json, const char *key,
                              AdminList *list) {
  char pattern[64];
  const char *p;
  StrBuf bytes;

  _snprintf_s(pattern, ARRAY_COUNT(pattern), _TRUNCATE, "\"%s\"", key);
  p = strstr(json, pattern);
  if (!p) {
    return FALSE;
  }
  p += strlen(pattern);

  while (*p && isspace((unsigned char)*p)) {
    ++p;
  }
  if (*p++ != ':') {
    return FALSE;
  }
  while (*p && isspace((unsigned char)*p)) {
    ++p;
  }
  if (*p++ != '[') {
    return FALSE;
  }

  sb_init(&bytes);
  for (;;) {
    WCHAR *name;

    while (*p && (isspace((unsigned char)*p) || *p == ',')) {
      ++p;
    }
    if (*p == ']') {
      break;
    }
    if (*p != '"') {
      sb_free(&bytes);
      return FALSE;
    }

    sb_clear(&bytes);
    if (!parse_json_string(&p, &bytes)) {
      sb_free(&bytes);
      return FALSE;
    }

    name = str_dup(bytes.data ? bytes.data : L"");
    if (!name || !append_admin(list, name)) {
      free(name);
      sb_free(&bytes);
      return FALSE;
    }
  }

  sb_free(&bytes);
  return TRUE;
}

static void message_from_result(WCHAR *message, size_t cap, const WCHAR *prefix,
                                const ProcessResult *result) {
  WCHAR *wide;

  if (!message || cap == 0) {
    return;
  }

  wide = process_output_wide(result);
  if (!wide) {
    str_copy(message, cap, prefix);
    return;
  }

  str_trim(wide);
  if (!wide[0]) {
    _snwprintf_s(message, cap, _TRUNCATE, L"%s (exit code %lu)", prefix,
                 (unsigned long)result->exit_code);
  } else {
    str_copy(message, cap, wide);
  }
  free(wide);
}

BOOL admins_load(const WCHAR *rola_exe, const WCHAR *vault_path,
                 AdminList *list, WCHAR *error, size_t error_cap) {
  CmdLine args;
  ProcessResult result;
  BOOL ok;

  admin_list_free(list);
  if (error && error_cap) {
    error[0] = L'\0';
  }

  cmd_init(&args);
  append_base(&args, vault_path);
  (void)cmd_add(&args, L"--json");
  (void)cmd_add(&args, L"vault");
  (void)cmd_add(&args, L"admin");

  process_result_init(&result);
  if (!run_process(rola_exe, args.text.data, NULL, &result, error, error_cap)) {
    cmd_free(&args);
    return FALSE;
  }
  cmd_free(&args);

  if (result.exit_code != 0) {
    message_from_result(error, error_cap, L"rola vault admin failed", &result);
    process_result_free(&result);
    return FALSE;
  }

  ok = json_string_array(result.output ? result.output : "", "admins", list);
  if (!ok) {
    if (error && error_cap) {
      WCHAR *wide = process_output_wide(&result);
      if (wide) {
        str_trim(wide);
        str_copy(error, error_cap, wide);
      } else {
        str_copy(error, error_cap,
                 L"The administrator list could not be read.");
      }
      free(wide);
    }
    admin_list_free(list);
  }

  process_result_free(&result);
  return ok;
}

static BOOL run_admin_command(const WCHAR *rola_exe, const WCHAR *vault_path,
                              const WCHAR *verb, const WCHAR *name,
                              WCHAR *message, size_t message_cap) {
  CmdLine args;
  ProcessResult result;
  BOOL ok;

  cmd_init(&args);
  append_base(&args, vault_path);
  (void)cmd_add(&args, L"vault");
  (void)cmd_add(&args, L"admin");
  (void)cmd_add(&args, verb);
  (void)cmd_add(&args, name);

  process_result_init(&result);
  if (!run_process(rola_exe, args.text.data, NULL, &result, message,
                   message_cap)) {
    cmd_free(&args);
    return FALSE;
  }
  cmd_free(&args);

  ok = result.exit_code == 0;
  message_from_result(
      message, message_cap,
      ok ? L"Administrator changed." : L"rola vault admin failed", &result);
  process_result_free(&result);
  return ok;
}

BOOL admin_add(const WCHAR *rola_exe, const WCHAR *vault_path,
               const WCHAR *name, WCHAR *message, size_t message_cap) {
  return run_admin_command(rola_exe, vault_path, L"add", name, message,
                           message_cap);
}

BOOL admin_remove(const WCHAR *rola_exe, const WCHAR *vault_path,
                  const WCHAR *name, WCHAR *message, size_t message_cap) {
  return run_admin_command(rola_exe, vault_path, L"rm", name, message,
                           message_cap);
}
