; The Dashboard's per-user installer (docs/release.md; docs/research/shell.md, section 8). Built by tools/package.sh
; with makensis, on Linux as on Windows:
;
;   makensis -DVERSION=0.1.0 -DSTAGE=<folder with dashboard.exe, README.txt, THIRD-PARTY-LICENSES.txt>
;            -DOUTFILE=<setup.exe> [-DSIGN_COMMAND=<command that signs the file named after it>] installer.nsi
;
; Install: no admin. The files to %LOCALAPPDATA%\Programs\XboxSeriesDashboard, a path that never changes between
; versions (shell mode names it in Winlogon's Shell and the sentinel's Run entry); a Start menu shortcut; and the
; one registry key a per-user install has, HKCU\...\Uninstall\XboxSeriesDashboard. Nothing of Winlogon, Run or Task
; Scheduler: those are shell mode's (#154). A copy that is running is renamed aside, dashboard.<version>.old, not
; asked to close: under shell mode the --shell process runs from it all session, and shell mode deletes .old at the
; next sign-in.
;
; Uninstall: `dashboard.exe --shell-off` first, and nothing more if it fails, so that Shell never names a missing exe.
; Then the files, the shortcut and the key. A dashboard.exe still in use (the --shell process this session) is left
; for a RunOnce entry to remove at the next sign-in, when explorer is the shell again. The data folder,
; %LOCALAPPDATA%\XboxSeriesDashboard, is kept unless "Your settings and library" is ticked, or /REMOVEDATA given.
;
; Command lines (NSIS's own: /S silent; /D=<folder> the install folder, last and unquoted):
;   setup.exe [/S] [/D=<folder>]
;   uninstall.exe [/S] [/REMOVEDATA] [/DATADIR=<folder>] [/DRYSHELL]
; /DATADIR and /DRYSHELL are for a test (e2e/payloads/install): the data folder named in place of the user's, given
; to --shell-off as --data-dir, and --shell-off run as a dry run (--dry-shell), which writes nothing; what it said
; is kept in <data>\uninstall-shell-off.log for the test to read.

Unicode true
ManifestDPIAware true
SetCompressor /SOLID lzma
RequestExecutionLevel user

!ifndef VERSION | STAGE | OUTFILE
  !error "VERSION, STAGE and OUTFILE are given by tools/package.sh"
!endif

!define NAME "Xbox Series Dashboard"
!define FOLDER "XboxSeriesDashboard"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${FOLDER}"
!define RUNONCE_KEY "Software\Microsoft\Windows\CurrentVersion\RunOnce"
!define SHORTCUT "$SMPROGRAMS\${NAME}.lnk"

; Signing (docs/release.md): the installer and the uninstaller inside it are signed as they are written, when
; tools/package.sh is given a command that signs. Unsigned otherwise.
!ifdef SIGN_COMMAND
  !finalize '${SIGN_COMMAND} "%1"' = 0
  !uninstfinalize '${SIGN_COMMAND} "%1"' = 0
!endif

!include LogicLib.nsh
!include FileFunc.nsh
!include MUI2.nsh

Name "${NAME}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\${FOLDER}"
BrandingText "${NAME} ${VERSION}"
ShowInstDetails show
ShowUninstDetails show

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${NAME}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "${NAME} setup"
VIAddVersionKey "LegalCopyright" ""

!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TEXT "This installs ${NAME} ${VERSION} for you alone, in your own folder:$\r$\n$\r$\n\
  $INSTDIR$\r$\n$\r$\nIt needs no administrator. Your settings and library are kept in \
  $LOCALAPPDATA\${FOLDER}, which uninstalling keeps unless you ask.$\r$\n$\r$\nThe console's sounds and boot videos \
  don't come with it: README.txt in the folder says where they come from."
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\dashboard.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Start ${NAME}"
!define MUI_FINISHPAGE_SHOWREADME "$INSTDIR\README.txt"
!define MUI_FINISHPAGE_SHOWREADME_TEXT "Show the README"
!define MUI_FINISHPAGE_SHOWREADME_NOTCHECKED
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!define MUI_COMPONENTSPAGE_TEXT_TOP "Uninstalling keeps your settings, Profiles and library unless you tick them here."
!insertmacro MUI_UNPAGE_COMPONENTS
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Var DataDir
Var ShellArgs
Var DryShell

Section "${NAME}" SecProgram
  SectionIn RO
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  ; A copy that is running can't be replaced, but can be renamed: it runs on from dashboard.<version>.old.
  ${If} ${FileExists} "$INSTDIR\dashboard.exe"
    ClearErrors
    Delete "$INSTDIR\dashboard.exe"
    ${If} ${Errors}
      ReadRegStr $0 HKCU "${UNINSTALL_KEY}" "DisplayVersion"
      ${If} $0 == ""
        StrCpy $0 "previous"
      ${EndIf}
      Delete "$INSTDIR\dashboard.$0.old"
      ClearErrors
      Rename "$INSTDIR\dashboard.exe" "$INSTDIR\dashboard.$0.old"
      ${If} ${Errors}
        DetailPrint "dashboard.exe is in use and couldn't be renamed aside"
        MessageBox MB_OK|MB_ICONSTOP "${NAME} is running and its file couldn't be replaced. Close it and run setup again." /SD IDOK
        SetErrorLevel 5
        Abort
      ${EndIf}
      DetailPrint "dashboard.exe is in use: renamed aside to dashboard.$0.old"
    ${EndIf}
  ${EndIf}
  File "${STAGE}/dashboard.exe"
  File "${STAGE}/README.txt"
  File "${STAGE}/THIRD-PARTY-LICENSES.txt"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "${SHORTCUT}" "$INSTDIR\dashboard.exe" "" "$INSTDIR\dashboard.exe" 0
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${NAME}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "${NAME}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\dashboard.exe"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" "$0"
SectionEnd

Section "un.${NAME}" SecUnProgram
  SectionIn RO
  SetShellVarContext current
  ; Shell mode off first, while the exe that knows how is still here. Off when it was never on says so and ends 0.
  ${If} ${FileExists} "$INSTDIR\dashboard.exe"
    DetailPrint "Turning shell mode off: dashboard.exe --shell-off$ShellArgs"
    nsExec::ExecToStack '"$INSTDIR\dashboard.exe" --shell-off$ShellArgs'
    Pop $0
    Pop $1
    DetailPrint "$1"
    ; A test's dry run keeps what it said in the test's data folder, where the test reads it.
    ${If} $DryShell == 1
      CreateDirectory "$DataDir"
      FileOpen $2 "$DataDir\uninstall-shell-off.log" w
      FileWrite $2 "exit $0$\r$\n$1"
      FileClose $2
    ${EndIf}
    ${If} $0 != 0
      DetailPrint "dashboard.exe --shell-off ended $0: nothing was uninstalled"
      MessageBox MB_OK|MB_ICONSTOP "Shell mode couldn't be turned off, so nothing was uninstalled: Windows would \
        otherwise start a program that is gone when you sign in.$\r$\n$\r$\nRun dashboard.exe --shell-off in \
        $INSTDIR to see why, then uninstall again." /SD IDOK
      SetErrorLevel 4
      Abort
    ${EndIf}
  ${EndIf}
  Delete "${SHORTCUT}"
  Delete "$INSTDIR\README.txt"
  Delete "$INSTDIR\THIRD-PARTY-LICENSES.txt"
  Delete "$INSTDIR\dashboard.*.old"
  ClearErrors
  Delete "$INSTDIR\dashboard.exe"
  ${If} ${Errors}
    ; In use: the --shell process of this session runs from it until sign-out.
    DetailPrint "dashboard.exe is in use: it is removed at your next sign-in"
    WriteRegStr HKCU "${RUNONCE_KEY}" "XboxSeriesDashboardCleanup" 'cmd.exe /c rmdir /s /q "$INSTDIR"'
  ${EndIf}
  Delete "$INSTDIR\uninstall.exe"
  ; Only an empty folder: whatever else someone put in it stays.
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd

Section /o "un.Your settings and library" SecUnData
  ${If} $DataDir != ""
  ${AndIf} ${FileExists} "$DataDir\*.*"
    DetailPrint "Removing $DataDir"
    RMDir /r "$DataDir"
  ${EndIf}
SectionEnd

!insertmacro MUI_UNFUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecUnProgram} "The program, its Start menu shortcut and its uninstall entry."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecUnData} "Your settings, Profiles, library, art and logs in $DataDir. \
    Leave it unticked to keep them for another install."
!insertmacro MUI_UNFUNCTION_DESCRIPTION_END

Function un.onInit
  SetShellVarContext current
  StrCpy $DataDir "$LOCALAPPDATA\${FOLDER}"
  StrCpy $ShellArgs ""
  StrCpy $DryShell 0
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/DATADIR=" $R1
  ${IfNot} ${Errors}
  ${AndIf} $R1 != ""
    StrCpy $DataDir $R1
    StrCpy $ShellArgs '$ShellArgs --data-dir "$R1"'
  ${EndIf}
  ClearErrors
  ${GetOptions} $R0 "/DRYSHELL" $R1
  ${IfNot} ${Errors}
    StrCpy $ShellArgs "$ShellArgs --dry-shell"
    StrCpy $DryShell 1
  ${EndIf}
  ClearErrors
  ${GetOptions} $R0 "/REMOVEDATA" $R1
  ${IfNot} ${Errors}
    SectionSetFlags ${SecUnData} ${SF_SELECTED}
  ${EndIf}
FunctionEnd
