using System.Runtime.InteropServices;

// Every P/Invoke in this assembly names a system library — shell32, user32 or gdi32 — and none of them
// is to be looked for anywhere but System32. The platform's default is to search the program's own
// directory first, which is how a library dropped beside the program is loaded in place of the
// system's; this assembly has no such library of its own and asks for the narrow path instead.
[assembly: DefaultDllImportSearchPaths(DllImportSearchPath.System32)]
