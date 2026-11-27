#!/usr/bin/env node
/**
 * PreToolUse guard: deny Bash/PowerShell commands matching dangerous
 * patterns. Two risk classes in this repo:
 * 1. Local destructive commands (rm -rf, git push --force, ...)
 * 2. PVE production mutation via ad-hoc curl/pvesh (the MCP tool surface
 *    enforces confirm=true in code; ad-hoc shell calls bypass that, so the
 *    guard blocks non-read PVE API verbs outright)
 * Read-only probes pass through to the normal permission flow.
 *
 * Input (stdin JSON): { tool_name, tool_input: { command } }
 * Output: exit 0 = no decision; JSON permissionDecision deny = block.
 */

const DANGEROUS_PATTERNS = [
  "rm -rf",
  "rm -fr",
  "git push --force",
  "git push -f",
  "git reset --hard",
  "git clean -f",
  "remove-item -recurse",
  "format c:",
  "mkfs",
  "dd if=",
  "curl -o- | sh",
  "curl -o- | bash",
  "wget -qo- | sh",
  "wget -qo- | bash",
  "iex (",
  "invoke-expression",
];

// PVE mutation verbs: any curl/pvesh carrying these against the cluster is
// blocked (read-only GET passes; the MCP tool layer owns gated mutations).
const PVE_MUTATION_PATTERNS = [
  "-x post",
  "-x put",
  "-x delete",
  "--request post",
  "--request put",
  "--request delete",
  "pvesh create",
  "pvesh set",
  "pvesh delete",
];

const READ_ONLY_PREFIXES = [
  "ls",
  "cat ",
  "head ",
  "tail ",
  "grep ",
  "rg ",
  "git status",
  "git diff",
  "git log",
  "git show",
  "cargo --version",
  "rustc --version",
  "node --version",
  "just --list",
];

function isReadOnly(command) {
  return READ_ONLY_PREFIXES.some((prefix) => command.startsWith(prefix));
}

function main() {
  let raw = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (chunk) => {
    raw += chunk;
  });
  process.stdin.on("end", () => {
    let toolName = "";
    let command = "";
    try {
      const input = JSON.parse(raw);
      toolName = typeof input.tool_name === "string" ? input.tool_name : "";
      command = typeof input.tool_input?.command === "string" ? input.tool_input.command : "";
    } catch {
      // unparsable input — nothing to guard
    }
    if (toolName !== "Bash" && toolName !== "PowerShell") {
      process.exit(0);
    }
    const lowered = command.toLowerCase().trim();
    if (isReadOnly(lowered)) {
      process.exit(0);
    }
    const pveHit = PVE_MUTATION_PATTERNS.find((pattern) => lowered.includes(pattern));
    if (pveHit !== undefined) {
      process.stdout.write(
        JSON.stringify({
          hookSpecificOutput: {
            hookEventName: "PreToolUse",
            permissionDecision: "deny",
            permissionDecisionReason: `PVE mutation via ad-hoc shell ("${pveHit}") is blocked. Use the project's MCP tool surface (confirm=true guardrail) or ask the user to run it directly.`,
          },
        }),
      );
      process.exit(0);
    }
    const hit = DANGEROUS_PATTERNS.find((pattern) => lowered.includes(pattern));
    if (hit === undefined) {
      process.exit(0);
    }
    process.stdout.write(
      JSON.stringify({
        hookSpecificOutput: {
          hookEventName: "PreToolUse",
          permissionDecision: "deny",
          permissionDecisionReason: `Dangerous pattern "${hit}" matches. If this is intentional, ask the user to run it directly or adjust the guard.`,
        },
      }),
    );
    process.exit(0);
  });
}

main();
