import { readFileSync, writeFileSync } from "node:fs";

const p = "crates/pve-server/tests/dispatch_mock_test.rs";
let s = readFileSync(p, "utf8");
const old = `    std::fs::write(
        &config_path,
        format!(
            "{}\\n[security]\\naccess_tier = \\"read_only\\"\\n",
            TEST_CONFIG_BASE.replace("{PORT}", &addr.port().to_string())
        ),
    )?;`;
const neu = `    std::fs::write(
        &config_path,
        format!(
            "{}\\naccess_tier = \\"read_only\\"\\n",
            TEST_CONFIG_BASE.replace("{PORT}", &addr.port().to_string())
                .trim_end()
        ),
    )?;`;
if (!s.includes(old)) { console.error("anchor missing"); process.exit(1); }
s = s.replace(old, neu, 1);
writeFileSync(p, s);
console.log("ok");
