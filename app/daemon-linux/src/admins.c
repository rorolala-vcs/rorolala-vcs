#include "admins.h"

#include <string.h>

#include "process.h"
#include "util.h"

void admin_list_init(AdminList *list) {
  list->items = NULL;
  list->count = 0;
}

void admin_list_free(AdminList *list) {
  size_t i;

  for (i = 0; i < list->count; ++i) {
    g_free(list->items[i]);
  }
  g_free(list->items);
  admin_list_init(list);
}

static gboolean append_admin(AdminList *list, char *name) {
  char **items = g_try_renew(char *, list->items, list->count + 1);

  if (!items) {
    return FALSE;
  }
  list->items = items;
  list->items[list->count++] = name;
  return TRUE;
}

static void append_base(CmdLine *args, const char *vault_path) {
  (void)cmd_add(args, "--no-pager");
  (void)cmd_add(args, "--no-color");
  (void)cmd_add(args, "--no-progress");
  (void)cmd_add(args, "--lang");
  (void)cmd_add(args, "en");
  (void)cmd_add(args, "--vault-dir");
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

static gboolean append_codepoint(GString *text, guint32 codepoint) {
  char buffer[6];
  int length;

  if (codepoint > 0x10FFFF || (codepoint >= 0xD800 && codepoint <= 0xDFFF)) {
    return FALSE;
  }

  length = g_unichar_to_utf8((gunichar)codepoint, buffer);
  if (length <= 0) {
    return FALSE;
  }
  g_string_append_len(text, buffer, length);
  return TRUE;
}

/* Read the JSON string starting at the opening quote. */
static gboolean parse_json_string(const char **cursor, GString *text) {
  const char *p = *cursor;

  if (*p != '"') {
    return FALSE;
  }
  ++p;

  while (*p && *p != '"') {
    char ch = *p;

    if (ch != '\\') {
      g_string_append_c(text, ch);
      ++p;
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
      guint32 codepoint = 0;
      int digit;
      int i;

      for (i = 0; i < 4; ++i) {
        digit = hex_digit(*p);
        if (digit < 0) {
          return FALSE;
        }
        codepoint = (codepoint << 4) | (guint32)digit;
        ++p;
      }

      /* A surrogate pair arrives as two \u escapes; combine it when the next
         one is present, otherwise write the replacement character. */
      if (codepoint >= 0xD800 && codepoint <= 0xDBFF && p[0] == '\\' &&
          p[1] == 'u') {
        guint32 low = 0;
        const char *save = p + 2;
        gboolean valid = TRUE;

        for (i = 0; i < 4; ++i) {
          digit = hex_digit(save[i]);
          if (digit < 0) {
            valid = FALSE;
            break;
          }
          low = (low << 4) | (guint32)digit;
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

    g_string_append_c(text, ch);
  }

  if (*p != '"') {
    return FALSE;
  }
  ++p;
  *cursor = p;
  return TRUE;
}

static gboolean json_string_array(const char *json, const char *key,
                                  AdminList *list) {
  char *pattern = g_strdup_printf("\"%s\"", key);
  size_t pattern_length = strlen(pattern);
  const char *p = strstr(json, pattern);

  if (!p) {
    g_free(pattern);
    return FALSE;
  }
  p += pattern_length;
  g_free(pattern);

  while (*p && g_ascii_isspace(*p)) {
    ++p;
  }
  if (*p++ != ':') {
    return FALSE;
  }
  while (*p && g_ascii_isspace(*p)) {
    ++p;
  }
  if (*p++ != '[') {
    return FALSE;
  }

  for (;;) {
    GString *text;
    char *name;

    while (*p && (g_ascii_isspace(*p) || *p == ',')) {
      ++p;
    }
    if (*p == ']') {
      break;
    }
    if (*p != '"') {
      return FALSE;
    }

    text = g_string_new(NULL);
    if (!parse_json_string(&p, text)) {
      g_string_free(text, TRUE);
      return FALSE;
    }

    name = g_string_free(text, FALSE);
    if (!append_admin(list, name)) {
      g_free(name);
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

  text = g_strndup(result->output, result->output_len);
  trimmed = str_trim(text);
  if (!trimmed[0]) {
    g_snprintf(message, cap, "%s (exit code %d)", prefix, result->exit_code);
  } else {
    str_copy(message, cap, trimmed);
  }
  g_free(text);
}

gboolean admins_load(const char *rola_exe, const char *vault_path,
                     AdminList *list, char *error, size_t error_cap) {
  CmdLine args;
  ProcessResult result;
  gboolean ok;

  admin_list_free(list);
  if (error && error_cap) {
    error[0] = '\0';
  }

  cmd_init(&args);
  append_base(&args, vault_path);
  (void)cmd_add(&args, "--json");
  (void)cmd_add(&args, "vault");
  (void)cmd_add(&args, "admin");

  process_result_init(&result);
  if (!run_process(rola_exe, &args, NULL, &result, error, error_cap)) {
    cmd_free(&args);
    return FALSE;
  }
  cmd_free(&args);

  if (result.exit_code != 0) {
    message_from_result(error, error_cap, "rola vault admin failed", &result);
    process_result_free(&result);
    return FALSE;
  }

  ok = json_string_array(result.output, "admins", list);
  if (!ok) {
    if (error && error_cap) {
      char *wide = g_strndup(result.output, result.output_len);
      char *trimmed = str_trim(wide);

      if (trimmed[0]) {
        str_copy(error, error_cap, trimmed);
      } else {
        str_copy(error, error_cap, "The administrator list could not be read.");
      }
      g_free(wide);
    }
    admin_list_free(list);
  }

  process_result_free(&result);
  return ok;
}

static gboolean run_admin_command(const char *rola_exe, const char *vault_path,
                                  const char *verb, const char *name,
                                  char *message, size_t message_cap) {
  CmdLine args;
  ProcessResult result;
  gboolean ok;

  cmd_init(&args);
  append_base(&args, vault_path);
  (void)cmd_add(&args, "vault");
  (void)cmd_add(&args, "admin");
  (void)cmd_add(&args, verb);
  (void)cmd_add(&args, name);

  process_result_init(&result);
  if (!run_process(rola_exe, &args, NULL, &result, message, message_cap)) {
    cmd_free(&args);
    return FALSE;
  }
  cmd_free(&args);

  ok = result.exit_code == 0;
  message_from_result(message, message_cap,
                      ok ? "Administrator changed." : "rola vault admin failed",
                      &result);
  process_result_free(&result);
  return ok;
}

gboolean admin_add(const char *rola_exe, const char *vault_path,
                   const char *name, char *message, size_t message_cap) {
  return run_admin_command(rola_exe, vault_path, "add", name, message,
                           message_cap);
}

gboolean admin_remove(const char *rola_exe, const char *vault_path,
                      const char *name, char *message, size_t message_cap) {
  return run_admin_command(rola_exe, vault_path, "rm", name, message,
                           message_cap);
}
