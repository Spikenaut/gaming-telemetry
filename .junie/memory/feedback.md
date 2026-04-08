[2026-04-08 17:38] - Updated by Junie
{
    "TYPE": "positive",
    "CATEGORY": "successful run",
    "EXPECTATION": "The program compiled and ran with `cargo run` on the first try without issues.",
    "NEW INSTRUCTION": "WHEN proposing Rust code or project changes THEN ensure `cargo run` works on first attempt and note verification"
}

[2026-04-08 17:48] - Updated by Junie
{
    "TYPE": "negative",
    "CATEGORY": "failing tests",
    "EXPECTATION": "They expected the project’s tests to pass; reporting only build/run success wasn’t sufficient.",
    "NEW INSTRUCTION": "WHEN proposing Rust code or project changes THEN ensure `cargo test` passes and note verification"
}

[2026-04-08 18:21] - Updated by Junie
{
    "TYPE": "correction",
    "CATEGORY": "license and gitignore",
    "EXPECTATION": "They want the project licensed under GPL-3.0 and to include a proper .gitignore.",
    "NEW INSTRUCTION": "WHEN initializing or updating repo metadata THEN set license to GPL-3.0 and add Rust .gitignore"
}

