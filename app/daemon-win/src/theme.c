#include "theme.h"

#include <commctrl.h>
#include <uxtheme.h>

#include <string.h>
#include <wchar.h>

#include "util.h"

/* The Windows 10 1809 build in which the dark application mode and the two
 * exports that turn it on first exist. Older builds, and any Windows whose
 * version cannot be read, are drawn light. */
#define THEME_MIN_BUILD 17763u

/* The undocumented uxtheme exports. They are read from the module rather than
 * linked to, so a Windows without them can be asked and simply says no. */
#define THEME_ALLOW_DARK_MODE_FOR_WINDOW 133
#define THEME_SET_PREFERRED_APP_MODE 135

#define THEME_COLOR_BACKGROUND RGB(32, 32, 32)
#define THEME_COLOR_CONTROL RGB(45, 45, 45)
#define THEME_COLOR_SELECTED RGB(55, 55, 55)
#define THEME_COLOR_EDIT RGB(28, 28, 28)
#define THEME_COLOR_TEXT RGB(242, 242, 242)
#define THEME_COLOR_MUTED RGB(160, 160, 160)
#define THEME_COLOR_ACCENT RGB(0, 120, 212)

/* The Windows 11 border attribute and the value that gives the border back to
 * the desktop. Both are absent on Windows 10, where the call fails and the
 * desktop's own border stays. */
#define THEME_BORDER_COLOR 34
#define THEME_COLOR_DEFAULT 0xFFFFFFFFu

/* What the two uxtheme exports are, when the module has them. */
typedef BOOL(WINAPI *AllowDarkModeForWindowFn)(HWND, BOOL);
typedef int(WINAPI *SetPreferredAppModeFn)(int);

/* What dwmapi's DwmSetWindowAttribute is, when the module has it. */
typedef HRESULT(WINAPI *DwmSetWindowAttributeFn)(HWND, DWORD, LPCVOID, DWORD);

typedef struct ThemeState {
  BOOL dark;
  BOOL supported;
  BOOL applying;
  BOOL applied;
  HBRUSH background;
  HBRUSH control;
  HBRUSH selected;
  HBRUSH edit;
  HMODULE uxtheme;
  HMODULE dwmapi;
  AllowDarkModeForWindowFn allow_dark;
  SetPreferredAppModeFn preferred_mode;
  DwmSetWindowAttributeFn set_window_attribute;
} ThemeState;

static ThemeState g_theme;

/* Reads the address of an export into a function pointer without the
 * implementation-defined conversion a cast would need. */
static BOOL take_proc(HMODULE module, LPCSTR name, void **target) {
  FARPROC raw;

  if (!module || !name || !target) {
    return FALSE;
  }

  raw = GetProcAddress(module, name);
  if (!raw) {
    return FALSE;
  }

  memcpy(target, &raw, sizeof(raw));
  return TRUE;
}

/* Whether the system is at least the first one that has the dark application
 * mode. The version is read through RtlGetVersion rather than GetVersionEx
 * because a manifest can make the latter lie. */
static BOOL theme_os_supports_dark(void) {
  typedef LONG(WINAPI * RtlGetVersionFn)(OSVERSIONINFOW *);
  HMODULE ntdll;
  RtlGetVersionFn get_version = NULL;
  OSVERSIONINFOW version;

  ntdll = GetModuleHandleW(L"ntdll.dll");
  if (!ntdll || !take_proc(ntdll, "RtlGetVersion", (void **)&get_version) ||
      !get_version) {
    return FALSE;
  }

  ZeroMemory(&version, sizeof(version));
  version.dwOSVersionInfoSize = sizeof(version);
  if (get_version(&version) != 0) {
    return FALSE;
  }

  return version.dwMajorVersion >= 10u &&
         version.dwBuildNumber >= THEME_MIN_BUILD;
}

/* Whether the desktop's application setting is dark. A missing key, a missing
 * value, or a value that is not a DWORD all read as light: the program does not
 * guess at a setting the user has not stated. */
static BOOL theme_desktop_is_dark(void) {
  DWORD value = 1;
  DWORD size = (DWORD)sizeof(value);
  LONG read;

  read = RegGetValueW(
      HKEY_CURRENT_USER,
      L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
      L"AppsUseLightTheme", RRF_RT_REG_DWORD, NULL, &value, &size);

  return read == ERROR_SUCCESS && size == (DWORD)sizeof(value) && value == 0;
}

/* High contrast is the reader's own statement about colour, and the dark palette
 * is not that statement. The system colours are left alone instead. */
