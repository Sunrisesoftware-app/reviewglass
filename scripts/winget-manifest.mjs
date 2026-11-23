// winget's three manifest files for one release (adr.rg.037), made by the release
// workflow and attached to the draft release. Submitting them to microsoft/winget-pkgs
// is the owner's step (P8); this only makes them, so the submission is a copy.
//
//   node scripts/winget-manifest.mjs --version 0.1.0 --sha256 <hex> --out <dir> [--date YYYY-MM-DD]
//
// The installer URL is the release asset's public address, which exists once the draft
// is published under the tag v<version>.

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const ID = "SunriseSoftware.ReviewGlass";
const REPO = "https://github.com/Sunrisesoftware-app/reviewglass";
const MANIFEST_VERSION = "1.6.0";

function arg(name) {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : undefined;
}

export function manifests({ version, sha256, date }) {
  if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version ?? "")) throw new Error(`not a version: ${version}`);
  if (!/^[0-9a-fA-F]{64}$/.test(sha256 ?? "")) throw new Error("--sha256 must be 64 hex digits");
  const url = `${REPO}/releases/download/v${version}/ReviewGlass_${version}_x64-setup.exe`;
  const head = (type) =>
    `# Created by scripts/winget-manifest.mjs\n# yaml-language-server: $schema=https://aka.ms/winget-manifest.${type}.${MANIFEST_VERSION}.schema.json\n\n`;
  return {
    [`${ID}.yaml`]:
      head("version") +
      `PackageIdentifier: ${ID}\nPackageVersion: ${version}\nDefaultLocale: en-US\nManifestType: version\nManifestVersion: ${MANIFEST_VERSION}\n`,
    [`${ID}.installer.yaml`]:
      head("installer") +
      `PackageIdentifier: ${ID}\nPackageVersion: ${version}\nInstallerType: nullsoft\nScope: user\n` +
      `InstallModes:\n- interactive\n- silent\nUpgradeBehavior: install\n` +
      (date ? `ReleaseDate: ${date}\n` : "") +
      `Installers:\n- Architecture: x64\n  InstallerUrl: ${url}\n  InstallerSha256: ${sha256.toUpperCase()}\n` +
      `ManifestType: installer\nManifestVersion: ${MANIFEST_VERSION}\n`,
    [`${ID}.locale.en-US.yaml`]:
      head("defaultLocale") +
      `PackageIdentifier: ${ID}\nPackageVersion: ${version}\nPackageLocale: en-US\n` +
      `Publisher: Sunrise Software Oy\nPackageName: ReviewGlass\nPackageUrl: ${REPO}\n` +
      `License: Apache-2.0\nLicenseUrl: ${REPO}/blob/main/LICENSE\n` +
      `ShortDescription: A magnifier and live diff for Claude Code on Windows.\n` +
      `Description: |-\n  A magnifier that reads a Claude Code pane at a size you can follow, a dock with every\n  running session and the account's quota, and a live diff of what the agent just\n  changed. Independent project; not made by Anthropic. ReviewGlass never checks for\n  updates itself: winget upgrade or the GitHub releases page bring a new version.\n` +
      `Moniker: reviewglass\nTags:\n- claude-code\n- magnifier\n- accessibility\n- diff\n- developer-tools\n` +
      `ReleaseNotesUrl: ${REPO}/releases/tag/v${version}\n` +
      `ManifestType: defaultLocale\nManifestVersion: ${MANIFEST_VERSION}\n`,
  };
}

if (process.argv[1] && process.argv[1].endsWith("winget-manifest.mjs")) {
  try {
    const version = arg("version");
    const files = manifests({ version, sha256: arg("sha256"), date: arg("date") });
    const out = arg("out");
    if (!out) throw new Error("--out is required");
    // winget-pkgs layout: manifests/<first letter>/<Publisher>/<Name>/<version>/
    const dir = join(out, "manifests", "s", "SunriseSoftware", "ReviewGlass", version);
    mkdirSync(dir, { recursive: true });
    for (const [name, text] of Object.entries(files)) writeFileSync(join(dir, name), text);
    console.log(dir);
  } catch (e) {
    console.error(String(e instanceof Error ? e.message : e));
    process.exit(1);
  }
}
