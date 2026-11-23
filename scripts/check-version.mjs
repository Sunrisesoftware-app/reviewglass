// One version, three manifests (adr.rg.037): package.json, src-tauri/tauri.conf.json and
// src-tauri/Cargo.toml must say the same, and Cargo.lock must agree with Cargo.toml.
// The installer, the About and the release tag all read one of them, so a release with
// two versions in it is stopped here, in `pnpm check` and in CI.
//
//   node scripts/check-version.mjs            check, print the version
//   node scripts/check-version.mjs 0.2.0      set all of them to 0.2.0 (then cargo
//                                             updates Cargo.lock on the next build)

import { readFileSync, writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const file = (p) => join(root, p);

const pkgPath = file("package.json");
const confPath = file("src-tauri/tauri.conf.json");
const cargoPath = file("src-tauri/Cargo.toml");
const lockPath = file("src-tauri/Cargo.lock");

const SEMVER = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;

// The [package] table's own version line, not a dependency's.
function cargoVersion(text) {
  const pkg = text.split(/^\[/m).find((t) => t.startsWith("package]"));
  const m = pkg && pkg.match(/^version\s*=\s*"([^"]+)"/m);
  return m ? m[1] : null;
}

function lockVersion(text) {
  const m = text.match(/\[\[package\]\]\r?\nname = "reviewglass"\r?\nversion = "([^"]+)"/);
  return m ? m[1] : null;
}

const set = process.argv[2];
if (set) {
  if (!SEMVER.test(set)) {
    console.error(`not a version: ${set}`);
    process.exit(1);
  }
  const pkg = readFileSync(pkgPath, "utf8");
  writeFileSync(pkgPath, pkg.replace(/("version":\s*")[^"]+(")/, `$1${set}$2`));
  const conf = readFileSync(confPath, "utf8");
  writeFileSync(confPath, conf.replace(/("version":\s*")[^"]+(")/, `$1${set}$2`));
  const cargo = readFileSync(cargoPath, "utf8");
  writeFileSync(
    cargoPath,
    cargo.replace(/(\[package\][^[]*?\nversion\s*=\s*")[^"]+(")/, `$1${set}$2`),
  );
}

const found = {
  "package.json": JSON.parse(readFileSync(pkgPath, "utf8")).version,
  "src-tauri/tauri.conf.json": JSON.parse(readFileSync(confPath, "utf8")).version,
  "src-tauri/Cargo.toml": cargoVersion(readFileSync(cargoPath, "utf8")),
};
if (!set) found["src-tauri/Cargo.lock"] = lockVersion(readFileSync(lockPath, "utf8"));

const versions = new Set(Object.values(found));
if (versions.size !== 1 || !SEMVER.test([...versions][0] ?? "")) {
  console.error("The version differs between the manifests:");
  for (const [f, v] of Object.entries(found)) console.error(`  ${f}: ${v ?? "(not found)"}`);
  process.exit(1);
}
console.log(`version ${[...versions][0]}${set ? " (set)" : ""}`);
