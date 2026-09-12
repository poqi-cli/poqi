Unicode True
RequestExecutionLevel user
ManifestDPIAware true

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "WinMessages.nsh"

!ifndef PRODUCT_VERSION
    !error "PRODUCT_VERSION is required"
!endif
!ifndef PRODUCT_VERSION_NUMBER
    !error "PRODUCT_VERSION_NUMBER is required"
!endif
!ifndef PACKAGE_DIR
    !error "PACKAGE_DIR is required"
!endif
!ifndef OUTPUT_FILE
    !error "OUTPUT_FILE is required"
!endif
!ifndef ESTIMATED_SIZE_KB
    !error "ESTIMATED_SIZE_KB is required"
!endif

!define PRODUCT_NAME "poqi"
!define PRODUCT_PUBLISHER "poqi"
!define PRODUCT_WEB_SITE "https://github.com/poqi-cli/poqi"
!define PRODUCT_APP_ID "poqi"

!ifdef POQI_TEST_NAMESPACE
    !define PATH_REGISTRY_SUBKEY "Software\poqi-installer-tests\${POQI_TEST_NAMESPACE}\Environment"
    !define PATH_OWNERSHIP_SUBKEY "Software\poqi-installer-tests\${POQI_TEST_NAMESPACE}\Installer"
    !define ARP_REGISTRY_SUBKEY "Software\poqi-installer-tests\${POQI_TEST_NAMESPACE}\Uninstall\${PRODUCT_APP_ID}"
!else
    !define PATH_REGISTRY_SUBKEY "Environment"
    !define PATH_OWNERSHIP_SUBKEY "Software\poqi\Installer"
    !define ARP_REGISTRY_SUBKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_APP_ID}"
!endif

Name "${PRODUCT_NAME} ${PRODUCT_VERSION}"
Caption "Install ${PRODUCT_NAME} ${PRODUCT_VERSION}"
BrandingText "poqi — PostgreSQL Query Interface"
OutFile "${OUTPUT_FILE}"
InstallDir "$PROFILE\.poqi\bin"
InstallDirRegKey HKCU "${ARP_REGISTRY_SUBKEY}" "InstallLocation"
SetCompressor zlib
ShowInstDetails show
ShowUninstDetails show

VIProductVersion "${PRODUCT_VERSION_NUMBER}"
VIAddVersionKey /LANG=1033 "ProductName" "poqi"
VIAddVersionKey /LANG=1033 "ProductVersion" "${PRODUCT_VERSION}"
VIAddVersionKey /LANG=1033 "CompanyName" "poqi"
VIAddVersionKey /LANG=1033 "FileDescription" "poqi setup"
VIAddVersionKey /LANG=1033 "FileVersion" "${PRODUCT_VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "See LICENSE included with poqi"

Var ExistingInstallDir
Var FailureMessage
Var PathChange
Var ShortcutDirectory
Var UpgradeMutationStarted
Var OldDisplayVersion
Var OldEstimatedSize

!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "Install poqi ${PRODUCT_VERSION}"
!define MUI_WELCOMEPAGE_TEXT "This wizard installs poqi, a PostgreSQL terminal client and SQL editor.$\r$\n$\r$\nSetup adds poqi to your user PATH and creates a Start menu shortcut."
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${PACKAGE_DIR}\LICENSE"
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE DirectoryLeave
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "Open poqi in a terminal"
!define MUI_FINISHPAGE_RUN_FUNCTION LaunchPoqi
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

!macro SetPathHelperEnvironment OperationName
    System::Call 'kernel32::SetEnvironmentVariableW(w, w)i("POQI_PATH_OPERATION", "${OperationName}").r0'
    System::Call 'kernel32::SetEnvironmentVariableW(w, w)i("POQI_PATH_INSTALL_DIRECTORY", "$INSTDIR").r0'
    System::Call 'kernel32::SetEnvironmentVariableW(w, w)i("POQI_PATH_REGISTRY_SUBKEY", "${PATH_REGISTRY_SUBKEY}").r0'
    System::Call 'kernel32::SetEnvironmentVariableW(w, w)i("POQI_PATH_OWNERSHIP_SUBKEY", "${PATH_OWNERSHIP_SUBKEY}").r0'
!macroend

!macro BroadcastEnvironmentChange
!ifndef POQI_TEST_NAMESPACE
    SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
!endif
!macroend

