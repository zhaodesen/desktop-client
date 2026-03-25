param(
  [string]$TargetTriple = "x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"

$RootDir = Split-Path -Parent $PSScriptRoot
$BinDir = Join-Path $RootDir "src-tauri/binaries"
$CacheDir = Join-Path $RootDir ".cache"
$WhisperDir = if ($env:WHISPER_CPP_DIR) { $env:WHISPER_CPP_DIR } else { Join-Path $CacheDir "whisper.cpp" }
$WhisperRef = if ($env:WHISPER_CPP_REF) { $env:WHISPER_CPP_REF } else { "master" }
$YtDlpSource = if ($env:YT_DLP_SOURCE) { $env:YT_DLP_SOURCE } else { "" }

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
New-Item -ItemType Directory -Force -Path $CacheDir | Out-Null

function Resolve-TargetName {
  param([string]$Name)
  return "$Name-$TargetTriple.exe"
}

$FfmpegTargetPath = Join-Path $BinDir (Resolve-TargetName "ffmpeg")
$WhisperTargetPath = Join-Path $BinDir (Resolve-TargetName "whisper-cli")
$YtDlpTargetPath = Join-Path $BinDir (Resolve-TargetName "yt-dlp")

function Find-Ffmpeg {
  $candidates = @(
    "C:\ProgramData\chocolatey\lib\ffmpeg\tools\ffmpeg\bin\ffmpeg.exe",
    "C:\tools\ffmpeg\bin\ffmpeg.exe",
    "C:\ProgramData\chocolatey\bin\ffmpeg.exe"
  )

  foreach ($candidate in $candidates) {
    if (Test-Path $candidate) {
      return $candidate
    }
  }

  $command = Get-Command ffmpeg -ErrorAction SilentlyContinue
  if ($command) {
    return $command.Source
  }

  return $null
}

function Find-YtDlp {
  $candidates = @(
    $YtDlpSource,
    "C:\ProgramData\chocolatey\bin\yt-dlp.exe",
    "C:\ProgramData\chocolatey\lib\yt-dlp\tools\yt-dlp.exe"
  ) | Where-Object { $_ }

  foreach ($candidate in $candidates) {
    if (Test-Path $candidate) {
      return $candidate
    }
  }

  $command = Get-Command yt-dlp -ErrorAction SilentlyContinue
  if ($command) {
    return $command.Source
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

$ytDlp = Find-YtDlp
if (-not $ytDlp) {
  choco install yt-dlp --yes
  $env:PATH = "C:\ProgramData\chocolatey\bin;C:\tools\ffmpeg\bin;$env:PATH"
  $ytDlp = Find-YtDlp
}

if (-not $ytDlp) {
  throw "Cannot locate yt-dlp after Chocolatey installation."
}

Copy-Item $ytDlp $YtDlpTargetPath -Force

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
cmake -B build -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF
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

& $YtDlpTargetPath --help *> $null
if ($LASTEXITCODE -ne 0) {
  throw "yt-dlp verification failed with exit code $LASTEXITCODE."
}

Write-Host "Prepared whisper sidecar: $WhisperTargetPath"
Write-Host "Prepared ffmpeg sidecar: $FfmpegTargetPath"
Write-Host "Prepared yt-dlp sidecar: $YtDlpTargetPath"
