#!/usr/bin/env node
/**
 * Desktop release build
 *
 * 1. （vue-tsc --noEmit）+  cargo check
 * 2.  desktop
 * 3. （release）， build_tmp/desktop/target/release/modules/
 * 4. Tauri （tauri build）
 *    -  Tauri  beforeBuildCommand  vite build → desktop/dist/
 *    - Rust  build_tmp/desktop/target/
 * 5.  bundle  → build/desktop/
 * 6.  → build/desktop/modules/
 * 7.  build_tmp/desktop/
 *
 * Usage: npm run build:desktop
 */

"use strict";

const path = require("path");
const fs   = require("fs");

const {
  ROOT, DESKTOP, DESKTOP_TARGET, BUILD_OUT, BUILD_TMP, NESTED,
  step, header, run, listDir, collectBinaries, buildModules, copyDir, installDesktop,
} = require("./common");

/*Tauri bundle output source */
const TAURI_BUNDLE_SRC = path.join(DESKTOP_TARGET, "release", "bundle");

/*Desktop artifact collection target */
const DESKTOP_BUILD_OUT = path.join(BUILD_OUT, "desktop");

/*Desktop build_tmp directory */
const DESKTOP_BUILD_TMP = path.join(BUILD_TMP, "desktop");

const TOTAL = 7;
header("Desktop Build", "check → install → modules → tauri build → collect → collect modules → cleanup");

// Type check ────────────────────────────────────────────
step(1, TOTAL, "Type checking desktop + modules");
run("node scripts/check.desktop.js", {
  cwd: ROOT,
  env: { ...process.env, CHECK_NESTED: "1" },
});

// Install dependencies ──────────────────────────────────
step(2, TOTAL, "Installing desktop dependencies");
if (fs.existsSync(DESKTOP_BUILD_OUT)) fs.rmSync(DESKTOP_BUILD_OUT, { recursive: true, force: true });
installDesktop();

// Build modules & copy binaries ───────────────────
step(3, TOTAL, "Building modules (release) → build/desktop/modules/");
const DESKTOP_MODULES_OUT = path.join(DESKTOP_BUILD_OUT, "modules");
buildModules("release", DESKTOP_MODULES_OUT);

// Tauri build ─────────────────────────────────────────
step(4, TOTAL, "Building desktop (tauri build) → build_tmp/desktop/target/");
run("npx tauri build", {
  cwd: DESKTOP,
  env: { ...process.env, CARGO_TARGET_DIR: DESKTOP_TARGET },
});

// Collect bundle artifacts ──────────────────────
step(5, TOTAL, "Collecting bundle artifacts → build/desktop/");

if (!fs.existsSync(TAURI_BUNDLE_SRC)) {
  console.error(`ERROR: Tauri bundle directory not found: ${TAURI_BUNDLE_SRC}`);
  process.exit(1);
}
copyDir(TAURI_BUNDLE_SRC, DESKTOP_BUILD_OUT, "build/desktop/");

// Collect module binaries ─────────────────────────
step(6, TOTAL, "Collecting module binaries → build/desktop/modules/");
const moduleBins = collectBinaries(DESKTOP_MODULES_OUT);
if (moduleBins.length === 0) {
  console.warn("  ⚠ No module binaries found");
} else {
  for (const bin of moduleBins) {
    console.log(`  ✓ build/desktop/modules/${bin}`);
  }
}

// Cleanup ───────────────────────────────────────────────────
step(7, TOTAL, "Cleanup");
fs.rmSync(DESKTOP_BUILD_TMP, { recursive: true, force: true });
console.log("  ✓ build_tmp/desktop/ removed");

// Done ──────────────────────────────────────────────────────────────
console.log(`\n${"═".repeat(60)}`);
console.log("  Desktop build complete!");
console.log("  Output: build/desktop/");
console.log("═".repeat(60));
listDir(DESKTOP_BUILD_OUT);
