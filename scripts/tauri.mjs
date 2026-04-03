import { existsSync, readdirSync, rmSync, statSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');
const args = process.argv.slice(2);

function run(command, commandArgs) {
  const isWindowsCmdScript = process.platform === 'win32' && command.toLowerCase().endsWith('.cmd');
  const spawnCommand = isWindowsCmdScript ? process.env.ComSpec || 'cmd.exe' : command;
  const spawnArgs = isWindowsCmdScript ? ['/d', '/s', '/c', command, ...commandArgs] : commandArgs;

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

function signMacBinary(filePath, entitlementsPath) {
  const args = ['--force', '--sign', '-'];
  if (entitlementsPath) {
    args.push('--options', 'runtime', '--entitlements', entitlementsPath);
  }
  args.push(filePath);
  run('codesign', args);
}

function resolveMacBuildContext() {
  if (process.platform !== 'darwin') {
    return null;
  }

  if (args[0] !== 'build' || args.includes('--no-bundle')) {
    return null;
  }

  const profileDir = args.includes('--debug') ? 'debug' : 'release';
  const targetIndex = args.findIndex((value) => value === '--target');
  const targetTriple = targetIndex >= 0 ? args[targetIndex + 1] : null;
  const targetBaseDir = targetTriple
    ? path.join(rootDir, 'src-tauri', 'target', targetTriple, profileDir)
    : path.join(rootDir, 'src-tauri', 'target', profileDir);
  const bundleDir = path.join(targetBaseDir, 'bundle');

  return {
    bundleDir,
    dmgDir: path.join(bundleDir, 'dmg'),
    macosDir: path.join(bundleDir, 'macos'),
  };
}

function prepareMacBuild() {
  const context = resolveMacBuildContext();
  if (!context) {
    return;
  }

  // 删除旧 bundle 产物，避免 postprocess 误拿到历史 .app / .dmg。
  rmSync(context.macosDir, { force: true, recursive: true });
  rmSync(context.dmgDir, { force: true, recursive: true });
}

function postprocessMacBuild() {
  const context = resolveMacBuildContext();
  if (!context) {
    return;
  }

  const entitlementsPath = path.join(rootDir, 'src-tauri', 'entitlements', 'whisper-cli.plist');

  if (!existsSync(context.macosDir)) {
    return;
  }

  const appNames = readdirSync(context.macosDir).filter((entry) => entry.endsWith('.app'));
  if (appNames.length === 0) {
    return;
  }
  if (appNames.length > 1) {
    throw new Error(`检测到多个 macOS App 产物，无法确定当前构建结果：${appNames.join(', ')}`);
  }

  const [appName] = appNames;
  const appPath = path.join(context.macosDir, appName);
  const sidecarPaths = [
    path.join(appPath, 'Contents', 'MacOS', 'ffmpeg'),
    path.join(appPath, 'Contents', 'MacOS', 'whisper-cli'),
    path.join(appPath, 'Contents', 'MacOS', 'yt-dlp'),
    path.join(appPath, 'Contents', 'MacOS', 'translator-cli'),
    path.join(appPath, 'Contents', 'MacOS', 'ct2-translator'),
    path.join(appPath, 'Contents', 'MacOS', 'spm_encode'),
    path.join(appPath, 'Contents', 'MacOS', 'spm_decode'),
  ].filter((candidate) => statSafe(candidate)?.isFile());

  for (const sidecarPath of sidecarPaths) {
    signMacBinary(sidecarPath, entitlementsPath);
  }
  run('codesign', ['--force', '--sign', '-', appPath]);

  if (!existsSync(context.dmgDir)) {
    return;
  }

  const dmgNames = readdirSync(context.dmgDir).filter((entry) => entry.endsWith('.dmg'));
  for (const dmgName of dmgNames) {
    const dmgPath = path.join(context.dmgDir, dmgName);
    rmSync(dmgPath, { force: true });
    run('hdiutil', [
      'create',
      '-volname',
      path.basename(appName, '.app'),
      '-srcfolder',
      appPath,
      '-ov',
      '-format',
      'UDZO',
      dmgPath,
    ]);
  }
}

function statSafe(filePath) {
  try {
    return statSync(filePath);
  } catch {
    return null;
  }
}

const tauriCommand = process.platform === 'win32' ? 'tauri.cmd' : 'tauri';
prepareMacBuild();
run(tauriCommand, args);
postprocessMacBuild();