Function .onInit
    SetShellVarContext current
    ReadRegStr $ExistingInstallDir HKCU "${ARP_REGISTRY_SUBKEY}" "InstallLocation"
    ${If} $ExistingInstallDir != ""
        ReadRegStr $OldDisplayVersion HKCU "${ARP_REGISTRY_SUBKEY}" "DisplayVersion"
        ReadRegDWORD $OldEstimatedSize HKCU "${ARP_REGISTRY_SUBKEY}" "EstimatedSize"
        StrCpy $INSTDIR $ExistingInstallDir
    ${EndIf}
FunctionEnd

Function un.onInit
    SetShellVarContext current
FunctionEnd

Function SetShortcutDirectory
!ifdef POQI_TEST_NAMESPACE
    StrCpy $ShortcutDirectory "$INSTDIR\test-shortcuts"
!else
    StrCpy $ShortcutDirectory "$SMPROGRAMS\poqi"
!endif
FunctionEnd

Function un.SetShortcutDirectory
!ifdef POQI_TEST_NAMESPACE
    StrCpy $ShortcutDirectory "$INSTDIR\test-shortcuts"
!else
    StrCpy $ShortcutDirectory "$SMPROGRAMS\poqi"
!endif
FunctionEnd

Function DirectoryLeave
    ${If} $ExistingInstallDir == ""
        Return
    ${EndIf}
    System::Call 'kernel32::lstrcmpiW(w "$INSTDIR", w "$ExistingInstallDir")i.r0'
    ${If} $0 != 0
        MessageBox MB_OK|MB_ICONEXCLAMATION "poqi is already installed in:$\r$\n$ExistingInstallDir$\r$\n$\r$\nUninstall it before choosing another location."
        Abort
    ${EndIf}
FunctionEnd

Function LaunchPoqi
    Exec '"$SYSDIR\cmd.exe" /D /K ""$INSTDIR\poqi.exe""'
FunctionEnd

Function RunPathAdd
    !insertmacro SetPathHelperEnvironment "Add"
    nsExec::ExecToStack /TIMEOUT=30000 '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\update-path.ps1"'
    Pop $PathChange
    Pop $0
    ${If} $PathChange == "0"
        Return
    ${EndIf}
    ${If} $PathChange == "10"
        !insertmacro BroadcastEnvironmentChange
        Return
    ${EndIf}
    StrCpy $FailureMessage "Could not add poqi to your user PATH.$\r$\n$\r$\n$0"
    Call InstallFailed
FunctionEnd

Function VerifyFreshInstallDestination
    ${If} $ExistingInstallDir != ""
        Return
    ${EndIf}

    IfFileExists "$INSTDIR\poqi.exe" fresh_install_collision
    IfFileExists "$INSTDIR\LICENSE" fresh_install_collision
    IfFileExists "$INSTDIR\THIRD-PARTY-LICENSES.txt" fresh_install_collision
    IfFileExists "$INSTDIR\README.md" fresh_install_collision
    IfFileExists "$INSTDIR\CHANGELOG.md" fresh_install_collision
    IfFileExists "$INSTDIR\NSIS-LICENSE.txt" fresh_install_collision
    IfFileExists "$INSTDIR\update-path.ps1" fresh_install_collision
    IfFileExists "$INSTDIR\Uninstall.exe" fresh_install_collision
    Return

fresh_install_collision:
    SetErrorLevel 1
    IfSilent fresh_install_collision_silent
    MessageBox MB_OK|MB_ICONSTOP "The selected folder already contains a file owned by poqi setup.$\r$\n$\r$\nMove or remove the previous portable/script installation, or choose an empty folder, then retry. No files were changed."
fresh_install_collision_silent:
    Quit
FunctionEnd

