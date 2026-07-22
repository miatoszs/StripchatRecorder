#!/usr/bin/env node
/**
 * Desktop type check
 *
 * 1.  desktop
 * 2. vue-tsc --noEmit
 * 3.  cargo check
 *
 * Usage: npm run check:desktop
 */

"use strict";

const {
  DESKTOP, NESTED,
  step, header, run, checkModules, installDesktop,
} = require("./common");

const TOTAL = 3;
header("Desktop Check", "install · vue-tsc · modules");

// Install dependencies ──────────────────────────────────
step(1, TOTAL, "Installing desktop dependencies");
installDesktop();

// Type check ────────────────────────────────────────────
step(2, TOTAL, "Checking desktop types (vue-tsc --noEmit)");
run("npx vue-tsc --noEmit", { cwd: DESKTOP });

// Modules ───────────────────────────────────────────────────
step(3, TOTAL, "Checking modules");
checkModules();

// Done ──────────────────────────────────────────────────────────────
const indent = NESTED ? "    " : "";
console.log(`\n${indent}Desktop check passed.`);
