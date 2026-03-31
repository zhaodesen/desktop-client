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

function walkFiles(dir, collector) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walkFiles(fullPath, collector);
      continue;
    }
    if (entry.isFile()) {
      collector(fullPath);
    }
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

function findFirstExisting(paths) {
  for (const candidate of paths) {
    if (statSafe(candidate)?.isFile()) {
      return candidate;
    }
  }
  return null;
}

function postprocessMacBuild() {
  if (process.platform !== 'darwin') {
    return;
  }

  if (args[0] !== 'build') {
    return;
  }

  const profileDir = args.includes('--debug') ? 'debug' : 'release';
  const bundleDir = path.join(rootDir, 'src-tauri', 'target', profileDir, 'bundle');
  const macosDir = path.join(bundleDir, 'macos');
  const dmgDir = path.join(bundleDir, 'dmg');
  const entitlementsPath = path.join(rootDir, 'src-tauri', 'entitlements', 'whisper-cli.plist');

  const appName = readdirSync(macosDir).find((entry) => entry.endsWith('.app'));
  if (!appName) {
    return;
  }

  const appPath = path.join(macosDir, appName);
  const whisperCliPath = path.join(appPath, 'Contents', 'MacOS', 'whisper-cli');
  const ytDlpPath = path.join(appPath, 'Contents', 'MacOS', 'yt-dlp');
  const translatorDir = path.join(appPath, 'Contents', 'Resources', '_up_', 'scripts', 'offline_translator');
  const translatorPythonPath = findFirstExisting([
    path.join(translatorDir, '.python-home', 'bin', 'python3.13'),
    path.join(translatorDir, '.python-home', 'bin', 'python3'),
    path.join(translatorDir, '.python-home', 'bin', 'python'),
  ]);

  for (const sidecarPath of [whisperCliPath, ytDlpPath]) {
    signMacBinary(sidecarPath, entitlementsPath);
  }

  if (translatorPythonPath) {
    const translatorLibraries = [];
    walkFiles(translatorDir, (filePath) => {
      if (
        filePath.endsWith('.dylib')
        || filePath.endsWith('.so')
      ) {
        translatorLibraries.push(filePath);
      }
    });
    for (const libraryPath of translatorLibraries) {
      signMacBinary(libraryPath);
    }
    signMacBinary(translatorPythonPath, entitlementsPath);
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
