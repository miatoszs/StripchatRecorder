#!/usr/bin/env node
/**
 * Dev mode
 *
 * 1. 、
 * 2. （ build_tmp/frontend/dist/）
 * 3. （debug）， build_tmp/backend/target/debug/modules/
 * 4. cargo run （ RustEmbed ）
 *
 * 。
 * Re-run this command after frontend or module changes.
 *
 * Usage: npm run dev
 */

"use strict";

const path = require("path");

const {
  ROOT, FRONTEND, BACKEND_MANIFEST, BACKEND_TARGET,
  step, header, run, buildModules, installFrontend,
} = require("./common");

/** （）
 *  Target directory for module binaries (loaded by the backend at runtime) */
const MODULES_OUT = path.join(BACKEND_TARGET, "debug", "modules");

const TOTAL = 4;
header("Dev", "check → frontend → modules → run backend (debug)");

// Check ─────────────────────────────────────────────────────
step(1, TOTAL, "Running checks");
run("node scripts/check.js", {
  cwd: ROOT,
  env: { ...process.env, CHECK_NESTED: "1" },
});

// Build frontend ────────────────────────────────────────
step(2, TOTAL, "Installing & building frontend");
installFrontend();
run("npm run build", { cwd: FRONTEND });

// Build modules & copy binaries ───────────────────
step(3, TOTAL, "Building modules (debug) → build_tmp/backend/target/debug/modules/");
buildModules("debug", MODULES_OUT);

// Start backend ─────────────────────────────────────────
step(4, TOTAL, "Starting backend (debug)");
run(`cargo run --manifest-path "${BACKEND_MANIFEST}"`, {
  env: { ...process.env, CARGO_TARGET_DIR: BACKEND_TARGET },
});
