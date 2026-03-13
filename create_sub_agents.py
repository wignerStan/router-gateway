import os

sub_projects = [
    ('apps/cli', 'my-cli'),
    ('apps/gateway', 'gateway'),
    ('packages/smart-routing', 'smart-routing'),
    ('packages/model-registry', 'model-registry'),
    ('packages/tracing', 'llm-tracing'),
]

for path, name in sub_projects:
    # 1. AGENTS.md
    with open(f"{path}/AGENTS.md", "w") as f:
        f.write(f"""# {name}

This project is part of the workspace. Please refer to the root [AGENTS.md](../../AGENTS.md) for global project guidelines and best practices.

## Crate-Specific Instructions
- **Location:** `{path}`
- **Build:** Run `cargo build -p {name}`
- **Test:** Run `cargo test -p {name}`
""")

    # 2. CLAUDE.md
    with open(f"{path}/CLAUDE.md", "w") as f:
        f.write("@AGENTS.md\n")

    # 3. GEMINI.md
    with open(f"{path}/GEMINI.md", "w") as f:
        f.write("@AGENTS.md\n")
