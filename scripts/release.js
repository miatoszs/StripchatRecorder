#!/usr/bin/env node
/**
 * Release build pipeline
 *
 * 1. 、
 * 2.   → build_tmp/frontend/dist/
 * 3.  (release)  → build_tmp/backend/target/release/
 * 4.         → build_tmp/modules/<name>/target/release/
 * 5.       → build/
 *    build/
 *    ├── stripchat-recorder
 *    └── modules/
 *        ├── contact_sheet_*
 *        ├── filter_short_*
 *        ├── notify_discord_*
 *        └── notify_telegram_*
 * 6.  build_tmp/
 *
 * Usage: npm run build
 */

"use strict";

const path = require("path");
const fs   = require("fs");

const {
  ROOT, BUILD_TMP, BUILD_OUT,
  BACKEND_MANIFEST, BACKEND_TARGET,
  step, header, run, collectBinaries, listDir, buildModules, installFrontend,
} = require("./common");

const TOTAL = 6;
header("Build", "check → frontend → backend → modules → collect → cleanup");

// Check ─────────────────────────────────────────────────────
step(1, TOTAL, "Running checks");
run("node scripts/check.js", {
  cwd: ROOT,
  env: { ...process.env, CHECK_NESTED: "1" },
});

// Frontend ──────────────────────────────────────────────────
step(2, TOTAL, "Installing & building frontend");
installFrontend();
run("npm run build --prefix frontend", { cwd: ROOT });

// Backend ───────────────────────────────────────────────────
step(3, TOTAL, "Building backend (release)");
if (fs.existsSync(BUILD_OUT)) fs.rmSync(BUILD_OUT, { recursive: true, force: true });
run(`cargo build --manifest-path "${BACKEND_MANIFEST}" --release`, {
  env: { ...process.env, CARGO_TARGET_DIR: BACKEND_TARGET },
});

// Modules ───────────────────────────────────────────────────
step(4, TOTAL, "Building modules (release) → build/modules/");
const BUILD_MODULES_OUT = path.join(BUILD_OUT, "modules");
buildModules("release", BUILD_MODULES_OUT);

// Collect backend binary ──────────────────────────
step(5, TOTAL, "Collecting backend binary → build/");

const backendReleaseDir = path.join(BACKEND_TARGET, "release");
const backendBins = collectBinaries(backendReleaseDir);
if (backendBins.length === 0) {
  console.error(`ERROR: No backend binary found in ${backendReleaseDir}`);
  process.exit(1);
}
for (const name of backendBins) {
  const dst = path.join(BUILD_OUT, name);
  fs.copyFileSync(path.join(backendReleaseDir, name), dst);
  if (process.platform !== "win32") fs.chmodSync(dst, 0o755);
  console.log(`  ✓ build/${name}`);
}

// Cleanup ───────────────────────────────────────────────────
step(6, TOTAL, "Cleanup");
fs.rmSync(BUILD_TMP, { recursive: true, force: true });
console.log("  ✓ build_tmp/ removed");

// Done ──────────────────────────────────────────────────────────────
console.log(`\n${"═".repeat(60)}`);
console.log("  Release build complete!");
console.log(`  Output: build/`);
console.log("═".repeat(60));
listDir(BUILD_OUT);
