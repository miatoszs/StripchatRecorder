/**
 * Shared build script utilities
 *
 * release.js 引用，避免重复代码。
 * Used by dev.js, check.js, and release.js to avoid duplication.
 */

"use strict";

const { execSync } = require("child_process");
const path = require("path");
const fs   = require("fs");

// Path constants ────────────────────────────────────────────────

const ROOT        = path.resolve(__dirname, "..");
const FRONTEND    = path.join(ROOT, "frontend");
const DESKTOP     = path.join(ROOT, "desktop");
const MODULES_DIR = path.join(ROOT, "modules");
const BUILD_TMP   = path.join(ROOT, "build_tmp");
const BUILD_OUT   = path.join(ROOT, "build");

/*Backend Cargo.toml path */
const BACKEND_MANIFEST = path.join(ROOT, "backend", "Cargo.toml");

/*Backend target directory */
const BACKEND_TARGET = path.join(BUILD_TMP, "backend", "target");

/*Desktop (Tauri) target directory */
const DESKTOP_TARGET = path.join(BUILD_TMP, "desktop", "target");

/*Target directory for a given module */
function moduleTarget(name) {
  return path.join(BUILD_TMP, "modules", name, "target");
}

/** （ crate）/ List all buildable module names (skip pure library crates) */
function listModules() {
  const skip = new Set(["pp_utils"]);
  return fs
    .readdirSync(MODULES_DIR)
    .filter(
      (n) => !skip.has(n) && fs.existsSync(path.join(MODULES_DIR, n, "Cargo.toml"))
    );
}

// ANSI colors ──────────────────────────────────────────────────

const C = {
  reset:  "\x1b[0m",
  cyan:   "\x1b[36m",   // header 边框
  yellow: "\x1b[33m",   // step 边框
  gray:   "\x1b[90m",   // 嵌套时的暗色
  bold:   "\x1b[1m",
};

/** （ dev/release ）
 *  Whether running as a nested subprocess (injected by dev/release via env var) */
const NESTED = process.env.CHECK_NESTED === "1";

// Output helpers ────────────────────────────────────────────────

/*Print the overall script header
 * title
 * description
 */
function header(title, desc) {
  if (NESTED) {
    const indent = "    ";
    console.log(`\n${indent}${C.gray}${"┄".repeat(52)}${C.reset}`);
    console.log(`${indent}${C.gray}  ${title}${C.reset}`);
    if (desc) console.log(`${indent}${C.gray}  ${desc}${C.reset}`);
    console.log(`${indent}${C.gray}${"┄".repeat(52)}${C.reset}`);
  } else {
    console.log(`\n${C.cyan}${"═".repeat(60)}${C.reset}`);
    console.log(`${C.bold}  ${title}${C.reset}`);
    if (desc) console.log(`  ${desc}`);
    console.log(`${C.cyan}${"═".repeat(60)}${C.reset}`);
  }
}

/*Print a separator with a step title
 * current step number
 * total steps
 * step description
 */
function step(current, total, msg) {
  if (NESTED) {
    const indent = "    ";
    console.log(`\n${indent}${C.gray}[${current}/${total}]  ${msg}${C.reset}`);
  } else {
    console.log(`\n${C.yellow}${"─".repeat(60)}${C.reset}`);
    console.log(`  ${C.bold}[${current}/${total}]${C.reset}  ${msg}`);
    console.log(`${C.yellow}${"─".repeat(60)}${C.reset}`);
  }
}

/**
 * Run a command synchronously, inheriting stdio.
 * @param {string} cmd
 * @param {import("child_process").ExecSyncOptions} [opts]
 */
function run(cmd, opts = {}) {
  execSync(cmd, { stdio: "inherit", ...opts });
}

// Binary collection ──────────────────────────────────────────

/**
 * 。
 * Collect all executable files at the top level of a directory.
 * Windows: *.exe；Linux/macOS: 。
 * @param {string} releaseDir
 * @returns {string[]}
 */
