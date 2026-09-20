import assert from "node:assert/strict";
import { test } from "node:test";

import { ARCH_SLUGS, dmgAssetName, readCaskVersion, updateCaskSource } from "../scripts/lib/cask.mjs";

const ARM_SHA = "9e700250980c7a1b11ae5672ff705fde3a439e71b08aba61edb89e7c61598013";
const INTEL_SHA = "6855416f3f0010ca7c8c5ca36fe421d6edeaabe50db4172d849e5e8277f2f8f9";
const NEW_ARM = "a".repeat(64);
const NEW_INTEL = "b".repeat(64);

// Mirrors the real Casks/skills-manager.rb layout closely enough that a change
// to the tap's formatting breaks these tests rather than silently breaking CI.
const CASK = `cask "skills-manager" do
  arch arm: "aarch64", intel: "x64"

  version "2.1.9"
  sha256 arm:   "${ARM_SHA}",
         intel: "${INTEL_SHA}"

  url "https://github.com/jiweiyeah/Skills-Manager/releases/download/v#{version}/Skills.Manager_#{version}_#{arch}.dmg"
  name "Skills Manager"
  app "Skills Manager.app"
end
`;

test("dmgAssetName matches GitHub's dotted asset naming", () => {
  assert.equal(dmgAssetName("2.2.0", "aarch64"), "Skills.Manager_2.2.0_aarch64.dmg");
  assert.equal(dmgAssetName("2.2.0", "x64"), "Skills.Manager_2.2.0_x64.dmg");
});

test("ARCH_SLUGS matches the cask's arch stanza", () => {
  assert.deepEqual(ARCH_SLUGS, { arm: "aarch64", intel: "x64" });
});

test("readCaskVersion extracts the current version", () => {
  assert.equal(readCaskVersion(CASK), "2.1.9");
  assert.equal(readCaskVersion("cask \"x\" do\nend\n"), null);
});

test("updateCaskSource swaps version and both checksums", () => {
  const updated = updateCaskSource(CASK, {
    version: "2.2.0",
    sha256: { arm: NEW_ARM, intel: NEW_INTEL },
  });

  assert.equal(readCaskVersion(updated), "2.2.0");
  assert.match(updated, new RegExp(`sha256 arm:\\s+"${NEW_ARM}"`));
  assert.match(updated, new RegExp(`intel:\\s+"${NEW_INTEL}"`));
  assert.equal(updated.includes(ARM_SHA), false);
  assert.equal(updated.includes(INTEL_SHA), false);
});

test("updateCaskSource leaves the interpolated url and arch stanza alone", () => {
  const updated = updateCaskSource(CASK, {
    version: "2.2.0",
    sha256: { arm: NEW_ARM, intel: NEW_INTEL },
  });

  // The url derives version/arch via Ruby interpolation, so it must not be
  // rewritten — and `arch arm: "aarch64"` must survive the sha256 arm rewrite.
  assert.match(updated, /Skills\.Manager_#\{version\}_#\{arch\}\.dmg/);
  assert.match(updated, /arch arm: "aarch64", intel: "x64"/);
});

test("updateCaskSource is a no-op when nothing changed", () => {
  const updated = updateCaskSource(CASK, {
    version: "2.1.9",
    sha256: { arm: ARM_SHA, intel: INTEL_SHA },
  });
  assert.equal(updated, CASK);
});

test("updateCaskSource rejects malformed checksums", () => {
  for (const bad of ["", "deadbeef", `${ARM_SHA}00`, "G".repeat(64), ARM_SHA.toUpperCase()]) {
    assert.throws(
      () => updateCaskSource(CASK, { version: "2.2.0", sha256: { arm: bad, intel: NEW_INTEL } }),
      /sha256/,
    );
  }
});

test("updateCaskSource rejects a missing version", () => {
  assert.throws(
    () => updateCaskSource(CASK, { version: "  ", sha256: { arm: NEW_ARM, intel: NEW_INTEL } }),
    /version/,
  );
});

test("updateCaskSource refuses to guess when an anchor is gone", () => {
  const noVersion = CASK.replace(/^\s*version\s+"[^"]*"\n/m, "");
  assert.throws(
    () => updateCaskSource(noVersion, { version: "2.2.0", sha256: { arm: NEW_ARM, intel: NEW_INTEL } }),
    /version 行/,
  );

  const noSha = CASK.replace(/^\s*sha256 arm:.*\n\s*intel:.*\n/m, "");
  assert.throws(
    () => updateCaskSource(noSha, { version: "2.2.0", sha256: { arm: NEW_ARM, intel: NEW_INTEL } }),
    /sha256 arm 行/,
  );
});

test("updateCaskSource refuses an ambiguous cask with two version lines", () => {
  const doubled = CASK.replace('  version "2.1.9"', '  version "2.1.9"\n  version "2.1.9"');
  assert.throws(
    () => updateCaskSource(doubled, { version: "2.2.0", sha256: { arm: NEW_ARM, intel: NEW_INTEL } }),
    /匹配到 2 处/,
  );
});
