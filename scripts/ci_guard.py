#!/usr/bin/env python3
"""CI guard script for theMQL.

Checks:
1. All TOML files parse successfully.
2. Every crate in specs/*.toml has a matching workspace member.
3. Every workspace member crate has a matching spec file.
4. Crate names in Cargo.toml match the directory name.

Exit code 0 = all checks pass. Non-zero = failure.
"""

import pathlib
import sys
import tomllib

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
SPECS_DIR = REPO_ROOT / "specs"
CRATES_DIR = REPO_ROOT / "crates"


def check_toml_parse() -> list[str]:
    """Check all TOML files parse."""
    errors = []
    for p in REPO_ROOT.rglob("*.toml"):
        if ".git" in p.parts or "target" in p.parts:
            continue
        try:
            tomllib.loads(p.read_text())
        except Exception as e:
            errors.append(f"  TOML parse error in {p.relative_to(REPO_ROOT)}: {e}")
    return errors


def check_crate_name_matches_dir() -> list[str]:
    """Check that crate names in Cargo.toml match directory names."""
    errors = []
    for cargo_toml in CRATES_DIR.glob("*/Cargo.toml"):
        data = tomllib.loads(cargo_toml.read_text())
        name = data.get("package", {}).get("name", "")
        dir_name = cargo_toml.parent.name
        if name != dir_name:
            errors.append(
                f"  Crate name mismatch: {cargo_toml.relative_to(REPO_ROOT)} "
                f"has name='{name}' but dir='{dir_name}'"
            )
    return errors


def check_spec_crate_names() -> list[str]:
    """Check that spec crate names match workspace members."""
    errors = []
    workspace_toml = tomllib.loads((REPO_ROOT / "Cargo.toml").read_text())
    members = workspace_toml.get("workspace", {}).get("members", [])
    member_names = set()
    for m in members:
        member_path = REPO_ROOT / m / "Cargo.toml"
        if member_path.exists():
            data = tomllib.loads(member_path.read_text())
            member_names.add(data.get("package", {}).get("name", ""))

    spec_crate_names = set()
    for spec in SPECS_DIR.glob("*.toml"):
        data = tomllib.loads(spec.read_text())
        crate_name = data.get("subsystem", {}).get("crate", "")
        if crate_name:
            spec_crate_names.add(crate_name)

    for name in spec_crate_names - member_names:
        errors.append(
            f"  Spec references crate '{name}' but no workspace member found"
        )
    for name in member_names - spec_crate_names:
        errors.append(
            f"  Workspace member '{name}' has no matching spec file"
        )

    return errors


def main() -> int:
    all_errors: list[str] = []

    errors = check_toml_parse()
    if errors:
        print("FAIL: TOML parse errors:")
        for e in errors:
            print(e)
        all_errors.extend(errors)
    else:
        print("OK: All TOML files parse.")

    errors = check_crate_name_matches_dir()
    if errors:
        print("FAIL: Crate name mismatches:")
        for e in errors:
            print(e)
        all_errors.extend(errors)
    else:
        print("OK: All crate names match directory names.")

    errors = check_spec_crate_names()
    if errors:
        print("FAIL: Spec/workspace member mismatches:")
        for e in errors:
            print(e)
        all_errors.extend(errors)
    else:
        print("OK: All spec crate names match workspace members.")

    if all_errors:
        print(f"\n{len(all_errors)} error(s) found.")
        return 1
    print("\nAll checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())