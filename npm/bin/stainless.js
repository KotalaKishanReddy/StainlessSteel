#!/usr/bin/env node
"use strict";
// Shim: exec the platform binary downloaded by postinstall.
const { spawnSync } = require("node:child_process");
const path = require("node:path");
const fs = require("node:fs");

const bin = path.join(__dirname, "..", "vendor", "stainless");
if (!fs.existsSync(bin)) {
  console.error(
    "stainless binary missing (postinstall did not complete?). " +
      "Reinstall with: npm install -g stainlesssteel"
  );
  process.exit(1);
}
const child = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
process.exit(child.status === null ? 1 : child.status);
