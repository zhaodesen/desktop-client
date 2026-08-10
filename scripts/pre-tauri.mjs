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

function verifyBundledSidecars(targetTriple) {
  const expected = ['ffmpeg', 'whisper-cli', 'translator-cli', 'ct2-translator', 'spm_encode', 'spm_decode'].map((name) =>
    path.join(binariesDir, targetSidecarName(name, targetTriple)),
  );
  if (targetTriple.includes('windows')) {
    expected.push(
      path.join(binariesDir, 'ggml-base.dll'),
      path.join(binariesDir, 'ggml-cpu.dll'),
      path.join(binariesDir, 'ggml.dll'),
      path.join(binariesDir, 'whisper.dll'),
    );
  }
  if (targetTriple.includes('apple-darwin')) {
    expected.push(
      path.join(binariesDir, 'libwhisper.1.dylib'),
      path.join(binariesDir, 'libggml.0.dylib'),
      path.join(binariesDir, 'libggml-base.0.dylib'),
      path.join(binariesDir, 'libggml-cpu.0.dylib'),
      path.join(binariesDir, 'libggml-blas.0.dylib'),
      path.join(binariesDir, 'libggml-metal.0.dylib'),
    );
  }

  const missing = expected.filter((candidate) => !existsSync(candidate));
  if (missing.length === 0) {
    return;
  }

  console.error('[sidecar] 检测到缺失的预置 sidecar：');
  for (const candidate of missing) {
    console.error(`- ${path.relative(rootDir, candidate)}`);
  }
  console.error('请先准备原生 sidecar。翻译链路现在额外要求 translator-cli / ct2-translator / spm_encode / spm_decode。');
  process.exit(1);
}

function signMacDynamicLibraries(targetTriple) {
  if (!targetTriple.includes('apple-darwin')) {
    return;
  }

  const dylibs = [
    'libwhisper.1.dylib',
    'libggml.0.dylib',
    'libggml-base.0.dylib',
    'libggml-cpu.0.dylib',
    'libggml-blas.0.dylib',
    'libggml-metal.0.dylib',
  ].map((name) => path.join(binariesDir, name));

  for (const dylib of dylibs) {
    if (!existsSync(dylib)) continue;
    const result = spawnSync('codesign', ['--force', '--sign', '-', dylib], {
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
}

const targetTriple =
  process.env.TAURI_ENV_TARGET_TRIPLE || process.env.TARGET_TRIPLE || detectTargetTriple();
if (!targetTriple) {
  console.error('无法确定当前 Rust target triple。');
  process.exit(1);
}

verifyBundledSidecars(targetTriple);
signMacDynamicLibraries(targetTriple);

const npmCommand = process.platform === 'win32' ? 'npm.cmd' : 'npm';
runCommand(npmCommand, ['run', mode]);
