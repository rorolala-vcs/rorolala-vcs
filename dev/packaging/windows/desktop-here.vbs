' Opens the Desktop program at a directory, through the command line, with no console window.

' What the context menu runs. `rola desktop` is a console program, and a menu item that started one would flash a
' window over whatever the reader was doing — so it is started by a script host instead of by a shell, and the
' command line is asked for a window that is not shown.

' Hiding the window hides the console and nothing else: the Desktop program it starts is a window of its own, and
' that one appears as it always does. What is hidden outlives the desktop window, and that is deliberate — `rola
' desktop` waits for the window it opened, so it is the process that is alive for as long as the reader is looking
' at one.

Option Explicit

Dim shell, here, rola, at

Set shell = CreateObject("WScript.Shell")

' Where this script is, which is where the command line it runs is: the two are laid down beside each other, so
' the layout is found rather than written out here — one install's path is not another's.
here = Left(WScript.ScriptFullName, InStrRev(WScript.ScriptFullName, "\"))
rola = here & "rola.exe"

' What the menu handed over, or where the menu was opened when it handed over nothing: what the window is to open
' onto is the directory the reader pointed at, not the one this script happens to sit in.
at = shell.CurrentDirectory
If WScript.Arguments.Count > 0 Then
	If WScript.Arguments(0) <> "" Then at = WScript.Arguments(0)
End If

shell.CurrentDirectory = at

' 0 is a window that is not shown, and False is not waiting: what is opened is a window, and nothing here has any
' business outliving the process that asked for one.
shell.Run """" & rola & """ desktop", 0, False