static BOOL theme_high_contrast(void) {
  HIGHCONTRASTW contrast;

  ZeroMemory(&contrast, sizeof(contrast));
  contrast.cbSize = (UINT)sizeof(contrast);

  if (!SystemParametersInfoW(SPI_GETHIGHCONTRAST, contrast.cbSize, &contrast,
                             0)) {
    return FALSE;
  }

  return (contrast.dwFlags & HCF_HIGHCONTRASTON) != 0;
}

static void theme_free_brushes(void) {
  if (g_theme.background) {
    DeleteObject(g_theme.background);
    g_theme.background = NULL;
  }
  if (g_theme.control) {
    DeleteObject(g_theme.control);
    g_theme.control = NULL;
  }
  if (g_theme.selected) {
    DeleteObject(g_theme.selected);
    g_theme.selected = NULL;
  }
  if (g_theme.edit) {
    DeleteObject(g_theme.edit);
    g_theme.edit = NULL;
  }
}

static BOOL theme_make_brushes(void) {
  theme_free_brushes();

  g_theme.background = CreateSolidBrush(THEME_COLOR_BACKGROUND);
  g_theme.control = CreateSolidBrush(THEME_COLOR_CONTROL);
  g_theme.selected = CreateSolidBrush(THEME_COLOR_SELECTED);
  g_theme.edit = CreateSolidBrush(THEME_COLOR_EDIT);

  if (g_theme.background && g_theme.control && g_theme.selected &&
      g_theme.edit) {
    return TRUE;
  }

  theme_free_brushes();
  return FALSE;
}

/* The controls the two exports have to be told about, and the theme each one
 * wears. A control whose class has no answer is left to the dialog's colours. */
static const WCHAR *theme_for_class(const WCHAR *class_name) {
  if (wcscmp(class_name, L"Button") == 0) {
    return L"DarkMode_Explorer";
  }

  if (wcscmp(class_name, L"Edit") == 0 ||
      wcscmp(class_name, L"ComboBox") == 0 ||
      wcscmp(class_name, L"ComboBoxEx32") == 0) {
    return L"DarkMode_CFD";
  }

  if (wcscmp(class_name, L"ListBox") == 0 ||
      wcscmp(class_name, L"SysListView32") == 0 ||
      wcscmp(class_name, L"SysTreeView32") == 0 ||
      wcscmp(class_name, L"ScrollBar") == 0 ||
      wcscmp(class_name, L"msctls_statusbar32") == 0) {
    return L"DarkMode_Explorer";
  }

  return NULL;
}

/* Draws the tab strip after the control has drawn itself. The strip is painted
 * here rather than by the control because the control's own theme keeps painting
 * the unselected tabs light in a dark window, whatever sub-theme it is given. */
