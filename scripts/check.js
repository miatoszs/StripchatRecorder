#!/usr/bin/env node
/**
 * Check all targets for type and compile errors
 *
 * 1.
 * 2.  vue-tsc
 * 3.  cargo check
 * 4.  cargo check
 *
 * Usage: npm run check
 */

"use strict";

const {
  FRONTEND, NESTED,
  BACKEND_MANIFEST, BACKEND_TARGET,
  step, header, run, checkModules, installFrontend,
} = require("./common");

const TOTAL = 4;
header("Check", "frontend types · backend · modules");

// Install dependencies ──────────────────────────────────
step(1, TOTAL, "Installing frontend dependencies");
installFrontend();

// Frontend ──────────────────────────────────────────────────
step(2, TOTAL, "Checking frontend (vue-tsc)");
run("npx vue-tsc --noEmit", { cwd: FRONTEND });

// Backend ───────────────────────────────────────────────────
step(3, TOTAL, "Checking backend");
run(`cargo check --manifest-path "${BACKEND_MANIFEST}"`, {
  env: { ...process.env, CARGO_TARGET_DIR: BACKEND_TARGET },
});

// Modules ───────────────────────────────────────────────────
step(4, TOTAL, "Checking modules");
checkModules();

// Done ──────────────────────────────────────────────────────────────
const indent = NESTED ? "    " : "";
console.log(`\n${indent}All checks passed.`);
