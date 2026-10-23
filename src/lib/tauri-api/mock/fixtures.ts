import featureFlags from "../../../../fixtures/feature-flags.json?raw";
import claudeSettings from "../../../../fixtures/claude-settings.json?raw";
import invalidMissingComma from "../../../../fixtures/invalid-missing-comma.json?raw";
import invalidComment from "../../../../fixtures/invalid-comment.json?raw";
import duplicateKeys from "../../../../fixtures/duplicate-keys.json?raw";
import secret from "../../../../fixtures/secret.json?raw";
import deep from "../../../../fixtures/deep.json?raw";
import numbers from "../../../../fixtures/numbers.json?raw";
import emptyObject from "../../../../fixtures/empty-object.json?raw";
import commented from "../../../../fixtures/commented.jsonc?raw";

export const FIXTURES = { featureFlags, claudeSettings, invalidMissingComma, invalidComment, duplicateKeys, secret, deep, numbers, emptyObject, commented };

/** A realistic settings file with nested arrays of objects (hooks) for the array-of-objects UI. */
export const claudeHooksSettings = `{
  "model": "example-model-name",
  "permissions": {
    "defaultMode": "acceptEdits",
    "allow": ["Bash(git status)", "Bash(pnpm test)"],
    "deny": ["Read(.env)"]
  },
  "env": {
    "EXAMPLE_NON_SECRET_FLAG": "1",
    "API_TOKEN": "sk-test-0000000000000000"
  },
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "enabled": true, "timeout": 30 },
      { "matcher": "Edit|Write", "enabled": false }
    ]
  }
}
`;
