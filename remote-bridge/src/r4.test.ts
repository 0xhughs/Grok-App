import test from "node:test";
import assert from "node:assert/strict";
import { DEFAULT_GROK_CONFIG, emptyConfig } from "./config/types.js";
import { parseConfigText } from "./config/load.js";
import { validateConfig } from "./config/validate.js";
import { isSenderAllowed } from "./core/acl.js";
import { buildGrokArgs } from "./grok/args.js";

test("R4: DEFAULT_GROK_CONFIG and parseMode default to 'default'", () => {
  const def = DEFAULT_GROK_CONFIG();
  assert.equal(def.mode, "default", "DEFAULT_GROK_CONFIG mode must be 'default'");

  const parsed = parseConfigText(`
[projects]
name = "test-proj"
allow_from = "ou_123"

[projects.grok]
work_dir = "/tmp"
`);
  assert.equal(parsed.projects[0]?.grok.mode, "default", "parsed mode without explicit mode must be 'default'");

  const parsedInvalid = parseConfigText(`
[projects]
name = "test-proj"
allow_from = "ou_123"

[projects.grok]
work_dir = "/tmp"
mode = "unknown_invalid_mode"
`);
  assert.equal(parsedInvalid.projects[0]?.grok.mode, "default", "invalid mode must fall back to 'default'");
});

test("R4: isSenderAllowed fails closed: rejects undefined, empty string, and wildcard *", () => {
  assert.equal(isSenderAllowed(undefined, "user_1"), false, "undefined allowFrom must be denied");
  assert.equal(isSenderAllowed("", "user_1"), false, "empty allowFrom must be denied");
  assert.equal(isSenderAllowed("   ", "user_1"), false, "whitespace allowFrom must be denied");
  assert.equal(isSenderAllowed("*", "user_1"), false, "wildcard * allowFrom must be denied");
  assert.equal(isSenderAllowed("user_2, *", "user_1"), false, "wildcard * in list must be denied");
  assert.equal(isSenderAllowed("user_2, *", "user_2"), false, "wildcard * in list must fail closed");

  // Valid allowed senders
  assert.equal(isSenderAllowed("user_1", "user_1"), true, "exact sender match allowed");
  assert.equal(isSenderAllowed("user_1, user_2", "user_2"), true, "listed sender allowed");
  assert.equal(isSenderAllowed("user_1, user_2", "user_3"), false, "unlisted sender denied");
});

test("R4: validateConfig flags empty allow_from and * as fatal level: error", () => {
  const configEmptyAllow = emptyConfig();
  configEmptyAllow.projects = [
    {
      name: "proj-empty",
      allow_from: "",
      require_mention: true,
      feishu: { platform: "feishu", app_id: "cli_app1", app_secret: "sec1" },
      grok: { ...DEFAULT_GROK_CONFIG(), work_dir: "/tmp" },
    },
  ];
  const resEmpty = validateConfig(configEmptyAllow);
  assert.equal(resEmpty.ok, false, "empty allow_from must cause validation failure");
  const emptyIssue = resEmpty.issues.find((i) => i.code === "permit_all_allow_from");
  assert.ok(emptyIssue, "must have permit_all_allow_from issue");
  assert.equal(emptyIssue?.level, "error", "issue level must be error");

  const configWildcard = emptyConfig();
  configWildcard.projects = [
    {
      name: "proj-wildcard",
      allow_from: "*",
      require_mention: true,
      feishu: { platform: "feishu", app_id: "cli_app1", app_secret: "sec1" },
      grok: { ...DEFAULT_GROK_CONFIG(), work_dir: "/tmp" },
    },
  ];
  const resWildcard = validateConfig(configWildcard);
  assert.equal(resWildcard.ok, false, "wildcard * allow_from must cause validation failure");
  const wildcardIssue = resWildcard.issues.find((i) => i.code === "permit_all_allow_from");
  assert.ok(wildcardIssue, "must have permit_all_allow_from issue");
  assert.equal(wildcardIssue?.level, "error", "issue level must be error");

  // Valid allow_from should pass permit_all_allow_from check
  const configValid = emptyConfig();
  configValid.projects = [
    {
      name: "proj-valid",
      allow_from: "ou_user123",
      require_mention: true,
      feishu: { platform: "feishu", app_id: "cli_app1", app_secret: "sec1" },
      grok: { ...DEFAULT_GROK_CONFIG(), work_dir: "/tmp" },
    },
  ];
  const resValid = validateConfig(configValid);
  const validIssue = resValid.issues.find((i) => i.code === "permit_all_allow_from");
  assert.equal(validIssue, undefined, "valid allow_from must not have permit_all_allow_from issue");
});

test("R4: DEFAULT_TEMPLATE and buildGrokArgs in default mode do not include --always-approve or bypassPermissions", () => {
  const argsDefault = buildGrokArgs({
    prompt: "hello",
    cwd: "/tmp",
  });
  assert.ok(!argsDefault.includes("--always-approve"), "default args must not include --always-approve");
  assert.ok(!argsDefault.includes("bypassPermissions"), "default args must not include bypassPermissions");
  const permIdx = argsDefault.indexOf("--permission-mode");
  assert.ok(permIdx >= 0, "must include --permission-mode");
  assert.equal(argsDefault[permIdx + 1], "default", "--permission-mode must be 'default'");

  const argsExplicitDefault = buildGrokArgs({
    prompt: "hello",
    cwd: "/tmp",
    mode: "default",
  });
  assert.ok(!argsExplicitDefault.includes("--always-approve"), "explicit default mode must not include --always-approve");
  assert.ok(!argsExplicitDefault.includes("bypassPermissions"), "explicit default mode must not include bypassPermissions");
  const permIdxExplicit = argsExplicitDefault.indexOf("--permission-mode");
  assert.equal(argsExplicitDefault[permIdxExplicit + 1], "default");
});
