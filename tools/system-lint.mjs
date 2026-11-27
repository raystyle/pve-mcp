#!/usr/bin/env node
/**
 * system-lint: global consistency checks for the pve-mcp dev-system assets.
 *
 * Precedent: vulnforge tools/system-lint.mjs (manypkg check-registry shape)
 * adapted to this repo's asset graph:
 *
 *   facts-of-record                      derived documents that must echo them
 *   ----------------                     ----------------------------------
 *   justfile ci recipe                   rules/verify skill/README/CLAUDE.md
 *   crates/pve-core/src/config.rs        pve-mcp.toml.example
 *   .ad_records/NNN-*.md                 .ad_records/README.md index rows
 *   .research/<feature>/ dirs            .research/README.md feature table
 *   .claude/settings.json hooks          .claude/hooks/*.mjs on disk
 *   .claude/skills/<name>/ dirs          SKILL.md with frontmatter
 *   docs-style.md term table             all Chinese markdown
 *   pve-mcp.toml (gitignored)            never tracked; value never leaks
 *
 * Exit 0 = all checks pass; exit 1 = findings listed.
 */
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join, relative } from "node:path";

const ROOT = join(import.meta.dirname, "..");
const read = (rel) => readFileSync(join(ROOT, rel), "utf8");
const exists = (rel) => existsSync(join(ROOT, rel));

const findings = [];
let checked = 0;

function check(name, validate) {
  const results = validate();
  for (const result of results) {
    findings.push({ check: name, ...result });
  }
  checked++;
}

// ---------------------------------------------------------------------------
// 1. Gate-docs alignment: every document that describes the check gate must
//    echo its stages. The justfile ci recipe is the fact of record.
// ---------------------------------------------------------------------------
check("gate-docs-alignment", () => {
  const justfile = read("justfile");
  const ciRecipe = justfile.slice(justfile.indexOf("ci:"));
  const stages = ["fmt", "lint", "test", "test-doc"].filter((s) =>
    new RegExp(`\\b${s}\\b`).test(ciRecipe),
  );
  if (stages.length === 0) {
    return [{ file: "justfile", text: "ci recipe has no recognizable stages" }];
  }
  const docs = [
    [".claude/rules/rust-dev.md", "rust dev rules (gate section)"],
    [".claude/skills/verify/SKILL.md", "verify skill (gate step)"],
    ["README.md", "README (quick start)"],
  ];
  const results = [];
  for (const [rel, what] of docs) {
    if (!exists(rel)) {
      results.push({ file: rel, text: `${what}: file missing` });
      continue;
    }
    const text = read(rel);
    // Documents must name the aggregate gate (just ci) and at least the
    // clippy denial + nextest runner — the two stages teams tend to drop.
    for (const needle of ["just ci", "clippy", "nextest"]) {
      if (!text.includes(needle)) {
        results.push({ file: rel, text: `${what}: does not mention "${needle}"` });
      }
    }
  }
  return results;
});

// ---------------------------------------------------------------------------
// 2. Config example sync: every field of the config structs (config.rs is
//    the fact of record) must appear in pve-mcp.toml.example. Field drift
//    (new config knob without example entry) fails here.
// ---------------------------------------------------------------------------
check("config-example-sync", () => {
  if (!exists("crates/pve-core/src/config.rs") || !exists("pve-mcp.toml.example")) {
    return []; // scaffold states — no crash
  }
  const source = read("crates/pve-core/src/config.rs");
  const example = read("pve-mcp.toml.example");
  const fields = [];
  for (const match of source.matchAll(/^\s+pub (\w+):/gm)) {
    fields.push(match[1]);
  }
  const results = [];
  for (const field of fields) {
    if (!new RegExp(`\\b${field}\\b`).test(example)) {
      results.push({
        file: "pve-mcp.toml.example",
        text: `config field "${field}" exists in config.rs but the example does not document it`,
      });
    }
  }
  return results;
});

// ---------------------------------------------------------------------------
// 3. ADR index: every .ad_records/NNN-*.md has an index row; every row
//    points to an existing file.
// ---------------------------------------------------------------------------
check("adr-index-alignment", () => {
  const files = readdirSync(join(ROOT, ".ad_records"))
    .filter((name) => /^\d{3}-.*\.md$/.test(name))
    .sort();
  const index = read(".ad_records/README.md");
  const results = [];
  for (const file of files) {
    const number = file.slice(0, 3);
    if (!new RegExp(`^\\|\\s*${number}\\s*\\|`, "m").test(index)) {
      results.push({
        file: ".ad_records/README.md",
        text: `ADR ${file} exists but the index has no row for it`,
      });
    }
  }
  for (const row of index.matchAll(/^\| (\d{3}) \|/gm)) {
    if (!files.some((file) => file.startsWith(row[1]))) {
      results.push({
        file: ".ad_records/README.md",
        text: `index row ${row[1]} has no .ad_records/${row[1]}-*.md file`,
      });
    }
  }
  return results;
});