Function PrepareUpgradeBackup
    StrCpy $UpgradeMutationStarted "0"
    ${If} $ExistingInstallDir == ""
        Return
    ${EndIf}

    InitPluginsDir
    ClearErrors
    CreateDirectory "$PLUGINSDIR\poqi-backup"
    IfFileExists "$INSTDIR\poqi.exe" 0 +2
    CopyFiles /SILENT "$INSTDIR\poqi.exe" "$PLUGINSDIR\poqi-backup\poqi.exe"
    IfFileExists "$INSTDIR\LICENSE" 0 +2
    CopyFiles /SILENT "$INSTDIR\LICENSE" "$PLUGINSDIR\poqi-backup\LICENSE"
    IfFileExists "$INSTDIR\THIRD-PARTY-LICENSES.txt" 0 +2
    CopyFiles /SILENT "$INSTDIR\THIRD-PARTY-LICENSES.txt" "$PLUGINSDIR\poqi-backup\THIRD-PARTY-LICENSES.txt"
    IfFileExists "$INSTDIR\README.md" 0 +2
    CopyFiles /SILENT "$INSTDIR\README.md" "$PLUGINSDIR\poqi-backup\README.md"
    IfFileExists "$INSTDIR\CHANGELOG.md" 0 +2
    CopyFiles /SILENT "$INSTDIR\CHANGELOG.md" "$PLUGINSDIR\poqi-backup\CHANGELOG.md"
    IfFileExists "$INSTDIR\NSIS-LICENSE.txt" 0 +2
    CopyFiles /SILENT "$INSTDIR\NSIS-LICENSE.txt" "$PLUGINSDIR\poqi-backup\NSIS-LICENSE.txt"
    IfFileExists "$INSTDIR\update-path.ps1" 0 +2
    CopyFiles /SILENT "$INSTDIR\update-path.ps1" "$PLUGINSDIR\poqi-backup\update-path.ps1"
    IfFileExists "$INSTDIR\Uninstall.exe" 0 +2
    CopyFiles /SILENT "$INSTDIR\Uninstall.exe" "$PLUGINSDIR\poqi-backup\Uninstall.exe"
    IfErrors 0 +3
    StrCpy $FailureMessage "Could not preserve the existing poqi installation for a safe upgrade."
    Call InstallFailed

    ClearErrors
    IfFileExists "$INSTDIR\poqi.exe" 0 +4
    FileOpen $0 "$INSTDIR\poqi.exe" a
    IfErrors upgrade_file_in_use
    FileClose $0
    IfFileExists "$INSTDIR\LICENSE" 0 +4
    FileOpen $0 "$INSTDIR\LICENSE" a
    IfErrors upgrade_file_in_use
    FileClose $0
    IfFileExists "$INSTDIR\THIRD-PARTY-LICENSES.txt" 0 +4
    FileOpen $0 "$INSTDIR\THIRD-PARTY-LICENSES.txt" a
    IfErrors upgrade_file_in_use
    FileClose $0
    IfFileExists "$INSTDIR\README.md" 0 +4
    FileOpen $0 "$INSTDIR\README.md" a
    IfErrors upgrade_file_in_use
    FileClose $0
    IfFileExists "$INSTDIR\CHANGELOG.md" 0 +4
    FileOpen $0 "$INSTDIR\CHANGELOG.md" a
    IfErrors upgrade_file_in_use
    FileClose $0
    IfFileExists "$INSTDIR\NSIS-LICENSE.txt" 0 +4
    FileOpen $0 "$INSTDIR\NSIS-LICENSE.txt" a
    IfErrors upgrade_file_in_use
    FileClose $0
    IfFileExists "$INSTDIR\update-path.ps1" 0 +4
    FileOpen $0 "$INSTDIR\update-path.ps1" a
    IfErrors upgrade_file_in_use
    FileClose $0
    IfFileExists "$INSTDIR\Uninstall.exe" 0 +4
    FileOpen $0 "$INSTDIR\Uninstall.exe" a
    IfErrors upgrade_file_in_use
    FileClose $0
    Return

upgrade_file_in_use:
    StrCpy $FailureMessage "An installed poqi file is in use. Close poqi and retry the upgrade. No files were changed."
    Call InstallFailed
FunctionEnd

