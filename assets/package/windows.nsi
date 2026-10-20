Unicode true
!include "MUI2.nsh"
!include "x64.nsh"

!ifndef APP_VERSION
  !error "APP_VERSION must be supplied by package-windows.ps1"
!endif
!ifndef BINARY_PATH
  !error "BINARY_PATH must be supplied by package-windows.ps1"
!endif
!ifndef OUTPUT_PATH
  !error "OUTPUT_PATH must be supplied by package-windows.ps1"
!endif

Name "Kite"
OutFile "${OUTPUT_PATH}"
InstallDir "$LOCALAPPDATA\Programs\Kite"
InstallDirRegKey HKCU "Software\Kite" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
VIProductVersion "${NUMERIC_VERSION}.0"
VIAddVersionKey "ProductName" "Kite"
VIAddVersionKey "FileDescription" "Kite installer"
VIAddVersionKey "FileVersion" "${APP_VERSION}"
VIAddVersionKey "ProductVersion" "${APP_VERSION}"
VIAddVersionKey "LegalCopyright" "Kite contributors"

!define MUI_ABORTWARNING
!define MUI_ICON "${__FILEDIR__}\kite.ico"
!define MUI_UNICON "${__FILEDIR__}\kite.ico"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${RESOURCES_PATH}\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "This package requires 64-bit Windows."
    Abort
  ${EndIf}
  SetShellVarContext current
  SetRegView 64
FunctionEnd

Section "Install"
  SetOutPath "$INSTDIR"
  File /oname=kite.exe "${BINARY_PATH}"
  File "${RESOURCES_PATH}\kite.ico"
  File "${RESOURCES_PATH}\LICENSE"
  File /nonfatal "${RESOURCES_PATH}\README.md"
  File /nonfatal "${RESOURCES_PATH}\NOTICE"
  File /nonfatal "${RESOURCES_PATH}\THIRD_PARTY_NOTICES.md"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateDirectory "$SMPROGRAMS\Kite"
  CreateShortcut "$SMPROGRAMS\Kite\Kite.lnk" "$INSTDIR\kite.exe" "" "$INSTDIR\kite.ico" 0
  WriteRegStr HKCU "Software\Kite" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\kite" "DisplayName" "Kite"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\kite" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\kite" "DisplayIcon" "$INSTDIR\kite.ico"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\kite" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\kite" "UninstallString" '$"$INSTDIR\uninstall.exe$"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\kite" "QuietUninstallString" '$"$INSTDIR\uninstall.exe$" /S'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\kite" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\kite" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  SetRegView 64
  Delete "$INSTDIR\kite.exe"
  Delete "$INSTDIR\kite.ico"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\NOTICE"
  Delete "$INSTDIR\THIRD_PARTY_NOTICES.md"
  Delete "$INSTDIR\uninstall.exe"
  Delete "$SMPROGRAMS\Kite\Kite.lnk"
  RMDir "$SMPROGRAMS\Kite"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\kite"
  DeleteRegKey HKCU "Software\Kite"
SectionEnd
