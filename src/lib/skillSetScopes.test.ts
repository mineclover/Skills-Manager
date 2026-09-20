import assert from "node:assert/strict";
import { test } from "node:test";
import { describeWorkScopeTags, getWorkScopeTags, parseWorkScopeTags } from "./skillSetScopes.ts";

test("scope input trims and deduplicates tags while preserving case and order", () => {
  assert.deepEqual(parseWorkScopeTags(" api, review, api, , API "), ["api", "review", "API"]);
  assert.deepEqual(parseWorkScopeTags(" ,  , "), []);
});

test("explicit scope tags override legacy values, including an empty array", () => {
  assert.deepEqual(getWorkScopeTags({ work_scope: "legacy", work_scope_tags: [" api ", "review", "api", ""] }), ["api", "review"]);
  assert.deepEqual(getWorkScopeTags({ work_scope: "legacy", work_scope_tags: [] }), []);
});

test("legacy scopes remain singleton tags without interpreting commas", () => {
  assert.deepEqual(getWorkScopeTags({ work_scope: " api,review " }), ["api,review"]);
  assert.deepEqual(getWorkScopeTags({ work_scope: "  " }), []);
  assert.deepEqual(getWorkScopeTags({}), []);
});

test("scope labels distinguish combined requirements and all scopes", () => {
  assert.equal(describeWorkScopeTags({ work_scope_tags: ["api", "review"] }), "api + review");
  assert.equal(describeWorkScopeTags({ work_scope: "api,review" }), "api,review");
  assert.equal(describeWorkScopeTags({ work_scope: "legacy", work_scope_tags: [] }), "All work scopes");
});

test("legacy blank scopes remain unspecified while explicit empty tags cover all scopes", () => {
  assert.equal(describeWorkScopeTags({}), "Scope not specified");
  assert.equal(describeWorkScopeTags({ work_scope: "  " }), "Scope not specified");
  assert.equal(describeWorkScopeTags({ work_scope: "", work_scope_tags: [] }), "All work scopes");
});
