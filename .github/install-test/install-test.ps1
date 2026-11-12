# The install test (Step 5 of phase 6), run by release.yml on a clean
# Windows machine: install, double-click a picture, update (a wrong
# fingerprint refused; a setup that dies halfway put right; a good one
# kept with the settings), uninstall, and screenshots of each part.
#
# The "old" Duck Quill here is a test build that says it's 0.0.1 and asks
# a little server on this machine (127.0.0.1:8765) for releases instead of
# GitHub. Published builds can't do that (release.yml checks).
param(
  [Parameter(Mandatory)] [string] $Version,
  # The setups: DuckQuill-<Version>-setup.exe, DuckQuill-0.0.1-setup.exe,
  # and broken\DuckQuill-<Version>-setup.exe (dies halfway on purpose).
  [Parameter(Mandatory)] [string] $Files
)
$ErrorActionPreference = 'Stop'
$Here = (Get-Location).Path
$Shots = "$Here\screenshots"
$Logs = "$Here\logs"
New-Item -ItemType Directory -Force $Shots, $Logs | Out-Null
$App = "$env:LOCALAPPDATA\Programs\Duck Quill"
$SettingsFolder = "$env:APPDATA\Duck Quill"
$Pictures = "$Here\pictures"
$U = "$env:TEMP\Duck Quill update"
$AppId = '{6E0F3C1A-9B2D-4C57-8E4A-D2B7F05A13C9}_is1'
$StartMenu = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Duck Quill.lnk"
$Setup = "$Files\DuckQuill-$Version-setup.exe"
$OldSetup = "$Files\DuckQuill-0.0.1-setup.exe"
$BrokenSetup = "$Files\broken\DuckQuill-$Version-setup.exe"
$script:failures = @()

function Check([bool] $ok, [string] $what) {
  if ($ok) { Write-Host "  ok    $what" } else { Write-Host "  FAIL  $what"; $script:failures += $what }
}
function Step([string] $what) { Write-Host ""; Write-Host "== $what" }
function Shot([string] $name) {
  try {
    Add-Type -AssemblyName System.Windows.Forms, System.Drawing
    $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
    $bitmap.Save("$Shots\$name.png")
    Write-Host "  (screenshot $name)"
  } catch { Write-Host "  (no screenshot $name`: $_)" }
}
function Installed-Version {
  $file = "$env:TEMP\dq-version.txt"
  Remove-Item $file -ErrorAction SilentlyContinue
  if (-not (Test-Path "$App\DuckQuill.exe")) { return '' }
  Start-Process "$App\DuckQuill.exe" -ArgumentList '--version-to', "`"$file`"" -Wait
  if (Test-Path $file) { (Get-Content $file -Raw).Trim() } else { '' }
}
function Wait-For([string] $what, [scriptblock] $test, [int] $seconds = 180) {
  for ($i = 0; $i -lt $seconds; $i++) {
    if (& $test) { return $true }
    Start-Sleep -Seconds 1
  }
  Write-Host "  (waited $seconds s for: $what)"
  return $false
}
function Test-Log { if (Test-Path "$U\test-log.txt") { Get-Content "$U\test-log.txt" -Raw } else { '' } }
function Folder-Hashes {
  Get-ChildItem -Recurse -File $App | Sort-Object FullName | ForEach-Object {
    "$($_.FullName.Substring($App.Length)) $((Get-FileHash $_.FullName).Hash)"
  } | Out-String
}
function Windows-Of-App {
  @(Get-Process DuckQuill -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 })
}
function Title { (Windows-Of-App | Select-Object -First 1).MainWindowTitle }
function Registry-Default([string] $key) {
  $item = Get-Item -Path "Registry::$key" -ErrorAction SilentlyContinue
  if ($item) { $item.GetValue('') } else { $null }
}
function User-Path { [Environment]::GetEnvironmentVariable('Path', 'User') }
function Ask-For-Update { New-Item -ItemType Directory -Force $U | Out-Null; Set-Content "$U\update-now" 'go' }
function Send-Keys([string] $keys, [string] $window) {
  $shell = New-Object -ComObject WScript.Shell
  if ($window) { $null = $shell.AppActivate($window) }
  Start-Sleep -Milliseconds 400
  $shell.SendKeys($keys)
}
function Serve([string] $setup, [string] $sums) {
  # What the little server hands out as the newest release's files.
  Copy-Item $setup "$Here\server\p\DuckQuill-$Version-setup.exe" -Force
  Set-Content "$Here\server\p\SHA256SUMS.txt" $sums -NoNewline
}
function Sums-Of([string] $file) {
  "$((Get-FileHash $file -Algorithm SHA256).Hash.ToLower())  DuckQuill-$Version-setup.exe`n"
}

Step "A clean machine"
Check (-not (Test-Path $App)) "Duck Quill isn't installed yet"
Check ($null -eq (Registry-Default 'HKCU\Software\Classes\.quill')) ".quill isn't known yet"

