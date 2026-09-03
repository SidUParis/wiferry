$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
if (-not (Test-Path ".runtime-venv\Scripts\python.exe")) {
  python -m venv .runtime-venv
}
.runtime-venv\Scripts\python.exe -m pip install --quiet --upgrade .
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& .runtime-venv\Scripts\wiferry.exe @args
exit $LASTEXITCODE
