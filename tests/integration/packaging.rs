/// Packaging integration tests for Milestone 13.
///
/// These tests verify that all packaging artifacts are in place and
/// the cargo-deb configuration is valid. The actual QEMU-based installation
/// plus failover/uninstall tests run via tests/installation/qemu_test.sh,
/// tests/installation/qemu_failover_test.sh, and
/// tests/installation/qemu_uninstall_test.sh.
use std::path::Path;

fn project_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

#[test]
fn test_systemd_service_file_exists() {
    let service = project_root().join("packaging/bitprotector.service");
    assert!(
        service.exists(),
        "systemd service file must exist at packaging/bitprotector.service"
    );
    let content = std::fs::read_to_string(&service).unwrap();
    assert!(
        content.contains("[Unit]"),
        "Service file must have [Unit] section"
    );
    assert!(
        content.contains("[Service]"),
        "Service file must have [Service] section"
    );
    assert!(
        content.contains("[Install]"),
        "Service file must have [Install] section"
    );
    assert!(
        content.contains("ExecStart="),
        "Service file must have ExecStart"
    );
}

#[test]
fn test_default_config_file_exists() {
    let config = project_root().join("packaging/config.toml");
    assert!(
        config.exists(),
        "Default config file must exist at packaging/config.toml"
    );
    let content = std::fs::read_to_string(&config).unwrap();
    assert!(
        content.contains("[server]"),
        "Config must have [server] section"
    );
    assert!(
        content.contains("[database]"),
        "Config must have [database] section"
    );
    assert!(
        content.contains("jwt_secret"),
        "Config must include JWT auth configuration"
    );
}

#[test]
fn test_profile_d_hook_exists() {
    let hook = project_root().join("scripts/bitprotector-status.sh");
    assert!(
        hook.exists(),
        "Profile.d hook must exist at scripts/bitprotector-status.sh"
    );
    let content = std::fs::read_to_string(&hook).unwrap();
    assert!(
        content.contains("bitprotector"),
        "Hook must invoke bitprotector"
    );
    assert!(
        content.contains("status"),
        "Hook must call the status subcommand"
    );
}

#[test]
fn test_qemu_install_script_exists() {
    let script = project_root().join("tests/installation/qemu_test.sh");
    assert!(
        script.exists(),
        "QEMU test script must exist at tests/installation/qemu_test.sh"
    );
    let content = std::fs::read_to_string(&script).unwrap();
    assert!(
        content.contains("qemu-system"),
        "Script must invoke qemu-system"
    );
    assert!(
        content.contains("bitprotector.deb"),
        "Script must install the .deb package"
    );
}

#[test]
fn test_qemu_failover_script_exists() {
    let script = project_root().join("tests/installation/qemu_failover_test.sh");
    assert!(
        script.exists(),
        "QEMU failover test script must exist at tests/installation/qemu_failover_test.sh"
    );
    let content = std::fs::read_to_string(&script).unwrap();
    assert!(
        content.contains("qmp"),
        "Failover script must use a QMP control socket"
    );
    assert!(
        content.contains("drives replace confirm"),
        "Failover script must exercise the replacement workflow"
    );
    assert!(
        content.contains("device_del"),
        "Failover script must hot-remove a disk for emergency failover coverage"
    );
}

#[test]
fn test_qemu_uninstall_script_exists() {
    let script = project_root().join("tests/installation/qemu_uninstall_test.sh");
    assert!(
        script.exists(),
        "QEMU uninstall test script must exist at tests/installation/qemu_uninstall_test.sh"
    );
    let content = std::fs::read_to_string(&script).unwrap();
    assert!(
        content.contains("apt-get purge -y bitprotector"),
        "Uninstall script must purge the package"
    );
    assert!(
        content.contains("/var/lib/bitprotector"),
        "Uninstall script must assert package-owned data removal"
    );
    assert!(
        content.contains("database run"),
        "Uninstall script must create a real backup artifact before purge"
    );
}

#[test]
fn test_cargo_deb_metadata_present() {
    let cargo_toml = project_root().join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).unwrap();
    assert!(
        content.contains("[package.metadata.deb]"),
        "Cargo.toml must have [package.metadata.deb] section"
    );
    assert!(
        content.contains("bitprotector.service"),
        "Cargo.toml deb config must reference systemd service file"
    );
    assert!(
        content.contains("bitprotector-status.sh"),
        "Cargo.toml deb config must reference profile.d hook"
    );
    assert!(
        content.contains("config.toml"),
        "Cargo.toml deb config must reference config template"
    );
    assert!(
        content.contains("frontend/dist/**/*"),
        "Cargo.toml deb config must package the built frontend assets"
    );
    assert!(
        content.contains("var/lib/bitprotector/frontend"),
        "Cargo.toml deb config must install frontend files under /var/lib/bitprotector/frontend"
    );
}

#[test]
fn test_postinst_script_exists() {
    let postinst = project_root().join("packaging/scripts/postinst");
    assert!(postinst.exists(), "postinst script must exist");
    let content = std::fs::read_to_string(&postinst).unwrap();
    assert!(
        content.contains("bitprotector"),
        "postinst must set up bitprotector user"
    );
    assert!(
        content.contains("/var/lib/bitprotector"),
        "postinst must create data directory"
    );
    assert!(
        content.contains("/var/lib/bitprotector/frontend"),
        "postinst must create the frontend asset directory"
    );
}

#[test]
fn test_postrm_script_exists() {
    let postrm = project_root().join("packaging/scripts/postrm");
    assert!(postrm.exists(), "postrm script must exist");
    let content = std::fs::read_to_string(&postrm).unwrap();
    assert!(
        content.contains("purge"),
        "postrm must handle purge actions"
    );
    assert!(
        content.contains("/var/lib/bitprotector"),
        "postrm must remove package-owned data directory on purge"
    );
    assert!(
        content.contains("/var/log/bitprotector"),
        "postrm must remove package-owned log directory on purge"
    );
    assert!(
        content.contains("/etc/bitprotector"),
        "postrm must remove package-owned config directory on purge"
    );
}