static void theme_paint_tabs(HWND tab) {
  HDC dc;
  RECT client;
  int count;
  int selected;
  int index;
  HGDIOBJ old_font;
  HFONT font;

  dc = GetDC(tab);
  if (!dc) {
    return;
  }

  if (GetClientRect(tab, &client)) {
    (void)FillRect(dc, &client, g_theme.background);
  }

  count = TabCtrl_GetItemCount(tab);
  selected = TabCtrl_GetCurSel(tab);
  font = (HFONT)SendMessageW(tab, WM_GETFONT, 0, 0);
  old_font = SelectObject(dc, font ? font : GetStockObject(DEFAULT_GUI_FONT));
  SetBkMode(dc, TRANSPARENT);
  SetTextColor(dc, THEME_COLOR_TEXT);

  for (index = 0; index < count; ++index) {
    WCHAR text[128];
    TCITEMW item;
    RECT area;

    if (!TabCtrl_GetItemRect(tab, index, &area)) {
      continue;
    }

    ZeroMemory(&item, sizeof(item));
    ZeroMemory(text, sizeof(text));
    item.mask = TCIF_TEXT;
    item.pszText = text;
    item.cchTextMax = (int)ARRAY_COUNT(text);

    if (!TabCtrl_GetItem(tab, index, &item)) {
      continue;
    }

    (void)FillRect(dc, &area,
                   index == selected ? g_theme.selected : g_theme.background);

    if (index == selected) {
      RECT line = area;
      HBRUSH accent = CreateSolidBrush(THEME_COLOR_ACCENT);

      line.bottom = line.top + 2;
      if (accent) {
        (void)FillRect(dc, &line, accent);
        DeleteObject(accent);
      }
    }

    InflateRect(&area, -4, 0);
    (void)DrawTextW(dc, text, -1, &area,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
  }

  if (old_font) {
    (void)SelectObject(dc, old_font);
  }
  ReleaseDC(tab, dc);
}

static LRESULT CALLBACK theme_tab_proc(HWND tab, UINT message, WPARAM wparam,
                                       LPARAM lparam, UINT_PTR id,
                                       DWORD_PTR reference) {
  LRESULT result;

  (void)id;
  (void)reference;

  result = DefSubclassProc(tab, message, wparam, lparam);

  if (g_theme.dark && message == WM_PAINT) {
    theme_paint_tabs(tab);
  }

  return result;
}

static BOOL CALLBACK theme_apply_child(HWND child, LPARAM dark) {
  WCHAR class_name[64];
  const WCHAR *name;
  int length;

  if (!child) {
    return TRUE;
  }

  if (g_theme.allow_dark) {
    (void)g_theme.allow_dark(child, dark ? TRUE : FALSE);
  }

  length = GetClassNameW(child, class_name, (int)ARRAY_COUNT(class_name));
  if (length <= 0) {
    return TRUE;
  }

  if (wcscmp(class_name, L"SysTabControl32") == 0) {
    DWORD_PTR reference = 0;

    if (!GetWindowSubclass(child, theme_tab_proc, 1, &reference)) {
      (void)SetWindowSubclass(child, theme_tab_proc, 1, 0);
    }
  }

  name = dark ? theme_for_class(class_name) : NULL;
  if (name) {
    (void)SetWindowTheme(child, name, NULL);
  } else {
    (void)SetWindowTheme(child, NULL, NULL);
  }

  return TRUE;
}

/* The frame is a DWM attribute rather than a control theme. The attribute number
 * moved from 19 to 20 in Windows 10 1903, so both are tried and a failure of
 * both leaves the frame as the desktop draws it. */
static void theme_apply_frame(HWND window) {
  BOOL dark;
  DWORD attributes[2];
  COLORREF border;
  size_t at;

  if (!g_theme.set_window_attribute || !window) {
    return;
  }

  if ((GetWindowLongPtrW(window, GWL_STYLE) & WS_CHILD) != 0) {
    return;
  }

  dark = g_theme.dark ? TRUE : FALSE;
  attributes[0] = 20;
  attributes[1] = 19;

  for (at = 0; at < ARRAY_COUNT(attributes); ++at) {
    if (SUCCEEDED(g_theme.set_window_attribute(
            window, attributes[at], &dark, (DWORD)sizeof(dark)))) {
      break;
    }
  }

  border = g_theme.dark ? THEME_COLOR_BACKGROUND : THEME_COLOR_DEFAULT;
  (void)g_theme.set_window_attribute(window, THEME_BORDER_COLOR, &border,
                                     (DWORD)sizeof(border));
}

/* What a child's background is, which is the editor's for an editor and the
 * dialog's for everything else. */
static HBRUSH theme_child_background(HWND child) {
  WCHAR class_name[64];

  if (!g_theme.dark) {
    return NULL;
  }

  if (child &&
      GetClassNameW(child, class_name, (int)ARRAY_COUNT(class_name)) > 0 &&
      (wcscmp(class_name, L"Edit") == 0 ||
       wcscmp(class_name, L"ComboBox") == 0 ||
       wcscmp(class_name, L"ComboBoxEx32") == 0)) {
    return g_theme.edit;
  }

  return g_theme.background;
}

/* Applies the colours in force to a control that has just been made. */
void theme_apply(HWND window) {
  if (!window) {
    return;
  }

  if (!g_theme.dark && !g_theme.applied) {
    return;
  }

  (void)EnumChildWindows(window, theme_apply_child, (LPARAM)g_theme.dark);
  theme_apply_frame(window);
  RedrawWindow(window, NULL, NULL,
               RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN);
}

BOOL theme_is_dark(void) { return g_theme.dark; }

/* Turns the dark look on or off, and says whether the brushes it needs could be
 * made: a dark mode with no brushes is the light one rather than an unreadable
 * one. */
static void theme_turn(BOOL dark) {
  BOOL can = dark;

  if (can) {
    can = theme_make_brushes();
  } else {
    theme_free_brushes();
  }

  g_theme.dark = can;

  if (can) {
    g_theme.applied = TRUE;
  }

  if (g_theme.preferred_mode) {
    (void)g_theme.preferred_mode(can ? 1 : 0);
  }
}

static BOOL CALLBACK theme_refresh_window(HWND window, LPARAM unused);

/* Reads the desktop again and applies whatever it now says. The guard keeps a
 * WM_THEMECHANGED sent by SetWindowTheme from starting a second pass inside the
 * first. */
static void theme_refresh(void) {
  BOOL dark;

  if (g_theme.applying) {
    return;
  }

  g_theme.applying = TRUE;
  dark = g_theme.supported && !theme_high_contrast() && theme_desktop_is_dark();
  theme_turn(dark);
  EnumThreadWindows(GetCurrentThreadId(), theme_refresh_window, 0);
  if (!g_theme.dark) {
    g_theme.applied = FALSE;
  }
  g_theme.applying = FALSE;}

void theme_init(void) {
  if (g_theme.uxtheme) {
    return;
  }

  g_theme.uxtheme = LoadLibraryW(L"uxtheme.dll");
  g_theme.dwmapi = LoadLibraryW(L"dwmapi.dll");

  if (g_theme.uxtheme) {
    (void)take_proc(g_theme.uxtheme,
                    MAKEINTRESOURCEA(THEME_ALLOW_DARK_MODE_FOR_WINDOW),
                    (void **)&g_theme.allow_dark);
    (void)take_proc(g_theme.uxtheme,
                    MAKEINTRESOURCEA(THEME_SET_PREFERRED_APP_MODE),
                    (void **)&g_theme.preferred_mode);
  }

  if (g_theme.dwmapi) {
    (void)take_proc(g_theme.dwmapi, "DwmSetWindowAttribute",
                    (void **)&g_theme.set_window_attribute);
  }

  g_theme.supported = theme_os_supports_dark() && g_theme.allow_dark &&
                      g_theme.preferred_mode;
  theme_turn(g_theme.supported && !theme_high_contrast() &&
             theme_desktop_is_dark());
}

BOOL theme_message(HWND dialog, UINT message, WPARAM wparam, LPARAM lparam,
                   INT_PTR *answer) {
  HDC dc;
  HWND child;
  HBRUSH brush;
  COLORREF background;
  COLORREF text;

  if (message == WM_INITDIALOG) {
    theme_apply(dialog);
    return FALSE;
  }

  if (message == WM_SETTINGCHANGE) {
    if (!lparam ||
        wcscmp((const WCHAR *)lparam, L"ImmersiveColorSet") == 0 ||
        wcscmp((const WCHAR *)lparam, L"HighContrast") == 0) {
      theme_refresh();
    }
    return FALSE;
  }

  if (message == WM_THEMECHANGED || message == WM_SYSCOLORCHANGE) {
    theme_refresh();
    return FALSE;
  }

  if (!g_theme.dark) {
    return FALSE;
  }

  switch (message) {
  case WM_ERASEBKGND: {
    RECT area;

    dc = (HDC)wparam;
    if (!dc || !GetClientRect(dialog, &area)) {
      return FALSE;
    }
    FillRect(dc, &area, g_theme.background);
    if (answer) {
      *answer = TRUE;
    }
    return TRUE;
  }

  case WM_CTLCOLORDLG:
    dc = (HDC)wparam;
    if (!dc) {
      return FALSE;
    }
    SetBkColor(dc, THEME_COLOR_BACKGROUND);
    if (answer) {
      *answer = (INT_PTR)g_theme.background;
    }
    return TRUE;

  case WM_CTLCOLORSTATIC:
  case WM_CTLCOLORBTN:
  case WM_CTLCOLOREDIT:
  case WM_CTLCOLORLISTBOX:
  case WM_CTLCOLORSCROLLBAR:
    dc = (HDC)wparam;
    child = (HWND)lparam;
    if (!dc) {
      return FALSE;
    }

    brush = message == WM_CTLCOLOREDIT ? g_theme.edit
            : message == WM_CTLCOLORLISTBOX ? g_theme.control
                                            : theme_child_background(child);
    if (!brush) {
      brush = g_theme.background;
    }

    background = message == WM_CTLCOLOREDIT ? THEME_COLOR_EDIT
                 : message == WM_CTLCOLORLISTBOX ? THEME_COLOR_CONTROL
                 : theme_child_background(child) == g_theme.edit
                     ? THEME_COLOR_EDIT
                     : THEME_COLOR_BACKGROUND;

    SetBkColor(dc, background);
    text = (child && !IsWindowEnabled(child)) ? THEME_COLOR_MUTED
                                               : THEME_COLOR_TEXT;
    SetTextColor(dc, text);

    if (answer) {
      *answer = (INT_PTR)brush;
    }
    return TRUE;

  default:
    return FALSE;
  }
}

static BOOL CALLBACK theme_refresh_window(HWND window, LPARAM unused) {
  (void)unused;

  if (!window) {
    return TRUE;
  }

  theme_apply(window);
  return TRUE;
}