Function RestoreUpgradePayload
    ${If} $UpgradeMutationStarted != "1"
        Return
    ${EndIf}

    Delete "$INSTDIR\poqi.exe"
    Delete "$INSTDIR\LICENSE"
    Delete "$INSTDIR\THIRD-PARTY-LICENSES.txt"
    Delete "$INSTDIR\README.md"
    Delete "$INSTDIR\CHANGELOG.md"
    Delete "$INSTDIR\NSIS-LICENSE.txt"
    Delete "$INSTDIR\update-path.ps1"
    Delete "$INSTDIR\Uninstall.exe"
    ClearErrors
    IfFileExists "$PLUGINSDIR\poqi-backup\poqi.exe" 0 +2
    CopyFiles /SILENT "$PLUGINSDIR\poqi-backup\poqi.exe" "$INSTDIR\poqi.exe"
    IfFileExists "$PLUGINSDIR\poqi-backup\LICENSE" 0 +2
    CopyFiles /SILENT "$PLUGINSDIR\poqi-backup\LICENSE" "$INSTDIR\LICENSE"
    IfFileExists "$PLUGINSDIR\poqi-backup\THIRD-PARTY-LICENSES.txt" 0 +2
    CopyFiles /SILENT "$PLUGINSDIR\poqi-backup\THIRD-PARTY-LICENSES.txt" "$INSTDIR\THIRD-PARTY-LICENSES.txt"
    IfFileExists "$PLUGINSDIR\poqi-backup\README.md" 0 +2
    CopyFiles /SILENT "$PLUGINSDIR\poqi-backup\README.md" "$INSTDIR\README.md"
    IfFileExists "$PLUGINSDIR\poqi-backup\CHANGELOG.md" 0 +2
    CopyFiles /SILENT "$PLUGINSDIR\poqi-backup\CHANGELOG.md" "$INSTDIR\CHANGELOG.md"
    IfFileExists "$PLUGINSDIR\poqi-backup\NSIS-LICENSE.txt" 0 +2
    CopyFiles /SILENT "$PLUGINSDIR\poqi-backup\NSIS-LICENSE.txt" "$INSTDIR\NSIS-LICENSE.txt"
    IfFileExists "$PLUGINSDIR\poqi-backup\update-path.ps1" 0 +2
    CopyFiles /SILENT "$PLUGINSDIR\poqi-backup\update-path.ps1" "$INSTDIR\update-path.ps1"
    IfFileExists "$PLUGINSDIR\poqi-backup\Uninstall.exe" 0 +2
    CopyFiles /SILENT "$PLUGINSDIR\poqi-backup\Uninstall.exe" "$INSTDIR\Uninstall.exe"
    IfErrors 0 +3
    StrCpy $0 $FailureMessage
    StrCpy $FailureMessage "$0$\r$\n$\r$\nThe previous poqi files could not be restored completely."
FunctionEnd

Function RestoreUpgradeRegistration
    WriteRegStr HKCU "${ARP_REGISTRY_SUBKEY}" "DisplayName" "poqi"
    WriteRegStr HKCU "${ARP_REGISTRY_SUBKEY}" "DisplayVersion" "$OldDisplayVersion"
    WriteRegStr HKCU "${ARP_REGISTRY_SUBKEY}" "Publisher" "${PRODUCT_PUBLISHER}"
    WriteRegStr HKCU "${ARP_REGISTRY_SUBKEY}" "URLInfoAbout" "${PRODUCT_WEB_SITE}"
    WriteRegStr HKCU "${ARP_REGISTRY_SUBKEY}" "DisplayIcon" "$ExistingInstallDir\poqi.exe"
    WriteRegStr HKCU "${ARP_REGISTRY_SUBKEY}" "InstallLocation" "$ExistingInstallDir"
    WriteRegStr HKCU "${ARP_REGISTRY_SUBKEY}" "UninstallString" "$\"$ExistingInstallDir\Uninstall.exe$\""
    WriteRegStr HKCU "${ARP_REGISTRY_SUBKEY}" "QuietUninstallString" "$\"$ExistingInstallDir\Uninstall.exe$\" /S"
    WriteRegDWORD HKCU "${ARP_REGISTRY_SUBKEY}" "NoModify" 1
    WriteRegDWORD HKCU "${ARP_REGISTRY_SUBKEY}" "NoRepair" 1
    WriteRegDWORD HKCU "${ARP_REGISTRY_SUBKEY}" "EstimatedSize" $OldEstimatedSize
FunctionEnd

