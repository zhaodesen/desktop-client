import { readdirSync, rmSync, statSync } from 'node:fs';
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

function postprocessMacBuild() {
  if (process.platform !== 'darwin') {
    return;
  }

  if (args[0] !== 'build') {
    return;
  }

  const profileDir = args.includes('--debug') ? 'debug' : 'release';
  const targetIndex = args.findIndex((value) => value === '--target');
  const targetTriple = targetIndex >= 0 ? args[targetIndex + 1] : null;
  const targetBaseDir = targetTriple
    ? path.join(rootDir, 'src-tauri', 'target', targetTriple, profileDir)
    : path.join(rootDir, 'src-tauri', 'target', profileDir);
  const bundleDir = path.join(targetBaseDir, 'bundle');
  const macosDir = path.join(bundleDir, 'macos');
  const dmgDir = path.join(bundleDir, 'dmg');
  const entitlementsPath = path.join(rootDir, 'src-tauri', 'entitlements', 'whisper-cli.plist');

  const appName = readdirSync(macosDir).find((entry) => entry.endsWith('.app'));
  if (!appName) {
    return;
  }

  const appPath = path.join(macosDir, appName);
  const sidecarPaths = [
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

  const dmgNames = readdirSync(dmgDir).filter((entry) => entry.endsWith('.dmg'));
  for (const dmgName of dmgNames) {
    const dmgPath = path.join(dmgDir, dmgName);
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
run(tauriCommand, args);
postprocessMacBuild();
