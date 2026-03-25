param(
  [string]$TargetTriple = "x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"

$RootDir = Split-Path -Parent $PSScriptRoot
$BinDir = Join-Path $RootDir "src-tauri/binaries"
$CacheDir = Join-Path $RootDir ".cache"
$WhisperDir = if ($env:WHISPER_CPP_DIR) { $env:WHISPER_CPP_DIR } else { Join-Path $CacheDir "whisper.cpp" }
$WhisperRef = if ($env:WHISPER_CPP_REF) { $env:WHISPER_CPP_REF } else { "master" }

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
New-Item -ItemType Directory -Force -Path $CacheDir | Out-Null

function Resolve-TargetName {
  param([string]$Name)
  return "$Name-$TargetTriple.exe"
}

$FfmpegTargetPath = Join-Path $BinDir (Resolve-TargetName "ffmpeg")
$WhisperTargetPath = Join-Path $BinDir (Resolve-TargetName "whisper-cli")

function Find-Ffmpeg {
  $command = Get-Command ffmpeg -ErrorAction SilentlyContinue
  if ($command) {
    return $command.Source
  }

  $candidates = @(
    "C:\tools\ffmpeg\bin\ffmpeg.exe",
    "C:\ProgramData\chocolatey\bin\ffmpeg.exe"
  )

  foreach ($candidate in $candidates) {
    if (Test-Path $candidate) {
      return $candidate
    }
  }

  return $null
}

$ffmpeg = Find-Ffmpeg
if (-not $ffmpeg) {
  choco install ffmpeg --yes
  $env:PATH = "C:\ProgramData\chocolatey\bin;C:\tools\ffmpeg\bin;$env:PATH"
  $ffmpeg = Find-Ffmpeg
}

if (-not $ffmpeg) {
  throw "Cannot locate ffmpeg after Chocolatey installation."
}

Copy-Item $ffmpeg $FfmpegTargetPath -Force

if (Test-Path (Join-Path $WhisperDir ".git")) {
  git -C $WhisperDir fetch --depth 1 origin $WhisperRef
  git -C $WhisperDir checkout -f FETCH_HEAD
} else {
  if (Test-Path $WhisperDir) {
    Remove-Item $WhisperDir -Recurse -Force
  }
  git clone --depth 1 --branch $WhisperRef https://github.com/ggml-org/whisper.cpp.git $WhisperDir
}

Push-Location $WhisperDir
if (Test-Path build) {
  Remove-Item build -Recurse -Force
}
cmake -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release
Pop-Location

$whisperCandidates = @(
  (Join-Path $WhisperDir "build/bin/Release/whisper-cli.exe"),
  (Join-Path $WhisperDir "build/bin/whisper-cli.exe")
)

$whisperBuilt = $whisperCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $whisperBuilt) {
  throw "Cannot find built whisper-cli.exe."
}

Copy-Item $whisperBuilt $WhisperTargetPath -Force

& $WhisperTargetPath --help *> $null
if ($LASTEXITCODE -notin @(0, 1)) {
  throw "whisper-cli verification failed with exit code $LASTEXITCODE."
}

& $FfmpegTargetPath -version *> $null
if ($LASTEXITCODE -ne 0) {
  throw "ffmpeg verification failed with exit code $LASTEXITCODE."
}

Write-Host "Prepared whisper sidecar: $WhisperTargetPath"
Write-Host "Prepared ffmpeg sidecar: $FfmpegTargetPath"
