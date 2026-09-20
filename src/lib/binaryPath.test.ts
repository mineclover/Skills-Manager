import { test } from "node:test";
import assert from "node:assert/strict";

import { binaryDir } from "./binaryPath.ts";

test("binaryDir strips the binary name from unix and windows paths", () => {
  assert.equal(binaryDir("/usr/local/bin/skm"), "/usr/local/bin");
  assert.equal(binaryDir("/Users/me/.local/bin/skm"), "/Users/me/.local/bin");
  assert.equal(binaryDir("C:\\Program Files\\Skills Manager\\skm.exe"), "C:\\Program Files\\Skills Manager");
});

test("binaryDir handles paths without a usable directory component", () => {
  assert.equal(binaryDir("skm"), "");
  assert.equal(binaryDir(""), "");
  assert.equal(binaryDir("/skm"), "/");
});