Function InstallFailed
    ${If} $PathChange == "10"
        !insertmacro SetPathHelperEnvironment "Remove"
        nsExec::ExecToStack /TIMEOUT=30000 '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\update-path.ps1"'
        Pop $0
        Pop $1
        !insertmacro BroadcastEnvironmentChange
    ${EndIf}

    ${If} $ExistingInstallDir == ""
        Call SetShortcutDirectory
        Delete "$ShortcutDirectory\poqi.lnk"
        RMDir "$ShortcutDirectory"
        DeleteRegKey HKCU "${ARP_REGISTRY_SUBKEY}"
        Delete "$INSTDIR\poqi.exe"
        Delete "$INSTDIR\LICENSE"
        Delete "$INSTDIR\THIRD-PARTY-LICENSES.txt"
        Delete "$INSTDIR\README.md"
        Delete "$INSTDIR\CHANGELOG.md"
        Delete "$INSTDIR\NSIS-LICENSE.txt"
        Delete "$INSTDIR\update-path.ps1"
        Delete "$INSTDIR\Uninstall.exe"
        SetOutPath "$TEMP"
        RMDir "$INSTDIR"
    ${Else}
        Call RestoreUpgradePayload
        Call RestoreUpgradeRegistration
    ${EndIf}

    SetErrorLevel 1
    IfSilent install_failed_silent
    MessageBox MB_OK|MB_ICONSTOP "$FailureMessage$\r$\n$\r$\nSetup did not complete."
install_failed_silent:
    Quit
FunctionEnd

Section "poqi" MainSection
    Call VerifyFreshInstallDestination

    ${If} $ExistingInstallDir != ""
        System::Call 'kernel32::lstrcmpiW(w "$INSTDIR", w "$ExistingInstallDir")i.r0'
        ${If} $0 != 0
            StrCpy $FailureMessage "An existing poqi installation uses a different location: $ExistingInstallDir"
            StrCpy $PathChange "0"
            Call InstallFailed
        ${EndIf}
    ${EndIf}

    Call PrepareUpgradeBackup
    StrCpy $UpgradeMutationStarted "1"
    ClearErrors
    SetOutPath "$INSTDIR"
    File "${PACKAGE_DIR}\poqi.exe"
    File "${PACKAGE_DIR}\LICENSE"
    File "${PACKAGE_DIR}\THIRD-PARTY-LICENSES.txt"
    File "${PACKAGE_DIR}\README.md"
    File "${PACKAGE_DIR}\CHANGELOG.md"
    File "${__FILEDIR__}\NSIS-LICENSE.txt"
    File "${__FILEDIR__}\update-path.ps1"
    WriteUninstaller "$INSTDIR\Uninstall.exe"
    IfErrors 0 +3
    StrCpy $FailureMessage "Could not write the poqi program files. Close poqi and retry."
    Call InstallFailed

    StrCpy $PathChange "0"
    Call RunPathAdd

    Call SetShortcutDirectory
    ClearErrors
    CreateDirectory "$ShortcutDirectory"
    CreateShortCut "$ShortcutDirectory\poqi.lnk" "$SYSDIR\cmd.exe" '/D /K ""$INSTDIR\poqi.exe""' "$INSTDIR\poqi.exe" 0 SW_SHOWNORMAL "" "poqi — PostgreSQL Query Interface"
    IfErrors 0 +3
    StrCpy $FailureMessage "Could not create the poqi Start menu shortcut."
    Call InstallFailed

    StrCpy $FailureMessage "Could not register poqi in Add or Remove Programs."
    ClearErrors
    WriteRegStr HKCU "${ARP_REGISTRY_SUBKEY}" "DisplayName" "poqi"
    IfErrors 0 +2
    Call InstallFailed
    ClearErrors
    WriteRegStr HKCU "${ARP_REGISTRY_SUBKEY}" "DisplayVersion" "${PRODUCT_VERSION}"
    IfErrors 0 +2
    Call InstallFailed
    ClearErrors
    WriteRegStr HKCU "${ARP_REGISTRY_SUBKEY}" "Publisher" "${PRODUCT_PUBLISHER}"
    IfErrors 0 +2
    Call InstallFailed
    ClearErrors
    WriteRegStr HKCU "${ARP_REGISTRY_SUBKEY}" "URLInfoAbout" "${PRODUCT_WEB_SITE}"
    IfErrors 0 +2
    Call InstallFailed
    ClearErrors
    WriteRegStr HKCU "${ARP_REGISTRY_SUBKEY}" "DisplayIcon" "$INSTDIR\poqi.exe"
    IfErrors 0 +2
    Call InstallFailed
    ClearErrors
    WriteRegStr HKCU "${ARP_REGISTRY_SUBKEY}" "InstallLocation" "$INSTDIR"
    IfErrors 0 +2
    Call InstallFailed
    ClearErrors
    WriteRegStr HKCU "${ARP_REGISTRY_SUBKEY}" "UninstallString" "$\"$INSTDIR\Uninstall.exe$\""
    IfErrors 0 +2
    Call InstallFailed
    ClearErrors
    WriteRegStr HKCU "${ARP_REGISTRY_SUBKEY}" "QuietUninstallString" "$\"$INSTDIR\Uninstall.exe$\" /S"
    IfErrors 0 +2
    Call InstallFailed
    ClearErrors
    WriteRegDWORD HKCU "${ARP_REGISTRY_SUBKEY}" "NoModify" 1
    IfErrors 0 +2
    Call InstallFailed
    ClearErrors
    WriteRegDWORD HKCU "${ARP_REGISTRY_SUBKEY}" "NoRepair" 1
    IfErrors 0 +2
    Call InstallFailed
    ClearErrors
    WriteRegDWORD HKCU "${ARP_REGISTRY_SUBKEY}" "EstimatedSize" ${ESTIMATED_SIZE_KB}
    IfErrors 0 +2
    Call InstallFailed
