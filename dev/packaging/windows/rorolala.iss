; Rorolala, as Windows is given it: one program that lays the export out where a shell and a file manager
; find it, and that can be asked to take all of it back.
;
; What it installs is the layout `cargo dev export` lays down, taken as it stands: the command line programs
; and the two libraries beside them, the Desktop program with its plugins, and the completion scripts, with
; README and LICENSE going in with all of it. It is offered as two presets over the parts — `compact`, the
; command line and its interface, and `full`, everything — with every part still on the page for a reader who
; wants one left out. What it *does*, beyond copying, is the three things that make a program reachable
; rather than merely present — a block in the user's `$PROFILE`, an entry on the path, and a context menu
; item — and each of them is written conservatively, so that an uninstall removes what this installer wrote
; and nothing else.
;
; It is compiled with the pinned Inno Setup that `cargo dev publish` lays out; the four things that differ
; from one build to the next are given to it on the command line rather than written here.

#ifndef Version
  #define Version "0.0.0"
#endif

; The layout that was exported, and where the installer is written. Both are handed in as paths a Windows
; program reads, which is why nothing here works them out from where this script happens to sit.
#ifndef SourceDir
  #define SourceDir "..\..\..\build"
#endif

#ifndef OutputDir
  #define OutputDir "..\..\..\build"
#endif

#ifndef Setup
  #define Setup "rorolala-setup"
#endif

#ifndef Publisher
  #define Publisher "RorolalaVCS"
#endif

; The version of the Desktop runtime the published Desktop program needs, and where a reader is sent when the
; machine does not have it. It is a framework-dependent build: what it needs is the runtime rather than the
; SDK, and the address is Microsoft's own page for it rather than a file this project would have to carry.
#define DotnetMajor "8"
#define DotnetUrl "https://dotnet.microsoft.com/download/dotnet/8.0"

; The two things this installer writes outside its own directory, named here so that the code that writes
; them, the code that removes them and this comment cannot drift apart.
#define ProfileBegin "# Rola begin"
#define ProfileEnd "# Rola end"

