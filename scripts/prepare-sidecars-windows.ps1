param(
  [string]$TargetTriple = "x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"

$RootDir = Split-Path -Parent $PSScriptRoot
$BinDir = Join-Path $RootDir "src-tauri/binaries"
$CacheDir = Join-Path $RootDir ".cache"
$WhisperDir = if ($env:WHISPER_CPP_DIR) { $env:WHISPER_CPP_DIR } else { Join-Path $CacheDir "whisper.cpp" }
$WhisperRef = if ($env:WHISPER_CPP_REF) { $env:WHISPER_CPP_REF } else { "master" }
$FfmpegSource = if ($env:FFMPEG_SOURCE) { $env:FFMPEG_SOURCE } elseif ($env:FFMPEG_BIN) { $env:FFMPEG_BIN } else { "" }
$WhisperCliSource = if ($env:WHISPER_CLI_SOURCE) { $env:WHISPER_CLI_SOURCE } elseif ($env:WHISPER_CLI_BIN) { $env:WHISPER_CLI_BIN } else { "" }
$YtDlpSource = if ($env:YT_DLP_SOURCE) { $env:YT_DLP_SOURCE } elseif ($env:YT_DLP_BIN) { $env:YT_DLP_BIN } else { "" }
$YtDlpDownloadUrl = if ($env:YT_DLP_DOWNLOAD_URL) { $env:YT_DLP_DOWNLOAD_URL } else { "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe" }
$WhisperReleaseApiUrl = if ($env:WHISPER_RELEASE_API_URL) { $env:WHISPER_RELEASE_API_URL } else { "https://api.github.com/repos/ggml-org/whisper.cpp/releases/latest" }

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
  if ($FfmpegSource) {
    if (-not (Test-Path $FfmpegSource)) {
      throw "FFMPEG_SOURCE does not exist: $FfmpegSource"
    }

    return (Resolve-Path $FfmpegSource).Path
  }

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

function Install-YtDlp {
  param([string]$TargetPath)

  Write-Host "Downloading yt-dlp from $YtDlpDownloadUrl"
  Invoke-WebRequest -Uri $YtDlpDownloadUrl -OutFile $TargetPath
}

function Test-PortableExecutable {
  param([string]$Path)

  if (-not (Test-Path $Path)) {
    return $false
  }

  $stream = [System.IO.File]::OpenRead($Path)
  try {
    if ($stream.Length -lt 2) {
      return $false
    }

    $header = New-Object byte[] 2
    $null = $stream.Read($header, 0, 2)
    return $header[0] -eq 0x4D -and $header[1] -eq 0x5A
  }
  finally {
    $stream.Dispose()
  }
}

function Resolve-YtDlpSource {
  if (-not $YtDlpSource) {
    return $null
  }

  if (-not (Test-Path $YtDlpSource)) {
    throw "YT_DLP_SOURCE does not exist: $YtDlpSource"
  }

  if (-not (Test-PortableExecutable $YtDlpSource)) {
    throw "YT_DLP_SOURCE must point to a standalone yt-dlp .exe, not a wrapper script or shim."
  }

  return (Resolve-Path $YtDlpSource).Path
}

function Resolve-WhisperCliSource {
  if (-not $WhisperCliSource) {
    $command = Get-Command whisper-cli -ErrorAction SilentlyContinue
    if ($command -and (Test-PortableExecutable $command.Source)) {
      return $command.Source
    }

    return $null
  }

  if (-not (Test-Path $WhisperCliSource)) {
    throw "WHISPER_CLI_SOURCE does not exist: $WhisperCliSource"
  }

  if (-not (Test-PortableExecutable $WhisperCliSource)) {
    throw "WHISPER_CLI_SOURCE must point to a native whisper-cli .exe."
  }

  return (Resolve-Path $WhisperCliSource).Path
}

function Test-CommandAvailable {
  param([string]$Name)

  return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Install-WhisperCliFromRelease {
  param(
    [string]$TargetPath,
    [string]$TargetTriple,
    [string]$BinDir,
    [string]$CacheDir
  )

  $assetName = switch ($TargetTriple) {
    "x86_64-pc-windows-msvc" { "whisper-bin-x64.zip" }
    "i686-pc-windows-msvc" { "whisper-bin-Win32.zip" }
    default { $null }
  }

  if (-not $assetName) {
    return $false
  }

  Write-Host "Resolving whisper.cpp release asset: $assetName"
  $release = Invoke-RestMethod -Uri $WhisperReleaseApiUrl
  $asset = $release.assets | Where-Object { $_.name -eq $assetName } | Select-Object -First 1
  if (-not $asset) {
    throw "Cannot find whisper.cpp release asset: $assetName"
  }

  $zipPath = Join-Path $CacheDir $asset.name
  $extractDir = Join-Path $CacheDir ("whisper-release-" + [System.IO.Path]::GetFileNameWithoutExtension($asset.name))

  Write-Host "Downloading whisper.cpp prebuilt package from $($asset.browser_download_url)"
  Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zipPath

  if (Test-Path $extractDir) {
    Remove-Item $extractDir -Recurse -Force
  }
  Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force

  $releaseDir = Join-Path $extractDir "Release"
  $whisperBuilt = Join-Path $releaseDir "whisper-cli.exe"
  if (-not (Test-Path $whisperBuilt)) {
    throw "Cannot find whisper-cli.exe in downloaded release package."
  }

  Copy-Item $whisperBuilt $TargetPath -Force

  $dllNames = @(
    "ggml-base.dll",
    "ggml-cpu.dll",
    "ggml.dll",
    "whisper.dll"
  )

  foreach ($dllName in $dllNames) {
    $dllSource = Join-Path $releaseDir $dllName
    if (Test-Path $dllSource) {
      Copy-Item $dllSource (Join-Path $BinDir $dllName) -Force
    }
  }

  return $true
}

function Get-ExecutableExitCode {
  param(
    [string]$FilePath,
    [string[]]$Arguments
  )

  $process = Start-Process -FilePath $FilePath -ArgumentList $Arguments -NoNewWindow -Wait -PassThru
  return $process.ExitCode
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

$ytDlp = Resolve-YtDlpSource
if (-not $ytDlp) {
  Install-YtDlp -TargetPath $YtDlpTargetPath
  $ytDlp = $YtDlpTargetPath
}

if (-not $ytDlp) {
  throw "Cannot locate yt-dlp after installation."
}

if (-not (Test-PortableExecutable $ytDlp)) {
  throw "yt-dlp binary is not a standalone Windows executable: $ytDlp"
}

if ((Resolve-Path $ytDlp).Path -ne (Resolve-Path $YtDlpTargetPath -ErrorAction SilentlyContinue | ForEach-Object { $_.Path })) {
  Copy-Item $ytDlp $YtDlpTargetPath -Force
}

if ($resolvedWhisper = Resolve-WhisperCliSource) {
  Copy-Item $resolvedWhisper $WhisperTargetPath -Force
} else {
  try {
    if (Install-WhisperCliFromRelease -TargetPath $WhisperTargetPath -TargetTriple $TargetTriple -BinDir $BinDir -CacheDir $CacheDir) {
      Write-Host "Prepared whisper sidecar from prebuilt release: $WhisperTargetPath"
    }
  }
  catch {
    Write-Warning "Failed to download prebuilt whisper-cli: $($_.Exception.Message)"
  }

  if (-not (Test-Path $WhisperTargetPath)) {
    if (-not (Test-CommandAvailable "git")) {
      throw "git is required to build whisper-cli automatically. Install Git or set WHISPER_CLI_BIN / WHISPER_CLI_SOURCE to an existing whisper-cli.exe."
    }

    if (-not (Test-CommandAvailable "cmake")) {
      throw "cmake is required to build whisper-cli automatically. Install CMake from https://cmake.org/download/ or set WHISPER_CLI_BIN / WHISPER_CLI_SOURCE to an existing whisper-cli.exe."
    }

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
  }
}

$whisperExitCode = Get-ExecutableExitCode -FilePath $WhisperTargetPath -Arguments @("--help")
if ($whisperExitCode -notin @(0, 1)) {
  throw "whisper-cli verification failed with exit code $whisperExitCode."
}

$ffmpegExitCode = Get-ExecutableExitCode -FilePath $FfmpegTargetPath -Arguments @("-version")
if ($ffmpegExitCode -ne 0) {
  throw "ffmpeg verification failed with exit code $ffmpegExitCode."
}

$ytDlpExitCode = Get-ExecutableExitCode -FilePath $YtDlpTargetPath -Arguments @("--help")
if ($ytDlpExitCode -ne 0) {
  throw "yt-dlp verification failed with exit code $ytDlpExitCode."
}

Write-Host "Prepared whisper sidecar: $WhisperTargetPath"
Write-Host "Prepared ffmpeg sidecar: $FfmpegTargetPath"
Write-Host "Prepared yt-dlp sidecar: $YtDlpTargetPath"
