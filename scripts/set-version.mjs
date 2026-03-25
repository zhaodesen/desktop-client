#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const rawVersion = process.argv[2]?.trim();

if (!rawVersion) {
  console.error("用法: npm run set-version -- <semver>");
  process.exit(1);
}

const version = rawVersion.startsWith("v") ? rawVersion.slice(1) : rawVersion;

if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error(`无效版本号: ${rawVersion}`);
  process.exit(1);
}

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(scriptDir, "..");
const packageJsonPath = path.join(rootDir, "package.json");
const packageLockPath = path.join(rootDir, "package-lock.json");
const cargoTomlPath = path.join(rootDir, "src-tauri", "Cargo.toml");
const tauriConfigPath = path.join(rootDir, "src-tauri", "tauri.conf.json");

const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
packageJson.version = version;
fs.writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`);

const packageLock = JSON.parse(fs.readFileSync(packageLockPath, "utf8"));
packageLock.version = version;
if (packageLock.packages?.[""]) {
  packageLock.packages[""].version = version;
}
fs.writeFileSync(packageLockPath, `${JSON.stringify(packageLock, null, 2)}\n`);

const cargoToml = fs.readFileSync(cargoTomlPath, "utf8");
const cargoVersionPattern = /(\[package\][\s\S]*?\nversion = ")([^"]+)(")/;
if (!cargoVersionPattern.test(cargoToml)) {
  console.error("未能定位 src-tauri/Cargo.toml 中的 package.version");
  process.exit(1);
}

const nextCargoToml = cargoToml.replace(
  cargoVersionPattern,
  `$1${version}$3`,
);

fs.writeFileSync(cargoTomlPath, nextCargoToml);

const tauriConfig = fs.readFileSync(tauriConfigPath, "utf8");
const tauriVersionPattern = /("version":\s*")([^"]+)(")/;
if (!tauriVersionPattern.test(tauriConfig)) {
  console.error("未能定位 src-tauri/tauri.conf.json 中的 version");
  process.exit(1);
}

fs.writeFileSync(
  tauriConfigPath,
  tauriConfig.replace(tauriVersionPattern, `$1${version}$3`),
);

console.log(`已同步版本号到 ${version}`);
console.log("- package.json");
console.log("- package-lock.json");
console.log("- src-tauri/Cargo.toml");
console.log("- src-tauri/tauri.conf.json");
console.log("未修改 src-tauri/Cargo.lock");