[Setup]
AppId={{8E1E9E31-6C2E-4C9B-9E7B-2C7A2E4B7C31}
; What is written into the environment is a change to it, and this is how the installer is told so: it is what
; makes the change broadcast to what is already running, and what keeps the uninstaller's account of the
; environment and this installer's the same one.
ChangesEnvironment=yes
AppName=Rorolala
AppVersion={#Version}
AppVerName=Rorolala {#Version}
; Who makes this, which is the first of the workspace's authors rather than a name written here: the manifest
; is where an author is named, and a second copy of the name is one that can go stale.
AppPublisher={#Publisher}
VersionInfoVersion={#Version}
; The licence the project is under, shown on the page that asks a reader to accept it: the file is the one the
; export laid down beside the programs, so what is shown is the text that ships with them and not a copy.
LicenseFile={#SourceDir}\LICENSE
DefaultDirName={autopf}\Rorolala
DefaultGroupName=Rorolala
DisableProgramGroupPage=yes
DisableDirPage=no
OutputDir={#OutputDir}
OutputBaseFilename={#Setup}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; The project's own title image, drawn by the wizard itself rather than by this script: Inno reads a PNG in its
; own image slots and scales it, where the bitmap a `[Code]` page can show is a Windows bitmap and nothing
; else. It is the same picture the title page of the README is, so the installer looks like the project.
; Only the small slot is filled: the large, upright image Inno draws down the side of the Welcome and Setup
; Completed pages is a second copy of the same picture, and blanking it is how Inno is told not to draw one —
; leaving the directive out would bring back Inno's own image instead. The page layout is unchanged.
WizardImageFile=
WizardSmallImageFile=..\..\..\res\icons\rorolala-title.png
; What the installer itself is drawn as in a file list and on a shortcut: the installer's own icon rather
; than the program's, and the multi-frame one this project generates for it — Windows asks a program for one
; of five sizes, and a single frame would be resampled for four of them. It is built from
; `res/icons/installer/installer.toml` by the workspace build script, so a build makes it before this is
; compiled (see `res/icons/.gitignore`).
SetupIconFile=..\..\..\res\icons\installer\installer.ico
UninstallDisplayIcon={app}\bin\desktop\RorolalaDesktop.exe
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
; Installed for the person who ran it rather than for the machine, which is what everything this installer
; writes is: a `$PROFILE` is one person's, the path entry is one person's, and the context menu is one
; person's. Asking for administrator rights would be asking for more than the install does.
PrivilegesRequired=lowest

[Languages]
; English is Inno's own and travels with it. Chinese does not: Inno Setup ships twenty-eight translations and
; that is not one of them — it is maintained in Inno's own source repository and has to be fetched — so the
; line is kept only where the file is there. A build whose network cannot reach it is a build with one
; language rather than a build that fails, and one whose network can is the bilingual installer this project
; wants: `publish` fetches it beside the toolchain and says so when it cannot.
Name: "en"; MessagesFile: "compiler:Default.isl"
#if FileExists(AddBackslash(CompilerPath) + "Languages\ChineseSimplified.isl")
Name: "zh"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"
#endif

; What this installer says in its own words, in both the languages this project writes in: what a component
; is, what the context menu item is called, and what a reader is told when the runtime the Desktop program
; needs is not here. The rest of the wizard is Inno's own and is translated by the language file above.
[CustomMessages]
en.TitleSubtitle=A version control system designed for binary assets
en.Rola=The command line (rola)
en.Compact=Compact
en.Full=Full
en.RolaEdit=The editor helper (rola-edit)
en.Lib=The C ABI and its bindings (library, header, RolaSharp)
en.Desktop=The Desktop program
en.Shortcuts=Start menu shortcuts
en.Daemon=The daemon and its window (rola-daemon, rola-daemon-gui)
en.Jj=Jujutsu support (rola-jj)
en.Git=Git support (git-rola)
en.Completions=The PowerShell completion scripts
en.Profile=The block in your PowerShell profile
en.Menu=The "Open with Rorolala Desktop" context menu
en.Path=Add the programs to your path
en.OpenWith=Open with Rorolala Desktop
en.RuntimeTitle=Rorolala
en.RuntimeSaid=The Desktop program needs the .NET Desktop Runtime {#DotnetMajor}, which this machine does not have. It is a free download from Microsoft:%n%n{#DotnetUrl}%n%nThe command line programs work without it.%n%nInstall anyway?
en.RuntimeRefused=The .NET Desktop Runtime {#DotnetMajor} is needed by the Desktop program.
en.Missed=The programs are installed, but this could not be written:

; The Chinese half is kept only where the language itself is: a message for a language the installer does not
; have is an error rather than a spare, and the two are fetched and left out together.
#if FileExists(AddBackslash(CompilerPath) + "Languages\ChineseSimplified.isl")
zh.TitleSubtitle=为二进制资产设计的版本控制系统
zh.Rola=命令行程序（rola）
zh.Compact=精简
zh.Full=完整
zh.RolaEdit=编辑器助手（rola-edit）
zh.Lib=C ABI 与其绑定（库、头文件、RolaSharp）
zh.Desktop=桌面端程序
zh.Shortcuts=开始菜单快捷方式
zh.Daemon=服务端与其窗口（rola-daemon、rola-daemon-gui）
zh.Jj=Jujutsu 支持（rola-jj）
zh.Git=Git 支持（git-rola）
zh.Completions=PowerShell 补全脚本
zh.Profile=写入 PowerShell profile 的段落
zh.Menu=“用 Rorolala Desktop 打开”右键菜单
zh.Path=把程序加入 PATH
zh.OpenWith=用 Rorolala Desktop 打开
zh.RuntimeTitle=Rorolala
zh.RuntimeSaid=桌面端程序需要 .NET Desktop Runtime {#DotnetMajor}，这台机器上没有。它是免费的，可从微软下载：%n%n{#DotnetUrl}%n%n命令行程序不受影响。%n%n仍然继续安装吗？
zh.RuntimeRefused=桌面端程序需要 .NET Desktop Runtime {#DotnetMajor}。
zh.Missed=程序已安装，但下面这些没能写入：
#endif

[Components]
; One component per thing a reader may want or not want, and the two presets those parts add up to: a
; quick or full install is a set of the parts below named in one go, and a reader who picks no preset is
; offered every part one by one. Everything is offered and everything starts selected — what was installed
; last time is not remembered, so the state a page opens in should be the whole of it — and the one
; component that cannot be cleared is the command line, since the Desktop program is opened by it and the
; context menu runs it: a Desktop program with no way to open it is a directory nobody can start.
;
; `compact` is the command line and its interface and nothing that is only ever reached from a shell: the
; Desktop program, the daemon it talks to, the completion scripts, the path entry and the context menu.
; `full` is that and the parts a developer reaches for — the editor helper, the C ABI, Jujutsu, Git and
; the ones the picker keeps off the compact list — which is why it is the one that is selected to begin
; with and the one Inno is named for, so a reader who reads nothing gets all of it.
Name: "compact"; Description: "{cm:Compact}"; Types: compact custom
Name: "full"; Description: "{cm:Full}"; Types: full custom; Flags: fixed

Name: "rola"; Description: "{cm:Rola}"; Types: compact full custom; Flags: fixed
Name: "editor"; Description: "{cm:RolaEdit}"; Types: full custom
Name: "lib"; Description: "{cm:Lib}"; Types: full custom
Name: "desktop"; Description: "{cm:Desktop}"; Types: compact full custom
Name: "shortcuts"; Description: "{cm:Shortcuts}"; Types: full custom
Name: "daemon"; Description: "{cm:Daemon}"; Types: compact full custom
Name: "jj"; Description: "{cm:Jj}"; Types: full custom
Name: "git"; Description: "{cm:Git}"; Types: full custom
Name: "completions"; Description: "{cm:Completions}"; Types: compact full custom
Name: "profile"; Description: "{cm:Profile}"; Types: full custom
Name: "menu"; Description: "{cm:Menu}"; Types: compact full custom
Name: "path"; Description: "{cm:Path}"; Types: compact full custom

[Files]
; What every install has, whatever was chosen above: the command line itself, which is the one program that
; cannot be left out — the Desktop program is opened by it, and the context menu runs it — beside the words
; that say what this is. README and LICENSE carry no component because there is nothing to leave them out
; for: they are two small files, and a layout a reader is handed should have the terms it is handed under.
Source: "{#SourceDir}\bin\rola.exe"; DestDir: "{app}\bin"; Components: rola; Flags: ignoreversion

; The parts of the command line that are not the command line: the helper a run opens edits in and the C ABI
; a consumer links against.
Source: "{#SourceDir}\bin\rola-edit.exe"; DestDir: "{app}\bin"; Components: editor; Flags: ignoreversion
Source: "{#SourceDir}\lib\*"; DestDir: "{app}\lib"; Components: lib; Flags: ignoreversion
Source: "{#SourceDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

; The daemon and Jujutsu are programs of their own rather than parts of the command line: what a reader
; installs is what they use. The daemon's window is laid down beside the daemon it starts, since that is
; where it looks for it — the two are one component because a window without the daemon it starts is a
; program that opens only to say it cannot.
Source: "{#SourceDir}\bin\rola-daemon.exe"; DestDir: "{app}\bin"; Components: daemon; Flags: ignoreversion
Source: "{#SourceDir}\bin\rola-daemon-gui.exe"; DestDir: "{app}\bin"; Components: daemon; Flags: ignoreversion
Source: "{#SourceDir}\bin\rola-jj.exe"; DestDir: "{app}\bin"; Components: jj; Flags: ignoreversion
Source: "{#SourceDir}\bin\git-rola.exe"; DestDir: "{app}\bin"; Components: git; Flags: ignoreversion

; The Desktop program: the whole directory it was published as, plugins and translations included, since
; what it is made of is what it reads beside itself.
Source: "{#SourceDir}\bin\desktop\*"; DestDir: "{app}\bin\desktop"; Components: desktop; Flags: ignoreversion recursesubdirs createallsubdirs

; The completion scripts, whose whole purpose is to be sourced from the profile the code below writes.
Source: "{#SourceDir}\scripts\*"; DestDir: "{app}\scripts"; Components: completions; Flags: ignoreversion recursesubdirs createallsubdirs

; What the context menu runs: a script of this installer's own, taken from beside this script rather than from the
; layout, since it is the installer's and not the export's. It is laid beside the command line, where it finds
; that command by being next to it.
Source: "{#SourcePath}\desktop-here.vbs"; DestDir: "{app}\bin"; Components: menu; Flags: ignoreversion

[Icons]
; The launcher, which is where a reader starts: it lists the Workspaces on this machine and opens one through
; `rola desktop`, which is what finds the Workspace and hands the window the command line it is to work with.
; A shortcut straight to the Desktop program would open a window whose commands could not run, and one straight
; to the command line is the third item. Each item carries the name of the program it starts, so that what a
; reader finds on the menu is what they would name on a command line.
;
; The daemon's window is offered on its own as well: it is started before a Workspace is opened and
; keeps running behind one, so opening it is not opening the Desktop program. The entry asks for both
; the shortcut and the daemon components, since a shortcut to a program whose component was cleared
; would open nothing.
Name: "{group}\ROLALauncher"; Filename: "{app}\bin\desktop\ROLALauncher.exe"; WorkingDir: "{app}"; IconFilename: "{app}\bin\desktop\ROLALauncher.exe"; IconIndex: 0; Components: shortcuts
Name: "{group}\ROLADaemon"; Filename: "{app}\bin\rola-daemon-gui.exe"; WorkingDir: "{app}"; IconFilename: "{app}\bin\rola-daemon-gui.exe"; IconIndex: 0; Components: shortcuts and daemon
Name: "{group}\Rorolala"; Filename: "{app}\bin\rola.exe"; WorkingDir: "{app}"; Comment: "The command line"; Components: shortcuts
Name: "{group}\Uninstall Rorolala"; Filename: "{uninstallexe}"

[Registry]
; The context menu, as two entries: one for a click on a folder's own background, and one for a click on the
; folder itself. Both hand the directory to `rola desktop`, which is what decides whether the directory is
; inside a Workspace — a menu item is drawn by the shell before anything of this project runs, so whether
; there is work here to open is a question only the program can answer, and it answers it by opening where
; there is work and doing nothing where there is not.
;
; What both run is a script host and not a shell, and the directory is handed over rather than being where the
; menu happened to leave the process: a shell would open a console window over whatever the reader was doing, and
; a click on the folder itself starts in the folder's parent. The script it runs is the one beside the command
; line, which starts that command hidden — see `desktop-here.vbs`.
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\RorolalaDesktop"; ValueType: string; ValueName: ""; ValueData: "{cm:OpenWith}"; Components: menu; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\RorolalaDesktop"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\bin\desktop\RorolalaDesktop.exe"; Components: menu
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\RorolalaDesktop\command"; ValueType: string; ValueName: ""; ValueData: "wscript.exe //B //Nologo ""{app}\bin\desktop-here.vbs"" ""%V"""; Components: menu; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Directory\shell\RorolalaDesktop"; ValueType: string; ValueName: ""; ValueData: "{cm:OpenWith}"; Components: menu; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Directory\shell\RorolalaDesktop"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\bin\desktop\RorolalaDesktop.exe"; Components: menu
Root: HKCU; Subkey: "Software\Classes\Directory\shell\RorolalaDesktop\command"; ValueType: string; ValueName: ""; ValueData: "wscript.exe //B //Nologo ""{app}\bin\desktop-here.vbs"" ""%V"""; Components: menu; Flags: uninsdeletekey

[Code]
const
  { The environment value the path is written into, and the separator its entries are read apart by. }
  EnvironmentKey = 'Environment';
  PathValue = 'Path';

var
  { The page the title is drawn on: the first thing a reader sees, and the only page of this installer that
    is not a question. }
  TitlePage: TWizardPage;

{ Whether the Desktop runtime the published program needs is on this machine.

  A framework-dependent build is what the export lays down — the program names the runtime rather than
  carrying it — so a machine without it would install a program that does not start, which is worth saying
  before it is copied rather than after it is run. Any version of the major the program was built against is
  enough, which is why the directory is searched rather than a version named. }
function RuntimeUnder(const Root: String): Boolean;
var
  Found: TFindRec;
begin
  Result := False;

  if not DirExists(Root) then
    Exit;

  if FindFirst(AddBackslash(Root) + '{#DotnetMajor}.*', Found) then
  begin
    try
      repeat
        if (Found.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0 then
        begin
          Result := True;
          Exit;
        end;
      until not FindNext(Found);
    finally
      FindClose(Found);
    end;
  end;
end;

function RuntimeInstalled: Boolean;
begin
  { Both places a runtime is installed to: the machine's, and the one a run as this user installs into. }
  Result :=
    RuntimeUnder(ExpandConstant('{commonpf}\dotnet\shared\Microsoft.WindowsDesktop.App')) or
    RuntimeUnder(ExpandConstant('{localappdata}\Microsoft\dotnet\shared\Microsoft.WindowsDesktop.App'));
end;

{ The whole address a reader is given, said once so that the message and the help text cannot differ. }
function RuntimeHelp: String;
begin
  Result := ExpandConstant('{cm:RuntimeSaid}');
end;

{ Whether the profile this user gets is one we may write, and where it is.

  `$PROFILE` is `Documents\PowerShell\Microsoft.PowerShell_profile.ps1` for PowerShell 7 and
  `Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1` for Windows PowerShell: two files for two
  shells, and the block is written into whichever of them exists — into both when both do, since a reader who
  has both should find the programs on the path in either. }
function Profiles: TArrayOfString;
var
  Said: TArrayOfString;
  At: Integer;
  Named: String;
begin
  SetArrayLength(Said, 0);

  Named := ExpandConstant('{userdocs}\PowerShell\Microsoft.PowerShell_profile.ps1');
  At := GetArrayLength(Said);
  SetArrayLength(Said, At + 1);
  Said[At] := Named;

  Named := ExpandConstant('{userdocs}\WindowsPowerShell\Microsoft.PowerShell_profile.ps1');
  At := GetArrayLength(Said);
  SetArrayLength(Said, At + 1);
  Said[At] := Named;

  Result := Said;
end;

{ Lines of a text file, as an array, and nothing at all for a file that is not there. }
function LinesOf(const Filename: String): TArrayOfString;
begin
  SetArrayLength(Result, 0);

  if FileExists(Filename) then
    LoadStringsFromFile(Filename, Result);
end;

function IndexOfLine(const Lines: TArrayOfString; const Wanted: String): Integer;
var
  At: Integer;
begin
  Result := -1;

  for At := 0 to GetArrayLength(Lines) - 1 do
    if Trim(Lines[At]) = Wanted then
    begin
      Result := At;
      Exit;
    end;
end;

{ What the block says, given where this install put its programs and which parts of it were asked for.

  The lines are the reader's choices and not this installer's: the completions are sourced only where the scripts
  were installed, and the path is set only where that was asked for. A block that named a script that is not
  there, or that put a directory on the path for a reader who said no, would be undoing the choice they just
  made.

  The last two lines are comments, and that is the same kind of choice: a reader who wants an editor or a language
  uncomments the one they want, which is a decision this installer has no business making for them. }
function BlockFor(
  const Bin: String;
  const Scripts: String;
  Completions: Boolean;
  Path: Boolean
): TArrayOfString;
var
  Said: TArrayOfString;
  At: Integer;
begin
  SetArrayLength(Said, 0);
  At := 0;

  SetArrayLength(Said, At + 1);
  Said[At] := '{#ProfileBegin}';
  At := At + 1;

  if Completions then
  begin
    SetArrayLength(Said, At + 1);
    Said[At] := '. "' + Scripts + '\rola\rola-completion.ps1"';
    At := At + 1;

    SetArrayLength(Said, At + 1);
    Said[At] := '. "' + Scripts + '\rola-daemon\rola-daemon-completion.ps1"';
    At := At + 1;
  end;

  if Path then
  begin
    SetArrayLength(Said, At + 1);
    Said[At] := '$env:Path = "' + Bin + ';" + $env:Path';
    At := At + 1;
  end;

  SetArrayLength(Said, At + 1);
  Said[At] := '# Uncomment to choose the editor a run opens its edits in, and the language it speaks:';
  At := At + 1;

  SetArrayLength(Said, At + 1);
  Said[At] := '# $env:ROLA_EDITOR = "rola-edit"; $env:ROLA_LANG = "en"';
  At := At + 1;

  SetArrayLength(Said, At + 1);
  Said[At] := '{#ProfileEnd}';

  Result := Said;
end;

{ Writes the block into one profile, keeping a copy of what was there before it.

  Conservative in the one way that matters: a block this installer did not write is a block it does not
  touch. It looks for its own two markers, replaces what stands between them if they are both there, and
  appends the block if they are not — which is also what makes running the installer twice one block rather
  than two. }
procedure WriteBlock(const Profile: String; const Block: TArrayOfString);
var
  Lines: TArrayOfString;
  Kept: TArrayOfString;
  Backup: String;
  At, Written, Begins, Ends: Integer;
begin
  Lines := LinesOf(Profile);

  if GetArrayLength(Lines) > 0 then
  begin
    Backup := Profile + '.rorolala-backup';

    { Once rather than on every run: what a reader wants back is the profile as it was before this project
      first touched it, and a backup taken over a backup with the block already in it would be no backup. }
    if not FileExists(Backup) then
      SaveStringsToFile(Backup, Lines, False);
  end;

  Begins := IndexOfLine(Lines, '{#ProfileBegin}');
  Ends := IndexOfLine(Lines, '{#ProfileEnd}');

  SetArrayLength(Kept, 0);
  Written := 0;

  if (Begins >= 0) and (Ends > Begins) then
  begin
    { What stood between the markers is replaced: it named the previous install's directories, which are not
      this one's. What stood outside them is kept as it was, in order. }
    for At := 0 to Begins - 1 do
    begin
      SetArrayLength(Kept, Written + 1);
      Kept[Written] := Lines[At];
      Written := Written + 1;
    end;

    for At := 0 to GetArrayLength(Block) - 1 do
    begin
      SetArrayLength(Kept, Written + 1);
      Kept[Written] := Block[At];
      Written := Written + 1;
    end;

    for At := Ends + 1 to GetArrayLength(Lines) - 1 do
    begin
      SetArrayLength(Kept, Written + 1);
      Kept[Written] := Lines[At];
      Written := Written + 1;
    end;
  end
  else
  begin
    for At := 0 to GetArrayLength(Lines) - 1 do
    begin
      SetArrayLength(Kept, Written + 1);
      Kept[Written] := Lines[At];
      Written := Written + 1;
    end;

    if Written > 0 then
    begin
      SetArrayLength(Kept, Written + 1);
      Kept[Written] := '';
      Written := Written + 1;
    end;

    for At := 0 to GetArrayLength(Block) - 1 do
    begin
      SetArrayLength(Kept, Written + 1);
      Kept[Written] := Block[At];
      Written := Written + 1;
    end;
  end;

  if not DirExists(ExtractFileDir(Profile)) then
    ForceDirectories(ExtractFileDir(Profile));

  SaveStringsToFile(Profile, Kept, False);
end;

{ Takes the block back out of one profile, keeping a copy of what was there before it. }
procedure RemoveBlock(const Profile: String);
var
  Lines: TArrayOfString;
  Kept: TArrayOfString;
  Backup: String;
  At, Begins, Ends, Written: Integer;
begin
  if not FileExists(Profile) then
    Exit;

  Lines := LinesOf(Profile);
  Begins := IndexOfLine(Lines, '{#ProfileBegin}');
  Ends := IndexOfLine(Lines, '{#ProfileEnd}');

  { A profile without both markers is a profile this installer did not write into: it is left as it stands,
    which is the conservative answer and the one that cannot lose a reader's own lines. }
  if (Begins < 0) or (Ends <= Begins) then
    Exit;

  Backup := Profile + '.rorolala-backup';

  if not FileExists(Backup) then
    SaveStringsToFile(Backup, Lines, False);

  SetArrayLength(Kept, 0);
  Written := 0;

  for At := 0 to GetArrayLength(Lines) - 1 do
    if (At < Begins) or (At > Ends) then
    begin
      SetArrayLength(Kept, Written + 1);
      Kept[Written] := Lines[At];
      Written := Written + 1;
    end;

  SaveStringsToFile(Profile, Kept, False);
end;

{ Whether one entry of a path names the same directory as another, ignoring the spelling that does not
  matter: the case, and a trailing separator. }
function SamePathEntry(const One, Other: String): Boolean;
var
  Here, There: String;
begin
  Here := Trim(One);
  There := Trim(Other);

  while (Length(Here) > 1) and (Here[Length(Here)] = '\') do
    Delete(Here, Length(Here), 1);
  while (Length(There) > 1) and (There[Length(There)] = '\') do
    Delete(There, Length(There), 1);

  Result := CompareText(Here, There) = 0;
end;

{ Whether an entry is already one of the path's, read the way Windows reads a path: entries are separated by
  semicolons, and two that name the same directory are one entry. }
function PathHas(const Path, Entry: String): Boolean;
var
  Rest, One: String;
begin
  Result := False;
  Rest := Path;

  while Rest <> '' do
  begin
    One := Rest;
    if Pos(';', Rest) > 0 then
    begin
      One := Copy(Rest, 1, Pos(';', Rest) - 1);
      Rest := Copy(Rest, Pos(';', Rest) + 1, Length(Rest));
    end
    else
      Rest := '';

    if SamePathEntry(One, Entry) then
    begin
      Result := True;
      Exit;
    end;
  end;
end;

{ The path entries this install adds, as the one entry it is: the directory the programs are in. }
function BinDirectory: String;
begin
  Result := ExpandConstant('{app}\bin');
end;

function PathOfUser: String;
begin
  if not RegQueryStringValue(HKCU, EnvironmentKey, PathValue, Result) then
    Result := '';
end;

{ Adds the entry to the path, and says nothing when it is already there.

  What was written is read back, because a write that did not take is the one thing this cannot tell from a write
  that did: whether the entry is there is the environment's answer rather than the registry call's, and it is
  what an install that says it added something should check.

  A machine that refuses it is a machine where the rest of the install is still worth having — the programs are
  there and the profile is written — so a refusal is answered with False rather than thrown: an exception out of
  here would take the whole install back over one line of the environment. }
function AddToPath: Boolean;
var
  Said: String;
begin
  Result := True;
  Said := PathOfUser;

  if PathHas(Said, BinDirectory) then
    Exit;

  if Said = '' then
    Said := BinDirectory
  else
    Said := BinDirectory + ';' + Said;

  try
    RegWriteExpandStringValue(HKCU, EnvironmentKey, PathValue, Said);
  except
    Log('the path entry could not be written: ' + GetExceptionMessage);
  end;

  Result := PathHas(PathOfUser, BinDirectory);

  if not Result then
    Log('the path entry is not in the environment after being written');
end;

{ Takes the entry back out of the path, and touches nothing else in it.

  Conservative in the one way that matters: the entry is removed because it is the one this installer wrote —
  the directory the programs were installed into — and every other entry, whoever wrote it, is kept exactly
  as it was, in the order it was in. A path without the entry is left alone. }
procedure RemoveFromPath;
var
  Said, Rest, One, Kept: String;
  Taken: Boolean;
begin
  Said := PathOfUser;
  Taken := False;
  Kept := '';
  Rest := Said;

  while Rest <> '' do
  begin
    One := Rest;
    if Pos(';', Rest) > 0 then
    begin
      One := Copy(Rest, 1, Pos(';', Rest) - 1);
      Rest := Copy(Rest, Pos(';', Rest) + 1, Length(Rest));
    end
    else
      Rest := '';

    if SamePathEntry(One, BinDirectory) then
      Taken := True
    else if Kept = '' then
      Kept := One
    else
      Kept := Kept + ';' + One;
  end;

  if Taken then
    RegWriteExpandStringValue(HKCU, EnvironmentKey, PathValue, Kept);
end;

procedure InitializeWizard;
begin
  { What this is, before any question is asked. The picture is the wizard's own — see `WizardImageFile` in
    `[Setup]` — rather than one drawn here: a page of this script's own can show a Windows bitmap and nothing
    else, and the project's title is a PNG.

    The two words are asked for through `ExpandConstant`, and that is not ceremony: a custom message is expanded
    by the installer where a message is used, and a page of this script's own is not one of those places — a
    message written straight into one is shown with its braces and its name, which is what a reader saw. Note
    what this comment is made of as well: these become comments at the first closing brace, so a message constant
    is not written out inside one. }
  TitlePage := CreateCustomPage(
    wpWelcome,
    ExpandConstant('{cm:RuntimeTitle}'),
    ExpandConstant('{cm:TitleSubtitle}')
  );
end;

{ Says what the missing runtime costs before anything is copied, and lets a reader decide: a machine without
  it is a machine where the Desktop program will not start, and the command line programs are worth having on
  their own.

  A silent install is neither asked nor stopped. There is nobody there to answer, and Inno answers a script's
  question for them — with "no" — so a run that asked one would be a run that refused itself. What is said in
  that case is the log, and what is installed is everything. }
function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';

  if not RuntimeInstalled then
  begin
    if WizardSilent then
      Log('the .NET Desktop Runtime {#DotnetMajor} was not found: the Desktop program will not start until it is installed')
    else if MsgBox(RuntimeHelp, mbConfirmation, MB_YESNO) = IDNO then
      Result := ExpandConstant('{cm:RuntimeRefused}');
  end;
end;

{ Says, after everything is copied, what could not be done about the machine itself.

  The two things written outside the install directory — the block in the profile and the entry on the path — are
  the ones a machine may refuse, and a refusal of either is not a reason to take the programs back: what is said
  is what was not done, so that a reader is not left working out why a command they just installed is not found. }
procedure CurStepChanged(CurStep: TSetupStep);
var
  Said: TArrayOfString;
  At: Integer;
  Missed: String;
begin
  if CurStep <> ssPostInstall then
    Exit;

  Missed := '';

  { The block is a component of its own, and what is written into it depends on two others: a reader may want the
    completions sourced, or the path set, or one of the two. }
  if WizardIsComponentSelected('profile') then
  begin
    Said := Profiles;

    for At := 0 to GetArrayLength(Said) - 1 do
    begin
      try
        WriteBlock(
          Said[At],
          BlockFor(
            BinDirectory,
            ExpandConstant('{app}\scripts'),
            WizardIsComponentSelected('completions'),
            WizardIsComponentSelected('path')
          )
        );
      except
        Log('the profile block could not be written to ' + Said[At] + ': ' + GetExceptionMessage);
        Missed := Missed + #13#10 + Said[At];
      end;
    end;
  end;

  if WizardIsComponentSelected('path') and not AddToPath then
    Missed := Missed + #13#10 + BinDirectory + '  (the path)';

  if Missed <> '' then
    MsgBox(ExpandConstant('{cm:Missed}') + Missed, mbInformation, MB_OK);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Said: TArrayOfString;
  At: Integer;
begin
  if CurUninstallStep <> usUninstall then
    Exit;

  { The profile is taken back out whether or not the completions were installed: an install that was
    updated by a later one, or a choice made in the wizard, is not something an uninstall should have to
    reconstruct — and the removal only ever touches a block with this installer's own markers in it. }
  Said := Profiles;
  for At := 0 to GetArrayLength(Said) - 1 do
    RemoveBlock(Said[At]);

  RemoveFromPath;
end;
