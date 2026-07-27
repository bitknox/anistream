; anistream installer.
;
; Installs the binary, puts it on PATH, and adds a Start Menu shortcut. Both matter: anistream is
; a terminal program people will type, and also something they will look for in the Start Menu.
;
; The shortcut targets Windows Terminal rather than the binary directly. Launching a console
; program from a shortcut gets the legacy console host, which has no truecolor and no graphics
; protocol; wt.exe gets the modern one.
;
; Built by CI:  makensis -DVERSION=x.y.z -DSOURCE=<dir> anistream.nsi

Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "WinMessages.nsh"   ; HWND_BROADCAST, WM_WININICHANGE
!include "WordFunc.nsh"      ; WordReplace, for taking ourselves back off PATH

!ifndef VERSION
	!define VERSION "0.0.0"
!endif
!ifndef SOURCE
	!define SOURCE "."
!endif

Name "anistream ${VERSION}"
OutFile "anistream-${VERSION}-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\anistream"
InstallDirRegKey HKCU "Software\anistream" "InstallDir"
RequestExecutionLevel user     ; per-user install, so no UAC prompt
SetCompressor /SOLID lzma

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "anistream"
VIAddVersionKey "FileDescription" "An anime streaming TUI"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "MIT"

!define MUI_ICON "${SOURCE}\anistream.ico"
!define MUI_UNICON "${SOURCE}\anistream.ico"
!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_LICENSE "${SOURCE}\LICENSE"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "anistream (required)" SecCore
	SectionIn RO
	SetOutPath "$INSTDIR"
	File "${SOURCE}\anistream.exe"
	File "${SOURCE}\anistream.ico"
	File "${SOURCE}\LICENSE"
	File "${SOURCE}\README.md"

	WriteRegStr HKCU "Software\anistream" "InstallDir" "$INSTDIR"
	WriteUninstaller "$INSTDIR\uninstall.exe"

	; Add/Remove Programs.
	!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\anistream"
	WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "anistream"
	WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
	WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\anistream.ico"
	WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" "$INSTDIR\uninstall.exe"
	WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "anistream"
	WriteRegStr HKCU "${UNINST_KEY}" "URLInfoAbout" "https://anistream.tv"
	WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
	WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
SectionEnd

Section "Start Menu shortcut" SecShortcut
	CreateDirectory "$SMPROGRAMS\anistream"
	; Prefer Windows Terminal; fall back to running the binary directly where it is absent.
	${If} ${FileExists} "$LOCALAPPDATA\Microsoft\WindowsApps\wt.exe"
		CreateShortcut "$SMPROGRAMS\anistream\anistream.lnk" \
			"$LOCALAPPDATA\Microsoft\WindowsApps\wt.exe" \
			'--title anistream "$INSTDIR\anistream.exe"' \
			"$INSTDIR\anistream.ico" 0
	${Else}
		CreateShortcut "$SMPROGRAMS\anistream\anistream.lnk" \
			"$INSTDIR\anistream.exe" "" "$INSTDIR\anistream.ico" 0
	${EndIf}
	CreateShortcut "$SMPROGRAMS\anistream\Uninstall anistream.lnk" "$INSTDIR\uninstall.exe"
SectionEnd

Section "Add to PATH" SecPath
	; Per-user PATH, read-modify-write. Registry only — the environment of already-open shells
	; cannot be changed, which is why the finish text says to open a new terminal.
	ReadRegStr $0 HKCU "Environment" "Path"
	${If} $0 == ""
		WriteRegExpandStr HKCU "Environment" "Path" "$INSTDIR"
	${Else}
		${If} $0 != "*$INSTDIR*"
			WriteRegExpandStr HKCU "Environment" "Path" "$0;$INSTDIR"
		${EndIf}
	${EndIf}
	SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
	!insertmacro MUI_DESCRIPTION_TEXT ${SecCore} "The anistream binary."
	!insertmacro MUI_DESCRIPTION_TEXT ${SecShortcut} "A Start Menu entry that opens anistream in Windows Terminal."
	!insertmacro MUI_DESCRIPTION_TEXT ${SecPath} "Make `anistream` runnable from any terminal."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
	Delete "$INSTDIR\anistream.exe"
	Delete "$INSTDIR\anistream.ico"
	Delete "$INSTDIR\LICENSE"
	Delete "$INSTDIR\README.md"
	Delete "$INSTDIR\uninstall.exe"
	RMDir "$INSTDIR"

	Delete "$SMPROGRAMS\anistream\anistream.lnk"
	Delete "$SMPROGRAMS\anistream\Uninstall anistream.lnk"
	RMDir "$SMPROGRAMS\anistream"

	; Take ourselves back off PATH, leaving anything else the user added.
	ReadRegStr $0 HKCU "Environment" "Path"
	${WordReplace} "$0" ";$INSTDIR" "" "+" $1
	WriteRegExpandStr HKCU "Environment" "Path" "$1"
	SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000

	DeleteRegKey HKCU "Software\anistream"
	DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\anistream"
SectionEnd