SectionEnd

Function un.PrepareRemovalBackup
    InitPluginsDir
    ClearErrors
    CreateDirectory "$PLUGINSDIR\poqi-uninstall-backup"
    IfFileExists "$INSTDIR\poqi.exe" 0 +2
    CopyFiles /SILENT "$INSTDIR\poqi.exe" "$PLUGINSDIR\poqi-uninstall-backup\poqi.exe"
    IfFileExists "$INSTDIR\LICENSE" 0 +2
    CopyFiles /SILENT "$INSTDIR\LICENSE" "$PLUGINSDIR\poqi-uninstall-backup\LICENSE"
    IfFileExists "$INSTDIR\THIRD-PARTY-LICENSES.txt" 0 +2
    CopyFiles /SILENT "$INSTDIR\THIRD-PARTY-LICENSES.txt" "$PLUGINSDIR\poqi-uninstall-backup\THIRD-PARTY-LICENSES.txt"
    IfFileExists "$INSTDIR\README.md" 0 +2
    CopyFiles /SILENT "$INSTDIR\README.md" "$PLUGINSDIR\poqi-uninstall-backup\README.md"
    IfFileExists "$INSTDIR\CHANGELOG.md" 0 +2
    CopyFiles /SILENT "$INSTDIR\CHANGELOG.md" "$PLUGINSDIR\poqi-uninstall-backup\CHANGELOG.md"
    IfFileExists "$INSTDIR\NSIS-LICENSE.txt" 0 +2
    CopyFiles /SILENT "$INSTDIR\NSIS-LICENSE.txt" "$PLUGINSDIR\poqi-uninstall-backup\NSIS-LICENSE.txt"
    IfFileExists "$INSTDIR\update-path.ps1" 0 +2
    CopyFiles /SILENT "$INSTDIR\update-path.ps1" "$PLUGINSDIR\poqi-uninstall-backup\update-path.ps1"
    IfErrors 0 +5
    SetErrorLevel 1
    IfSilent removal_backup_silent
    MessageBox MB_OK|MB_ICONSTOP "Could not prepare poqi for safe removal. Close programs using its files and retry. No files were removed."
removal_backup_silent:
    Quit
FunctionEnd

