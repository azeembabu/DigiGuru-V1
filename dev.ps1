#requires -Version 5.1
<#
  Digi Guru -- one-command local dev.

    ./dev.ps1              # deps + migrations + gateway + web
    ./dev.ps1 -WebOnly     # just the Next.js app (no Docker, no Rust)
    ./dev.ps1 -NoDeps      # assume Postgres/Redis/Qdrant are already up

  Everything runs in parallel: Docker deps come up with --wait, migrations run
  once they are healthy, then the gateway and the web dev server start together
  and stream into this window. Ctrl-C stops both; the containers stay up (they
  are the slow part to recreate -- `docker compose -f infra/docker-compose.yml down`
  when you actually want them gone).
#>
[CmdletBinding()]
param(
  [switch]$WebOnly,
  [switch]$NoDeps,
  [switch]$SkipMigrations
)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
Set-Location $root

function Info($m) { Write-Host "==> $m" -ForegroundColor Cyan }
function Warn($m) { Write-Host "==> $m" -ForegroundColor Yellow }

# cargo/sqlx are installed per-user and are often missing from a non-login PATH.
$cargoBin = Join-Path $HOME '.cargo\bin'
if ((Test-Path $cargoBin) -and ($env:PATH -notlike "*$cargoBin*")) { $env:PATH = "$cargoBin;$env:PATH" }

$have = { param($exe) [bool](Get-Command $exe -ErrorAction SilentlyContinue) }

# `docker info` writes to stderr when the engine is down, which PowerShell turns
# into a terminating NativeCommandError under $ErrorActionPreference='Stop'.
# Probing the engine is expected to fail, so silence the stream and read the code.
function Test-DockerEngine {
  $prev = $ErrorActionPreference
  $ErrorActionPreference = 'Continue'
  try {
    & docker info *> $null
    return ($LASTEXITCODE -eq 0)
  } catch {
    return $false
  } finally {
    $ErrorActionPreference = $prev
  }
}

# --- 1. local dependencies -------------------------------------------------
# Fail loudly and specifically here. A missing Docker used to surface much later
# as an opaque gateway panic on connect, which cost real debugging time -- the
# engine is a hard prerequisite for the backend, so say so up front.
if (-not $WebOnly -and -not $NoDeps) {
  if (-not (& $have 'docker')) {
    $msg = @'
Docker is not installed, so Postgres / Redis / Qdrant cannot start.

  Installer on this machine: E:\DOCKER\Docker Desktop Installer.exe
  Install silently (accept the UAC prompt):
    & "E:\DOCKER\Docker Desktop Installer.exe" install --quiet --accept-license --backend=wsl-2

WSL2 is already present, so no reboot is needed.
Run ./dev.ps1 -WebOnly to work on the frontend alone in the meantime.
'@
    Write-Host $msg -ForegroundColor Red
    exit 1
  }

  # Docker Desktop does not auto-start on login here, so the CLI can exist while
  # the engine is down. Start it and wait rather than letting compose fail.
  if (-not (Test-DockerEngine)) {
    $desktop = 'C:\Program Files\Docker\Docker\Docker Desktop.exe'
    if (Test-Path $desktop) {
      Info 'Docker engine is down -- starting Docker Desktop'
      Start-Process -FilePath $desktop | Out-Null
    }
    Info 'waiting for the Docker engine'
    # First launch after an install provisions the WSL distro, which is slow.
    $deadline = (Get-Date).AddMinutes(5)
    while (-not (Test-DockerEngine)) {
      if ((Get-Date) -gt $deadline) { throw 'Docker engine did not come up within 5 minutes -- open Docker Desktop and check for a first-run prompt' }
      Start-Sleep -Seconds 3
    }
  }

  Info 'starting Postgres / Redis / Qdrant'
  # --wait blocks on the healthchecks in the compose file, so migrations below
  # never race a Postgres that is listening but not yet accepting queries.
  docker compose -f infra/docker-compose.yml up -d --wait
  if ($LASTEXITCODE -ne 0) { throw 'docker compose failed' }
}

# --- 2. migrations ---------------------------------------------------------
if (-not $WebOnly -and -not $SkipMigrations) {
  # sqlx's compile-time query macros need a live, migrated database, so a skipped
  # migration step shows up later as a confusing build failure. Never skip it silently.
  if (-not (& $have 'sqlx')) {
    throw 'sqlx-cli not found. Install it with: cargo install sqlx-cli --no-default-features --features postgres'
  }
  Info 'applying migrations'
  sqlx migrate run
  if ($LASTEXITCODE -ne 0) { throw 'sqlx migrate run failed' }
}

# --- 3. processes ----------------------------------------------------------
$procs = @()

function Start-Dev($name, $file, $argList, $workdir) {
  Info "starting $name"
  $p = Start-Process -FilePath $file -ArgumentList $argList -WorkingDirectory $workdir -NoNewWindow -PassThru
  return [pscustomobject]@{ Name = $name; Proc = $p }
}

if (-not $WebOnly) {
  $procs += Start-Dev 'gateway (http://localhost:8080)' 'cargo' @('run','-p','gateway') $root
}
$npm = if ($IsWindows -or $env:OS -eq 'Windows_NT') { 'npm.cmd' } else { 'npm' }
$procs += Start-Dev 'web (http://localhost:3000)' $npm @('run','dev') (Join-Path $root 'apps\web')

Info 'Ctrl-C to stop. Web: http://localhost:3000'

$stop = $false
try {
  while (-not $stop) {
    foreach ($e in $procs) {
      if ($e.Proc.HasExited) {
        # One process dying takes the stack down, so the window never shows a
        # half-running app. Warn, don't throw: a throw buries the reason under
        # a PowerShell stack trace.
        Warn "$($e.Name) exited with code $($e.Proc.ExitCode) -- shutting the rest down"
        $stop = $true
      }
    }
    if (-not $stop) { Start-Sleep -Milliseconds 500 }
  }
} finally {
  foreach ($e in $procs) {
    if (-not $e.Proc.HasExited) {
      Info "stopping $($e.Name)"
      # Kill the whole tree: `cargo run` and `npm run dev` are both parents.
      & taskkill /PID $e.Proc.Id /T /F 2>$null | Out-Null
    }
  }
}
