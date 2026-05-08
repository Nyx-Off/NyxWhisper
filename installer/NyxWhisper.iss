; NyxWhisper - Inno Setup script
; Genere un installeur Windows propre avec choix CUDA / CPU.
;
; Compilation : iscc installer\NyxWhisper.iss
; Sortie      : installer\out\NyxWhisper-Setup-{version}.exe

#define MyAppName "NyxWhisper"
#define MyAppVersion "0.1.0"
#define MyAppPublisher "NyxWhisper"
#define MyAppExeName "NyxWhisper.exe"
#define MyAppId "{{A6A5C2F0-1E78-4D11-9F2A-NYXWHISPER001}"

[Setup]
AppId={#MyAppId}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
DisableDirPage=no
OutputDir=out
OutputBaseFilename=NyxWhisper-Setup-{#MyAppVersion}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64compatible
ArchitecturesAllowed=x64compatible
; lowest = pas besoin d'admin pour installer dans %LOCALAPPDATA%\Programs.
; L'utilisateur peut elever via le bouton de la fenetre si il veut Program Files.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
LicenseFile=..\LICENSE

[Languages]
Name: "french";  MessagesFile: "compiler:Languages\French.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: checkedonce
Name: "startupicon"; Description: "Lancer NyxWhisper au démarrage de Windows"; GroupDescription: "Démarrage automatique :"; Flags: unchecked

[Types]
Name: "cuda"; Description: "GPU NVIDIA (CUDA) - le plus rapide sur RTX/GTX recentes"
Name: "cpu";  Description: "CPU uniquement - aucune GPU requise (plus lent)"

[Components]
Name: "cuda"; Description: "Backend CUDA (NVIDIA)"; Types: cuda; Flags: exclusive
Name: "cpu";  Description: "Backend CPU";          Types: cpu;  Flags: exclusive

[Files]
; --- Variante CUDA (les DLLs sont copiees lors du build CUDA) ---
Source: "..\dist-cuda\NyxWhisper.exe";    DestDir: "{app}"; Components: cuda; Flags: ignoreversion
Source: "..\dist-cuda\cudart64_13.dll";   DestDir: "{app}"; Components: cuda; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\dist-cuda\cublas64_13.dll";   DestDir: "{app}"; Components: cuda; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\dist-cuda\cublasLt64_13.dll"; DestDir: "{app}"; Components: cuda; Flags: ignoreversion skipifsourcedoesntexist

; --- Variante CPU ---
Source: "..\dist-cpu\NyxWhisper.exe";     DestDir: "{app}"; Components: cpu;  Flags: ignoreversion

; --- Commun ---
Source: "..\README.md";                   DestDir: "{app}"; Flags: ignoreversion isreadme
Source: "..\scripts\download-model.ps1";  DestDir: "{app}\scripts"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}";  Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon
Name: "{userstartup}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: startupicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; On NE supprime PAS %LOCALAPPDATA%\NyxWhisper\models (plusieurs Go) ni
; %APPDATA%\NyxWhisper\config.toml a la desinstallation : l'utilisateur peut
; les supprimer manuellement s'il le souhaite.

[Code]
function HasNvidiaGpu(): Boolean;
var
  ResultCode: Integer;
  TmpFile: string;
  TmpContents: AnsiString;
begin
  Result := False;
  TmpFile := ExpandConstant('{tmp}\nyxgpu.txt');
  if Exec(ExpandConstant('{cmd}'), '/C wmic path win32_VideoController get Name > "' + TmpFile + '"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
  begin
    if LoadStringFromFile(TmpFile, TmpContents) then
    begin
      if Pos('NVIDIA', UpperCase(TmpContents)) > 0 then
        Result := True;
    end;
  end;
end;

function InitializeSetup(): Boolean;
begin
  Result := True;
end;