Function un.RestoreRemovalAndFail
    IfFileExists "$PLUGINSDIR\poqi-uninstall-backup\poqi.exe" 0 +2
    CopyFiles /SILENT "$PLUGINSDIR\poqi-uninstall-backup\poqi.exe" "$INSTDIR\poqi.exe"
    IfFileExists "$PLUGINSDIR\poqi-uninstall-backup\LICENSE" 0 +2
    CopyFiles /SILENT "$PLUGINSDIR\poqi-uninstall-backup\LICENSE" "$INSTDIR\LICENSE"
    IfFileExists "$PLUGINSDIR\poqi-uninstall-backup\THIRD-PARTY-LICENSES.txt" 0 +2
    CopyFiles /SILENT "$PLUGINSDIR\poqi-uninstall-backup\THIRD-PARTY-LICENSES.txt" "$INSTDIR\THIRD-PARTY-LICENSES.txt"
    IfFileExists "$PLUGINSDIR\poqi-uninstall-backup\README.md" 0 +2
    CopyFiles /SILENT "$PLUGINSDIR\poqi-uninstall-backup\README.md" "$INSTDIR\README.md"
    IfFileExists "$PLUGINSDIR\poqi-uninstall-backup\CHANGELOG.md" 0 +2
    CopyFiles /SILENT "$PLUGINSDIR\poqi-uninstall-backup\CHANGELOG.md" "$INSTDIR\CHANGELOG.md"
    IfFileExists "$PLUGINSDIR\poqi-uninstall-backup\NSIS-LICENSE.txt" 0 +2
    CopyFiles /SILENT "$PLUGINSDIR\poqi-uninstall-backup\NSIS-LICENSE.txt" "$INSTDIR\NSIS-LICENSE.txt"
    IfFileExists "$PLUGINSDIR\poqi-uninstall-backup\update-path.ps1" 0 +2
    CopyFiles /SILENT "$PLUGINSDIR\poqi-uninstall-backup\update-path.ps1" "$INSTDIR\update-path.ps1"
    Call un.SetShortcutDirectory
    CreateDirectory "$ShortcutDirectory"
    CreateShortCut "$ShortcutDirectory\poqi.lnk" "$SYSDIR\cmd.exe" '/D /K ""$INSTDIR\poqi.exe""' "$INSTDIR\poqi.exe" 0 SW_SHOWNORMAL "" "poqi — PostgreSQL Query Interface"

    ${If} $PathChange == "10"
        !insertmacro SetPathHelperEnvironment "Add"
        nsExec::ExecToStack /TIMEOUT=30000 '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\poqi-uninstall-backup\update-path.ps1"'
        Pop $0
        Pop $1
        !insertmacro BroadcastEnvironmentChange
    ${EndIf}

    SetErrorLevel 1
    IfSilent removal_failed_silent
    MessageBox MB_OK|MB_ICONSTOP "$FailureMessage$\r$\n$\r$\nThe previous poqi installation was kept. Close poqi and any program using its files, then retry."
removal_failed_silent:
    Quit
FunctionEnd

Section "Uninstall"
    ReadRegStr $0 HKCU "${ARP_REGISTRY_SUBKEY}" "InstallLocation"
    ${If} $0 != ""
        System::Call 'kernel32::lstrcmpiW(w "$INSTDIR", w "$0")i.r1'
        ${If} $1 != 0
            SetErrorLevel 1
            IfSilent uninstall_location_silent
            MessageBox MB_OK|MB_ICONSTOP "The recorded poqi install location does not match this uninstaller. No files were removed."
uninstall_location_silent:
            Quit
        ${EndIf}
    ${EndIf}

    Call un.PrepareRemovalBackup

    !insertmacro SetPathHelperEnvironment "Remove"
    nsExec::ExecToStack /TIMEOUT=30000 '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\poqi-uninstall-backup\update-path.ps1"'
    Pop $PathChange
    Pop $0
    ${If} $PathChange != "0"
        ${If} $PathChange != "10"
            SetErrorLevel 1
            IfSilent uninstall_path_silent
            MessageBox MB_OK|MB_ICONSTOP "Could not remove the installer-owned poqi PATH entry.$\r$\n$\r$\n$0$\r$\n$\r$\nNo program files were removed."
uninstall_path_silent:
            Quit
        ${EndIf}
        !insertmacro BroadcastEnvironmentChange
    ${EndIf}

    ClearErrors
    Delete "$INSTDIR\poqi.exe"
    Delete "$INSTDIR\LICENSE"
    Delete "$INSTDIR\THIRD-PARTY-LICENSES.txt"
    Delete "$INSTDIR\README.md"
    Delete "$INSTDIR\CHANGELOG.md"
    Delete "$INSTDIR\NSIS-LICENSE.txt"
    Delete "$INSTDIR\update-path.ps1"
    IfErrors 0 +3
    StrCpy $FailureMessage "Could not remove all installed poqi files."
    Call un.RestoreRemovalAndFail

    Call un.SetShortcutDirectory
    ClearErrors
    Delete "$ShortcutDirectory\poqi.lnk"
    IfErrors 0 +3
    StrCpy $FailureMessage "Could not remove the poqi Start menu shortcut."
    Call un.RestoreRemovalAndFail
    RMDir "$ShortcutDirectory"

    ClearErrors
    DeleteRegKey HKCU "${ARP_REGISTRY_SUBKEY}"
    IfErrors 0 +3
    StrCpy $FailureMessage "Could not remove poqi from Add or Remove Programs."
    Call un.RestoreRemovalAndFail

    Delete "$INSTDIR\Uninstall.exe"
    SetOutPath "$TEMP"
    RMDir "$INSTDIR"
SectionEnd