// ---------------------------------------------------------------------------
// 4. Research feature table: every .research/<feature>/ dir is listed in
//    .research/README.md; every table link points to an existing dir.
// ---------------------------------------------------------------------------
check("research-feature-alignment", () => {
  const features = readdirSync(join(ROOT, ".research"), { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name);
  const readme = read(".research/README.md");
  const results = [];
  for (const feature of features) {
    if (!readme.includes(`](${feature}/)`)) {
      results.push({
        file: ".research/README.md",
        text: `feature dir "${feature}" exists but the feature table has no row`,
      });
    }
  }
  for (const link of readme.matchAll(/\[([^\]]+)\]\(([^)/]+\/)\)/g)) {
    if (!features.includes(link[2].replace(/\/$/, ""))) {
      results.push({
        file: ".research/README.md",
        text: `feature table links "${link[2]}" but the directory does not exist`,
      });
    }
  }
  return results;
});

// ---------------------------------------------------------------------------
// 5. Hook shape: every hook referenced by settings.json exists on disk.
// ---------------------------------------------------------------------------
check("hook-shape", () => {
  const settings = JSON.parse(read(".claude/settings.json"));
  const referenced = [
    ...JSON.stringify(settings.hooks ?? {}).matchAll(/([A-Za-z0-9-]+\.(?:mjs|sh))/g),
  ].map((m) => m[1]);
  const results = [];
  for (const name of referenced) {
    if (!exists(join(".claude", "hooks", name))) {
      results.push({
        file: ".claude/settings.json",
        text: `referenced hook "${name}" missing on disk`,
      });
    }
  }
  return results;
});

