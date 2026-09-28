; Inno Setup Script for TELEVAULT Windows 11 Installer
#define MyAppName "TELEVAULT"
#define MyAppVersion "2.1.4"
#define MyAppPublisher "Ankit Sharma (@ankitshx)"
#define MyAppURL "https://github.com/ankitshx/TELEVAULT"
#define MyAppExeName "TELEVAULT.exe"
#define MyAppBackendExeName "TELEVAULT-backend.exe"

[Setup]
AppId={{D99F26E4-998C-4D0C-B2B2-3788F9651C5E}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DisableProgramGroupPage=yes
OutputDir=..\release
OutputBaseFilename=TELEVAULT-Setup-x64
Compression=lzma2/ultra64
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequiredOverridesAllowed=dialog commandline
UsedUserAreasWarning=no
WizardStyle=modern
SetupIconFile=..\assets\icons\televault.ico
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "sendto"; Description: "Add to Windows Explorer 'Send To' menu"; GroupDescription: "Explorer Integration:"

[Files]
Source: "..\dist\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\{#MyAppBackendExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon
Name: "{userappdata}\Microsoft\Windows\SendTo\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Parameters: "backup ""%1"""; Tasks: sendto

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Code]
// Prompt during uninstallation to optionally preserve or remove user data
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  UserDataDirTeleVault: string;
  UserDataDirTeleCloud: string;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    UserDataDirTeleVault := ExpandConstant('{localappdata}\TeleVault');
    UserDataDirTeleCloud := ExpandConstant('{localappdata}\TeleCloud');
    
    if DirExists(UserDataDirTeleVault) or DirExists(UserDataDirTeleCloud) then
    begin
      // Prompt user with default button = NO (protect user data from accidental loss)
      if MsgBox('Do you also want to remove your TELEVAULT local database, configuration, and cached transfer data?' + #13#10 + #13#10 +
                'Click NO to keep your local data and Telegram session intact for future reinstalls.' + #13#10 +
                'Click YES to completely delete all local cache and databases.',
                mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
      begin
        if DirExists(UserDataDirTeleVault) then
          DelTree(UserDataDirTeleVault, True, True, True);
        if DirExists(UserDataDirTeleCloud) then
          DelTree(UserDataDirTeleCloud, True, True, True);
      end;
    end;
  end;
end;