Step "Install the old version (0.0.1), with the PATH tick box"
Start-Process $OldSetup -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/TASKS="quillpath"', "/LOG=`"$Logs\install-old.log`"" -Wait
Check (Test-Path "$App\DuckQuill.exe") "DuckQuill.exe is in $App"
Check (Test-Path "$App\quill.exe") "quill.exe is there too"
Check (Test-Path "$App\skills\duck-quill\SKILL.md") "the skill folder is there"
Check (Test-Path "$App\installed.ini") "it knows it was installed (installed.ini)"
Check ((Installed-Version) -eq '0.0.1') "it says it's 0.0.1"
Check (Test-Path $StartMenu) "it's in the Start menu"
Check ((Registry-Default 'HKCU\Software\Classes\.quill') -eq 'DuckQuill.Picture') ".quill opens in Duck Quill"
Check ((Registry-Default 'HKCU\Software\Classes\.artapp') -eq 'DuckQuill.Picture') ".artapp too"
Check ((Registry-Default 'HKCU\Software\Classes\DuckQuill.Picture\shell\open\command') -eq "`"$App\DuckQuill.exe`" `"%1`"") "with DuckQuill.exe"
Check ((Registry-Default 'HKCU\Software\Classes\.png') -ne 'DuckQuill.Image') "PNG files still open in whatever opened them before"
Check ($null -ne (Get-ItemProperty 'HKCU:\Software\Classes\.png\OpenWithProgids' -Name 'DuckQuill.Image' -ErrorAction SilentlyContinue)) "PNG has Open with -> Duck Quill"
Check (Test-Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$AppId") "Windows Settings -> Apps can uninstall it"
Check (-not (Test-Path "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$AppId")) "nothing written for the whole computer"
Check ((User-Path) -split ';' -contains $App) "the folder is on your account's PATH"
$env:Path = "$(User-Path);$([Environment]::GetEnvironmentVariable('Path', 'Machine'))"
$quill = (& quill --version) 2>&1
Check ($quill -eq "quill $Version") "a new terminal finds quill ($quill)"

Step "Your settings and pictures"
New-Item -ItemType Directory -Force $SettingsFolder, $Pictures, "$Here\server\p" | Out-Null
# A colour only you would have chosen, to see it survive.
Set-Content "$SettingsFolder\settings.json" '{"color": [12, 34, 56]}'
Copy-Item "$Here\crates\engine\tests\fixtures\picture-0.5.0.quill" "$Pictures\pond.quill"
Copy-Item "$Here\crates\engine\tests\fixtures\plain-0.6.0.quill" "$Pictures\duck.quill"
@"
[
 {"tag_name": "v99.0.0", "draft": true, "prerelease": false, "assets": []},
 {"tag_name": "v98.0.0", "draft": false, "prerelease": true, "assets": []},
 {"tag_name": "v0.0.1", "draft": false, "prerelease": false, "assets": []},
 {"tag_name": "v$Version", "draft": false, "prerelease": false, "html_url": "http://127.0.0.1:8765/page",
  "assets": [
   {"name": "DuckQuill-$Version-setup.exe", "browser_download_url": "http://127.0.0.1:8765/p/DuckQuill-$Version-setup.exe"},
   {"name": "SHA256SUMS.txt", "browser_download_url": "http://127.0.0.1:8765/p/SHA256SUMS.txt"}]}
]
"@ | Set-Content "$Here\server\releases"
Serve $Setup ('0' * 64 + "  DuckQuill-$Version-setup.exe`n")
$server = Start-Process python -ArgumentList '-m', 'http.server', '8765', '--bind', '127.0.0.1', '--directory', "`"$Here\server`"" -PassThru -WindowStyle Hidden -RedirectStandardError "$Logs\server.log"
Start-Sleep -Seconds 2

Step "Double-click a picture"
Start-Process "$Pictures\pond.quill"
Check (Wait-For "a window" { (Windows-Of-App).Count -ge 1 } 60) "Duck Quill opens"
Check (Wait-For "the picture" { (Title) -like '*pond*' } 30) "with the picture ($(Title))"
Shot "1-opened-by-double-click"
Start-Process "$Pictures\duck.quill"
Check (Wait-For "the second picture" { (Title) -like '*duck*' } 30) "a second double-click opens in the same window ($(Title))"
Start-Sleep -Seconds 3
Check (@(Get-Process DuckQuill -ErrorAction SilentlyContinue).Count -eq 1) "and no second Duck Quill is left running"

Step "Update 1: the download doesn't match the published fingerprint"
Ask-For-Update
Check (Wait-For "the answer" { (Test-Log) -match 'refused:' }) "the update is refused"
Write-Host ((Test-Log) -split "`n" | Select-String 'refused:')
Check (-not (Test-Path "$U\DuckQuill-$Version-setup.exe")) "the download is deleted"
Check (-not (Test-Path "$U\old version")) "nothing was installed"
Check ((Installed-Version) -eq '0.0.1') "0.0.1 is still the version installed"
Check ((Windows-Of-App).Count -eq 1) "Duck Quill is still open"
Start-Sleep -Seconds 1
Shot "2-banner-update-refused"

Step "Update 2: the setup dies halfway"
$before = Folder-Hashes
Serve $BrokenSetup (Sums-Of $BrokenSetup)
Ask-For-Update
Check (Wait-For "the updater" { (Test-Log) -match 'updater:' } 300) "the updater ran"
$said = ((Test-Log) -split "`n" | Select-String 'updater:' | Select-Object -Last 1).ToString()
Write-Host "  $said"
Check ($said -match 'put back') "it says the version you had was put back"
Check (Wait-For "Duck Quill again" { (Windows-Of-App).Count -ge 1 } 60) "Duck Quill opens again"
Check ((Installed-Version) -eq '0.0.1') "0.0.1 is the version installed"
$after = Folder-Hashes
Check ($before -eq $after) "every program file is exactly as before (none added, none changed)"
if ($before -ne $after) { Compare-Object ($before -split "`n") ($after -split "`n") | Out-String | Write-Host }
Check (Wait-For "the picture" { (Title) -like '*duck*' } 30) "with the same picture ($(Title))"
Shot "3-after-failed-update"

Step "Update 3: a good setup"
Serve $Setup (Sums-Of $Setup)
Ask-For-Update
Check (Wait-For "the updater" { ((Test-Log) -split "`n" | Select-String 'updater:').Count -ge 2 } 300) "the updater ran"
$said = ((Test-Log) -split "`n" | Select-String 'updater:' | Select-Object -Last 1).ToString()
Write-Host "  $said"
Check ($said -match "updated to $Version") "it says Duck Quill was updated"
Check (Wait-For "Duck Quill again" { (Windows-Of-App).Count -ge 1 } 60) "Duck Quill opens again"
Check ((Installed-Version) -eq $Version) "$Version is the version installed"
Check ((Get-Content "$App\installed.ini" -Raw) -match [regex]::Escape($Version)) "installed.ini says $Version"
Check (Wait-For "the picture" { (Title) -like '*duck*' } 30) "with the same picture ($(Title))"
Check ((Get-Content "$SettingsFolder\settings.json" -Raw) -replace '\s', '' -match '"color":\[12,34,56\]') "your settings are kept"
Check ((User-Path) -split ';' -contains $App) "quill is still on the PATH"
Check ((Registry-Default 'HKCU\Software\Classes\.quill') -eq 'DuckQuill.Picture') ".quill still opens in Duck Quill"
Check (Test-Path $StartMenu) "still in the Start menu"
Start-Sleep -Seconds 2
Shot "4-after-update"
Get-Content "$U\test-log.txt" | Out-File "$Logs\update-test-log.txt"

Step "Uninstall"
Get-Process DuckQuill -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2
Start-Process "$App\unins000.exe" -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART' -Wait
Check (Wait-For "the files to go" { -not (Test-Path "$App\DuckQuill.exe") } 60) "the program files are gone"
Check (-not (Test-Path $StartMenu)) "the Start menu entry is gone"
Check ($null -eq (Registry-Default 'HKCU\Software\Classes\.quill')) ".quill isn't Duck Quill's any more"
Check (-not (Test-Path 'HKCU:\Software\Classes\DuckQuill.Picture')) "Duck Quill's file type is gone"
Check ($null -eq (Get-ItemProperty 'HKCU:\Software\Classes\.png\OpenWithProgids' -Name 'DuckQuill.Image' -ErrorAction SilentlyContinue)) "PNG's Open with entry is gone"
Check (-not (Test-Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$AppId")) "it's gone from Windows Settings -> Apps"
Check (-not ((User-Path) -split ';' -contains $App)) "the PATH no longer has the folder"
Check ((Get-Content "$SettingsFolder\settings.json" -Raw) -replace '\s', '' -match '"color":\[12,34,56\]') "your settings are still there"
Check ((Test-Path "$Pictures\pond.quill") -and (Test-Path "$Pictures\duck.quill")) "your pictures are still there"

Step "Install by hand, as you would (screenshots of each page)"
Stop-Process $server -ErrorAction SilentlyContinue
Start-Process $Setup
for ($page = 1; $page -le 4; $page++) {
  Start-Sleep -Seconds 4
  Shot "5-setup-page-$page"
  Send-Keys '{ENTER}' 'Setup - Duck Quill'
}
Check (Wait-For "Duck Quill" { (Windows-Of-App).Count -ge 1 } 60) "Launch Duck Quill on the last page opens it"
Check ((Installed-Version) -eq $Version) "$Version is installed"
Start-Sleep -Seconds 5
Shot "6-home"
Send-Keys '^k' (Title)
Start-Sleep -Seconds 1
Send-Keys 'onion' ''
Start-Sleep -Seconds 2
Shot "7-command-search"

Step "Done"
Get-Process DuckQuill -ErrorAction SilentlyContinue | Stop-Process -Force
if ($script:failures.Count -gt 0) {
  Write-Host "$($script:failures.Count) checks failed:"
  $script:failures | ForEach-Object { Write-Host "  - $_" }
  exit 1
}
Write-Host "Every check passed."
