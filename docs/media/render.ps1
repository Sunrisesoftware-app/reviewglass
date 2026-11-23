# Renders docs/media/hero.html into the README's picture and GitHub's social preview,
# with Microsoft Edge in headless mode. Run from anywhere:  pwsh docs/media/render.ps1
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$edge = @(
  "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
  "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $edge) { throw 'Microsoft Edge was not found' }

$page = ([System.Uri](Join-Path $here 'hero.html')).AbsoluteUri
$profile = Join-Path ([System.IO.Path]::GetTempPath()) 'reviewglass-render'

function Render-Page($url, $out, $scale) {
  & $edge --headless=new --disable-gpu --hide-scrollbars --no-first-run `
    "--user-data-dir=$profile" --window-size=1600,800 "--force-device-scale-factor=$scale" `
    "--screenshot=$out" $url 2>$null | Out-Null
  Write-Output ("{0}  {1:N0} bytes" -f $out, (Get-Item $out).Length)
}

# The README's picture at 1.5x for sharp text; the social preview at GitHub's 1280x640.
Render-Page $page (Join-Path $here 'reviewglass-hero.png') 1.5
Render-Page "$page`?social" (Join-Path $here 'social-preview.png') 0.8
