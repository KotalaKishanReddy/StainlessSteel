"use strict";
// Downloads the prebuilt stainless binary for this platform from the
// GitHub release matching this package version, verifies its SHA-256
// checksum, and places it in vendor/. No dependencies; stdlib only.
const { createHash } = require("node:crypto");
const { createWriteStream, promises: fs } = require("node:fs");
const https = require("node:https");
const os = require("node:os");
const path = require("node:path");

const REPO = "KotalaKishanReddy/StainlessSteel";
const VERSION = process.env.STAINLESS_VERSION || require("./package.json").version;
const TAG = VERSION.startsWith("v") ? VERSION : `v${VERSION}`;

const TARGETS = {
  "linux-x64": "stainless-linux-x86_64",
  "linux-arm64": "stainless-linux-aarch64",
  "darwin-arm64": "stainless-macos-aarch64",
};

function platformKey() {
  return `${os.platform()}-${os.arch()}`;
}

function get(url) {
  return new Promise((resolve, reject) => {
    const req = https.get(
      url,
      { headers: { "User-Agent": "stainlesssteel-npm-install" } },
      (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          res.resume();
          resolve(get(res.headers.location));
          return;
        }
        if (res.statusCode !== 200) {
          res.resume();
          reject(new Error(`download failed: HTTP ${res.statusCode} for ${url}`));
          return;
        }
        const chunks = [];
        res.on("data", (c) => chunks.push(c));
        res.on("end", () => resolve(Buffer.concat(chunks)));
        res.on("error", reject);
      }
    );
    req.on("error", reject);
    req.setTimeout(120000, () => req.destroy(new Error("download timed out")));
  });
}

async function main() {
  const key = platformKey();
  const current = TARGETS[key];
  if (!current) {
    throw new Error(
      `no prebuilt stainless binary for ${key}; build from source: ` +
        `git clone https://github.com/${REPO}.git && cd StainlessSteel && cargo build --release --locked`
    );
  }
  const base = `https://github.com/${REPO}/releases/download/${TAG}`;
  const sums = (await get(`${base}/SHA256SUMS.txt`)).toString("utf8");
  let asset = current;
  let entry = sums.split("\n").find((l) => l.trimEnd().endsWith(`  ${asset}`));
  if (!entry) {
    // Legacy v0.1.0 asset naming fallback.
    asset = `stainless-${TAG.slice(1)}-${key === "linux-x64" ? "linux-x86_64" : key === "linux-arm64" ? "linux-aarch64" : "macos-aarch64"}`;
    entry = sums.split("\n").find((l) => l.trimEnd().endsWith(`  ${asset}`));
  }
  if (!entry) {
    throw new Error(`no asset for ${key} in ${TAG}`);
  }
  const want = entry.split(/\s+/)[0].toLowerCase();
  const bytes = await get(`${base}/${asset}`);
  const got = createHash("sha256").update(bytes).digest("hex");
  if (got !== want) {
    throw new Error("checksum mismatch: refusing to install");
  }
  const dir = path.join(__dirname, "vendor");
  await fs.mkdir(dir, { recursive: true });
  const dest = path.join(dir, "stainless");
  await fs.writeFile(dest, bytes, { mode: 0o755 });
  await fs.chmod(dest, 0o755);
  console.log(`stainless ${VERSION} installed for ${key} (sha256 verified)`);
}

main().catch((err) => {
  console.error(`stainlesssteel postinstall failed: ${err.message}`);
  process.exit(1);
});
