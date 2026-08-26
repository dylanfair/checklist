mod common;

use crate::common::Sandbox;

#[test]
fn first_init_creates_config_dir_files() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();
    assert!(sb.db_path().exists());
    assert!(sb.config_path().exists());
    assert!(sb.theme_path().exists());
}
