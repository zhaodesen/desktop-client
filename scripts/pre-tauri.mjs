import { execFileSync, spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');
const binariesDir = path.join(rootDir, 'src-tauri', 'binaries');
const mode = process.argv[2] === 'dev' ? 'dev' : 'build';

function detectTargetTriple() {
  try {
    return execFileSync('rustc', ['--print', 'host-tuple'], {
      cwd: rootDir,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim();
  } catch {
    return execFileSync('rustc', ['-vV'], {
      cwd: rootDir,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    })
      .split('\n')
      .map((line) => line.trim())
      .find((line) => line.startsWith('host:'))
      ?.slice('host:'.length)
      .trim();
  }
}

function targetSidecarName(name, targetTriple) {
  const suffix = targetTriple.includes('windows') ? '.exe' : '';
  return `${name}-${targetTriple}${suffix}`;
}

function runCommand(command, args) {
  const isWindowsCmdScript = process.platform === 'win32' && command.toLowerCase().endsWith('.cmd');
  const spawnCommand = isWindowsCmdScript ? process.env.ComSpec || 'cmd.exe' : command;
  const spawnArgs = isWindowsCmdScript ? ['/d', '/s', '/c', command, ...args] : args;

  const result = spawnSync(spawnCommand, spawnArgs, {
    cwd: rootDir,
    env: process.env,
    stdio: 'inherit',
  });

  if (result.error) {
    throw result.error;
  }

  if (typeof result.status === 'number' && result.status !== 0) {
    process.exit(result.status);
  }
}

function ensureWindowsSidecars(targetTriple) {
  const expected = ['ffmpeg', 'whisper-cli', 'yt-dlp'].map((name) =>
    path.join(binariesDir, targetSidecarName(name, targetTriple)),
  );
  const missing = expected.filter((candidate) => !existsSync(candidate));

  if (missing.length === 0) {
    return;
  }

  console.log('[sidecar] 检测到 Windows sidecar 缺失，开始自动准备：');
  for (const candidate of missing) {
    console.log(`- ${path.relative(rootDir, candidate)}`);
  }

  const scriptPath = path.join(rootDir, 'scripts', 'prepare-sidecars-windows.ps1');
  const powershellCandidates = ['pwsh', 'powershell'];
  let launched = false;

  for (const shell of powershellCandidates) {
    const result = spawnSync(
      shell,
      ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', scriptPath, '-TargetTriple', targetTriple],
      {
        cwd: rootDir,
        env: process.env,
        stdio: 'inherit',
      },
    );

    if (result.error) {
      continue;
    }

    launched = true;
    if (typeof result.status === 'number' && result.status !== 0) {
      process.exit(result.status);
    }
    break;
  }

  if (!launched) {
    console.error('未找到可用的 PowerShell，可手动执行 scripts/prepare-sidecars-windows.ps1。');
    process.exit(1);
  }
}

const targetTriple = process.env.TARGET_TRIPLE || detectTargetTriple();
if (!targetTriple) {
  console.error('无法确定当前 Rust target triple。');
  process.exit(1);
}

if (process.platform === 'win32') {
  ensureWindowsSidecars(targetTriple);
}

const npmCommand = process.platform === 'win32' ? 'npm.cmd' : 'npm';
runCommand(npmCommand, ['run', mode]);
