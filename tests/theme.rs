mod common;

use common::Sandbox;
use predicates::prelude::*;
use predicates::str::contains;

#[test]
fn theme_without_migrate_flag_errors() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();

    sb.command()
        .arg("theme")
        .assert()
        .failure()
        .stderr(contains("No action specified").and(contains("theme --migrate")));
}

/// Simulates a theme.toml written by an older release: only a few keys
/// exist, one deliberately customized per table. `--migrate` must restore
/// everything that's missing while leaving the custom values alone.
#[test]
fn theme_migrate_restores_removed_keys_and_preserves_custom_values() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();

    std::fs::write(
        sb.theme_path(),
        "\
[theme_colors]
normal_row_bg = '#123456'

[text_colors]
status_open = 'red'

[theme_styles]
highlight_symbol = '>'
",
    )
    .unwrap();

    sb.command()
        .args(["theme", "--migrate"])
        .assert()
        .success()
        .stdout(contains(
            "Re-serialized theme.toml with all current keys and defaults",
        ));

    let raw = std::fs::read_to_string(sb.theme_path()).unwrap();
    let parsed: toml::Value = toml::from_str(&raw).expect("migrated theme should be valid TOML");

    // Custom values survive through the migration. Note: ratatui
    // canonicalizes NAMED colors when serializing ('red' -> "Red"); hex
    // strings like '#123456' round-trip verbatim.
    assert_eq!(
        parsed["theme_colors"]["normal_row_bg"].as_str(),
        Some("#123456"),
        "custom theme_colors value must survive"
    );
    assert_eq!(
        parsed["text_colors"]["status_open"].as_str(),
        Some("Red"),
        "custom text_colors value must survive (named colors canonicalize to Capitalized)"
    );
    assert_eq!(
        parsed["theme_styles"]["highlight_symbol"].as_str(),
        Some(">"),
        "custom theme_styles value must survive"
    );

    // ...and previously missing keys come back (with defaults filled in).
    assert!(
        parsed["text_colors"]["status_working"].is_str(),
        "removed text_colors key should be restored"
    );
    assert!(
        parsed["theme_colors"]["alt_row_bg"].is_str(),
        "removed theme_colors key should be restored"
    );
    assert_eq!(
        parsed["theme_styles"]["urgency_low"].as_str(),
        Some("   "),
        "missing theme_styles key should be restored to its default"
    );
}
