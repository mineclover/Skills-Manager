// Copies the release-built skm CLI binary into src-tauri/resources/ so the
// Tauri bundler picks it up as an app resource (see bundle.resources in
// tauri.conf.json). Run via `npm run bundle:cli`, wired into beforeBuildCommand.
//
// Cross-compiled builds (`cargo build --target <triple>`, which CI uses on
// macOS) put the binary in target/<triple>/release/, not target/release/.
// Tauri sets TAURI_ENV_TARGET_TRIPLE for hook commands, so prefer that and
// fall back to scanning target/*/release for a plain `cargo build`.
//
// Missing binary is a warning locally (frontend-only iterations keep working)
// but a hard error when SKM_REQUIRE_CLI=1, so a release can never silently
// ship without the CLI.
import { copyFileSync, existsSync, mkdirSync, readdirSync } from "node:fs";
import { join } from "node:path";

const root = process.cwd();
const required = process.env.SKM_REQUIRE_CLI === "1";
const binaryNames = ["skm", "skm.exe"];

function releaseDirs() {
  const dirs = [];

  // Explicit target triple from Tauri's hook env, or a cargo config default.
  const triple =
    process.env.TAURI_ENV_TARGET_TRIPLE || process.env.CARGO_BUILD_TARGET;
  if (triple) {
    dirs.push(join(root, "target", triple, "release"));
  }

  // Plain `cargo build --release` (no --target).
  dirs.push(join(root, "target", "release"));

  // Last resort: any target/<triple>/release left by an earlier build.
  const targetRoot = join(root, "target");
  if (existsSync(targetRoot)) {
    for (const entry of readdirSync(targetRoot, { withFileTypes: true })) {
      if (!entry.isDirectory() || entry.name === "release" || entry.name === "debug") {
        continue;
      }
      dirs.push(join(targetRoot, entry.name, "release"));
    }
  }

  return dirs;
}

function findBinary() {
  for (const dir of releaseDirs()) {
    for (const name of binaryNames) {
      const candidate = join(dir, name);
      if (existsSync(candidate)) {
        return candidate;
      }
    }
  }
  return null;
}

const source = findBinary();
if (!source) {
  const searched = releaseDirs().join(", ");
  const message = `[bundle:cli] skm not found (run \`cargo build --release -p skm\` first); searched: ${searched}`;
  if (required) {
    console.error(`${message}\n[bundle:cli] SKM_REQUIRE_CLI=1 — refusing to package without the CLI`);
    process.exit(1);
  }
  console.warn(`${message}\n[bundle:cli] packaging app without CLI resource`);
  process.exit(0);
}

const destDir = join(root, "src-tauri", "resources");
mkdirSync(destDir, { recursive: true });
const dest = join(destDir, source.endsWith(".exe") ? "skm.exe" : "skm");
copyFileSync(source, dest);
console.log(`[bundle:cli] copied ${source} -> ${dest}`);