function collectBinaries(releaseDir) {
  if (!fs.existsSync(releaseDir)) return [];
  const isWindows = process.platform === "win32";
  return fs.readdirSync(releaseDir).filter((name) => {
    const full = path.join(releaseDir, name);
    const stat = fs.statSync(full);
    if (!stat.isFile()) return false;
    if (isWindows) return name.endsWith(".exe");
    return !path.extname(name) && (stat.mode & 0o111) !== 0;
  });
}

// Directory listing ────────────────────────────────────────────

/**
 * （）。
 * Recursively print directory contents (display artifacts after build).
 * @param {string} dir
 * @param {string} [prefix]
 */
function listDir(dir, prefix = "") {
  for (const name of fs.readdirSync(dir).sort()) {
    const full = path.join(dir, name);
    if (fs.statSync(full).isDirectory()) {
      console.log(`  ${prefix}${name}/`);
      listDir(full, prefix + "  ");
    } else {
      const size = (fs.statSync(full).size / 1024).toFixed(0);
      console.log(`  ${prefix}${name}  (${size} KB)`);
    }
  }
}

// Module build & check ───────────────────────────────────

/**
 * cargo check。
 * Run `cargo check` for all modules.
 */
function checkModules() {
  for (const name of listModules()) {
    run(
      `cargo check --manifest-path "${path.join(MODULES_DIR, name, "Cargo.toml")}"`,
      { env: { ...process.env, CARGO_TARGET_DIR: moduleTarget(name) } }
    );
  }
}

/**
 * 。
 * Build all modules and copy output binaries to the given directory.
 *
 * Cargo build profile
 * Target directory for copied binaries
 */
function buildModules(profile, outDir) {
  const releaseFlag = profile === "release" ? " --release" : "";
  fs.mkdirSync(outDir, { recursive: true });
  for (const name of listModules()) {
    console.log(`  → ${name}`);
    run(
      `cargo build --manifest-path "${path.join(MODULES_DIR, name, "Cargo.toml")}" --bins${releaseFlag}`,
      { env: { ...process.env, CARGO_TARGET_DIR: moduleTarget(name) } }
    );
    const bins = collectBinaries(path.join(moduleTarget(name), profile));
    for (const bin of bins) {
      const dst = path.join(outDir, bin);
      fs.copyFileSync(path.join(moduleTarget(name), profile, bin), dst);
      if (process.platform !== "win32") fs.chmodSync(dst, 0o755);
    }
    console.log(`  ✓ ${name}\n`);
  }
}

/**
 * （）。
 * Recursively copy a directory (used for collecting build artifacts).
 *
 * Source directory
 * Destination directory
 * Base path prefix for log output
 */
function copyDir(src, dst, logBase) {
  fs.mkdirSync(dst, { recursive: true });
  for (const entry of fs.readdirSync(src)) {
    const srcPath = path.join(src, entry);
    const dstPath = path.join(dst, entry);
    if (fs.statSync(srcPath).isDirectory()) {
      copyDir(srcPath, dstPath, logBase);
    } else {
      fs.copyFileSync(srcPath, dstPath);
      if (logBase !== undefined) {
        console.log(`  ✓ ${logBase}${path.relative(dst, dstPath).replace(/\\/g, "/")}`);
      }
    }
  }
}

// Frontend dependency install ──────────────────────────────

/**
 * npm （）。
 * Install frontend npm dependencies (called before each script runs).
 */
function installFrontend() {
  console.log("Installing frontend dependencies...");
  run("npm install", { cwd: FRONTEND });
}

/**
 * desktop npm （ desktop ）。
 * Install desktop npm dependencies (called before each desktop script runs).
 */
function installDesktop() {
  console.log("Installing desktop dependencies...");
  run("npm install", { cwd: DESKTOP });
}

// Exports ───────────────────────────────────────────────────────────

module.exports = {
  ROOT,
  FRONTEND,
  DESKTOP,
  MODULES_DIR,
  BUILD_TMP,
  BUILD_OUT,
  BACKEND_MANIFEST,
  BACKEND_TARGET,
  DESKTOP_TARGET,
  NESTED,
  moduleTarget,
  listModules,
  step,
  header,
  run,
  collectBinaries,
  listDir,
  checkModules,
  buildModules,
  copyDir,
  installFrontend,
  installDesktop,
};