// ---------------------------------------------------------------------------
// 6. Skill shape: every skills/<name>/ ships SKILL.md with name and
//    description frontmatter (the description is the trigger — a skill
//    without one never fires).
// ---------------------------------------------------------------------------
check("skill-shape", () => {
  const skillsDir = join(ROOT, ".claude", "skills");
  const results = [];
  for (const entry of readdirSync(skillsDir, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const skillRel = join(".claude", "skills", entry.name, "SKILL.md");
    if (!exists(skillRel)) {
      results.push({ file: `.claude/skills/${entry.name}/`, text: "missing SKILL.md" });
      continue;
    }
    const text = read(skillRel);
    if (!/^name:\s*\S+/m.test(text)) {
      results.push({ file: skillRel, text: "frontmatter has no name" });
    }
    if (!/^description:\s*\S+/m.test(text)) {
      results.push({ file: skillRel, text: "frontmatter has no description (the trigger)" });
    }
  }
  return results;
});

// ---------------------------------------------------------------------------
// 7. Rule frontmatter: path-scoped rules must declare their paths.
// ---------------------------------------------------------------------------
check("rule-frontmatter", () => {
  const expected = {
    "rust-dev.md": ["crates/**"],
    "adr-docs.md": [".ad_records/**"],
    "skill-authoring.md": [".claude/skills/**"],
  };
  const results = [];
  for (const [file, paths] of Object.entries(expected)) {
    const rel = join(".claude", "rules", file);
    if (!exists(rel)) {
      results.push({ file: rel, text: "rule file missing" });
      continue;
    }
    const text = read(rel);
    for (const p of paths) {
      if (!text.includes(`- "${p}"`)) {
        results.push({ file: rel, text: `frontmatter does not scope path "${p}"` });
      }
    }
  }
  return results;
});

// ---------------------------------------------------------------------------
// 8. Controlled-language terminology (docs-style.md term table). Forbidden
//    synonyms for fixed terms, enforced across the Chinese markdown corpus.
//    The rule file itself (docs-style.md) is exempt — it quotes the terms.
// ---------------------------------------------------------------------------
const TERM_RULES = [
  // [forbidden, correct term, exception pattern]
  { pattern: /门禁|闸门/, term: "检查门", exception: null },
  { pattern: /确认门/, term: "护栏", exception: null },
  { pattern: /单例锁/, term: "单实例互斥", exception: null },
  { pattern: /虚机/, term: "虚拟机", exception: null },
  { pattern: /危险操作/, term: "破坏性操作", exception: null },
];
function* markdownFiles(dirRel) {
  const abs = dirRel === "." ? ROOT : join(ROOT, dirRel);
  let entries;
  try {
    entries = readdirSync(abs, { withFileTypes: true });
  } catch {
    return;
  }
  for (const entry of entries) {
    if (entry.name === "node_modules" || entry.name === "target") continue;
    if (dirRel === "." && entry.name.startsWith(".") && ![".ad_records", ".research", ".claude", ".review"].includes(entry.name)) {
      continue;
    }
    const rel = dirRel === "." ? entry.name : `${dirRel}/${entry.name}`;
    if (entry.isDirectory()) {
      if (rel.endsWith("sessions") || rel.endsWith("memory") || rel.endsWith("plans")) continue;
      yield* markdownFiles(rel);
    } else if (entry.name.endsWith(".md")) {
      if (entry.name === "docs-style.md") continue; // the rule quotes forbidden terms
      yield rel;
    }
  }
}
check("docs-terminology", () => {
  const results = [];
  const seen = new Set();
  for (const dir of [".", ".ad_records", ".research", ".claude", ".review"]) {
    for (const rel of markdownFiles(dir)) {
      if (seen.has(rel)) continue;
      seen.add(rel);
      const text = read(rel);
      for (const rule of TERM_RULES) {
        for (const match of text.matchAll(new RegExp(rule.pattern.source, "g"))) {
          const line = text.slice(0, match.index).split("\n").length;
          results.push({
            file: rel,
            text: `line ${line}: "${match[0]}" must be "${rule.term}" (docs-style.md term table)`,
          });
        }
      }
    }
  }
  return results;
});

// ---------------------------------------------------------------------------
// 9. Credential red line: pve-mcp.toml (the real config) must never be
//    tracked by git, its password value must appear in no other file, and
//    the example must carry placeholders only.
// ---------------------------------------------------------------------------
check("credential-redline", () => {
  const results = [];
  // 9a. Real config untracked (git check; skip when git is absent).
  try {
    const tracked = execFileSync("git", ["ls-files", "pve-mcp.toml"], {
      cwd: ROOT,
      encoding: "utf8",
    }).trim();
    if (tracked.length > 0) {
      results.push({ file: "pve-mcp.toml", text: "gitignored config is TRACKED by git — untrack immediately" });
    }
  } catch {
    results.push({ file: "pve-mcp.toml", text: "git check failed (no git or no repo)" });
  }
  // 9b. The real password must not leak into any repo file.
  if (exists("pve-mcp.toml")) {
    const real = read("pve-mcp.toml");
    const m = real.match(/^password\s*=\s*"([^"]+)"/m);
    if (m && !m[1].includes("PLACEHOLDER")) {
      const secret = m[1];
      const scan = (dirRel) => {
        const abs = dirRel === "." ? ROOT : join(ROOT, dirRel);
        let entries;
        try {
          entries = readdirSync(abs, { withFileTypes: true });
        } catch {
          return;
        }
        for (const entry of entries) {
          if ([".git", "target", "node_modules", ".reference"].includes(entry.name)) continue;
          const rel = dirRel === "." ? entry.name : `${dirRel}/${entry.name}`;
          if (entry.isDirectory()) {
            scan(rel);
          } else if (/\.(rs|toml|md|mjs|sh|json|example)$/.test(entry.name) || entry.name === "justfile") {
            if (rel === "pve-mcp.toml") continue;
            let text;
            try {
              text = readFileSync(join(ROOT, rel), "utf8");
            } catch {
              continue;
            }
            if (text.includes(secret)) {
              results.push({ file: rel, text: "contains the real PVE password (credential red line)" });
            }
          }
        }
      };
      scan(".");
    }
  }
  // 9c. The example must not carry real-looking secrets.
  if (exists("pve-mcp.toml.example")) {
    const example = read("pve-mcp.toml.example");
    for (const match of example.matchAll(/^(?:password|token_value)\s*=\s*"([^"]*)"/gm)) {
      if (match[1].length > 0 && !/^(|xxx+|<[^>]*>|your-.*|test-.*)$/.test(match[1])) {
        results.push({
          file: "pve-mcp.toml.example",
          text: `example carries a non-placeholder value in "${match[0].split(":")[0]}"`,
        });
      }
    }
  }
  return results;
});

// ---------------------------------------------------------------------------
// 10. CLAUDE.md freshness: the status section must anchor the gate and the
//     config template (navigation entries every session relies on).
// ---------------------------------------------------------------------------
check("claude-md-current", () => {
  const text = read("CLAUDE.md");
  const results = [];
  for (const needle of ["just ci", "pve-mcp.toml"]) {
    if (!text.includes(needle)) {
      results.push({ file: "CLAUDE.md", text: `does not mention "${needle}"` });
    }
  }
  return results;
});

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------
for (const finding of findings) {
  console.error(`[system-lint] ${finding.check}: ${finding.file}: ${finding.text}`);
}
if (findings.length > 0) {
  console.error(`[system-lint] ${findings.length} finding(s) across ${checked} check(s)`);
  process.exit(1);
}
console.log(`[system-lint] ${checked} checks passed`);
