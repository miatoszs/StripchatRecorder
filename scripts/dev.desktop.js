#!/usr/bin/env node
/**
 * Desktop dev mode
 *
 * 1.  desktop
 * 2. （debug）， build_tmp/desktop/target/debug/modules/
 * 3.  Tauri （tauri dev）
 *    -  Vite  Tauri  beforeDevCommand
 *    - Rust  build_tmp/desktop/target/
 *
 * Usage: npm run dev:desktop
 */

"use strict";

const path = require("path");

const {
  DESKTOP, DESKTOP_TARGET,
  step, header, run, buildModules, installDesktop,
} = require("./common");

/** （Tauri ）
 *  Target directory for module binaries (loaded by Tauri at runtime) */
const MODULES_OUT = path.join(DESKTOP_TARGET, "debug", "modules");

const TOTAL = 3;
header("Desktop Dev", "install → modules → tauri dev");

// Install dependencies ──────────────────────────────────
step(1, TOTAL, "Installing desktop dependencies");
installDesktop();

// Build modules & copy binaries ───────────────────
step(2, TOTAL, "Building modules (debug) → build_tmp/desktop/target/debug/modules/");
buildModules("debug", MODULES_OUT);

// Start Tauri dev ──────────────────────────
step(3, TOTAL, "Starting Tauri dev (build_tmp/desktop/target/)");
run("npx tauri dev", {
  cwd: DESKTOP,
  env: { ...process.env, CARGO_TARGET_DIR: DESKTOP_TARGET },
});
