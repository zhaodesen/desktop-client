#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const bumpType = process.argv[2] ?? "patch";
const allowedBumpTypes = new Set(["patch", "minor", "major"]);

if (!allowedBumpTypes.has(bumpType)) {
  console.error("用法: npm run release:version [patch|minor|major]");
  process.exit(1);
}

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(scriptDir, "..");

function run(command, args, options = {}) {
  return execFileSync(command, args, {
    cwd: rootDir,
    encoding: "utf8",
    stdio: ["inherit", "pipe", "pipe"],
    ...options,
  }).trim();
}

function runStreaming(command, args) {
  execFileSync(command, args, {
    cwd: rootDir,
    stdio: "inherit",
  });
}

function parseVersion(input) {
  const match = input.match(/^v?(\d+)\.(\d+)\.(\d+)$/);
  if (!match) {
    throw new Error(`无法解析语义化版本号: ${input}`);
  }

  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
  };
}

function bumpVersion(version, type) {
  if (type === "major") {
    return `${version.major + 1}.0.0`;
  }

  if (type === "minor") {
    return `${version.major}.${version.minor + 1}.0`;
  }

  return `${version.major}.${version.minor}.${version.patch + 1}`;
}

try {
  const status = run("git", ["status", "--short"]);
  if (status) {
    console.error("工作区不干净，请先提交或清理现有改动后再执行发版。");
    process.exit(1);
  }

  const branch = run("git", ["branch", "--show-current"]);
  if (!branch) {
    console.error("未检测到当前分支，无法继续发版。");
    process.exit(1);
  }

  const latestTag = run("git", ["tag", "--list", "v*", "--sort=-version:refname"])
    .split("\n")
    .find(Boolean);

  const currentVersion = latestTag ? parseVersion(latestTag) : { major: 0, minor: 0, patch: 0 };
  const nextVersion = bumpVersion(currentVersion, bumpType);
  const nextTag = `v${nextVersion}`;

  console.log(`当前分支: ${branch}`);
  console.log(`最新 tag: ${latestTag || "无"}`);
  console.log(`即将发布: ${nextTag}`);

  runStreaming("node", ["scripts/set-version.mjs", nextVersion]);
  runStreaming("git", [
    "add",
    "package.json",
    "package-lock.json",
    "src-tauri/Cargo.toml",
    "src-tauri/tauri.conf.json",
  ]);
  runStreaming("git", ["commit", "-m", `release: ${nextTag}`]);
  runStreaming("git", ["push", "origin", branch]);
  runStreaming("git", ["tag", nextTag]);
  runStreaming("git", ["push", "origin", nextTag]);

  console.log(`发版完成: ${nextTag}`);
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  console.error(message);
  process.exit(1);
}
