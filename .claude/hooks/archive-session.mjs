#!/usr/bin/env node
/**
 * SessionEnd hook: mirror the conversation transcript into this project's
 * .claude/sessions/<id>.jsonl (+ INDEX.md row). Bodies are gitignored; the
 * index is the tracked pointer. Opt out with PVE_MCP_SESSION_ARCHIVE=0 (the
 * transcript can carry pasted secrets — permanent copies may be unwanted).
 */
import { copyFileSync, existsSync, mkdirSync, readFileSync, appendFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const hookDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(hookDir, "..", "..");
const sessionsDir = join(repoRoot, ".claude", "sessions");

let raw = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => {
  raw += chunk;
});
process.stdin.on("end", () => {
  try {
    if (process.env.PVE_MCP_SESSION_ARCHIVE === "0") {
      console.error("[pve-mcp session-archive] disabled via PVE_MCP_SESSION_ARCHIVE=0");
      process.exit(0);
    }
    const input = JSON.parse(raw);
    const sessionId = input.session_id;
    const transcriptPath = input.transcript_path;
    const cwd = input.cwd;
    if (typeof sessionId !== "string" || typeof transcriptPath !== "string" || typeof cwd !== "string") {
      process.exit(0);
    }
    // Archive only for sessions whose project is this repo (the hook's home).
    if (resolve(cwd) !== repoRoot) {
      process.exit(0);
    }
    if (!existsSync(transcriptPath)) {
      process.exit(0);
    }
    mkdirSync(sessionsDir, { recursive: true });
    const dest = join(sessionsDir, `${sessionId}.jsonl`);
    copyFileSync(transcriptPath, dest);
    const reason = typeof input.reason === "string" ? input.reason : "other";
    const date = new Date().toISOString().slice(0, 10);
    const indexRow = `| ${sessionId.slice(0, 8)} | ${date} | ${reason} | \`${sessionId}.jsonl\` |\n`;
    const index = join(sessionsDir, "INDEX.md");
    if (!existsSync(index)) {
      appendFileSync(
        index,
        "# 会话归档索引\n\n> 实体（.jsonl）gitignored 不进 git；本索引进 git。\n\n| 短 id | 日期 | 结束原因 | 文件 |\n| --- | --- | --- | --- |\n",
      );
    }
    appendFileSync(index, indexRow);
    console.error(`[pve-mcp session-archive] mirrored ${sessionId} (${reason}) -> ${dest}`);
  } catch (error) {
    console.error(`[pve-mcp session-archive] skipped: ${error instanceof Error ? error.message : String(error)}`);
  }
  process.exit(0);
});
