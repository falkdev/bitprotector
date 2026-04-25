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

fn read_existing_file(relative_path: &str) -> String {
    let path = project_root().join(relative_path);
    assert!(path.exists(), "File must exist at {relative_path}");
    std::fs::read_to_string(path).unwrap()
}

fn assert_qemu_wrapper_delegates(wrapper_relative_path: &str, bundle_file_name: &str) {
    let wrapper = read_existing_file(wrapper_relative_path);
    assert!(
        wrapper.contains("exec"),
        "Wrapper script {wrapper_relative_path} must exec into a bundle"
    );
    assert!(
        wrapper.contains(bundle_file_name),
        "Wrapper script {wrapper_relative_path} must delegate to {bundle_file_name}"
    );
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
    assert_qemu_wrapper_delegates("tests/installation/qemu_test.sh", "bundles/smoke.sh");

    let content = read_existing_file("tests/installation/bundles/smoke.sh");
    assert!(
        content.contains("qemu-system-x86_64"),
        "Smoke bundle must invoke qemu-system-x86_64"
    );
    assert!(
        content.contains("bitprotector*.deb"),
        "Smoke bundle must install the .deb package"
    );
}

#[test]
fn test_qemu_failover_script_exists() {
    assert_qemu_wrapper_delegates(
        "tests/installation/qemu_failover_test.sh",
        "bundles/failover.sh",
    );

    let content = read_existing_file("tests/installation/bundles/failover.sh");
    assert!(
        content.contains("qmp"),
        "Failover bundle must use a QMP control socket"
    );

    let planned =
        read_existing_file("tests/installation/scenarios/failover/failover-01-planned.sh");
    assert!(
        planned.contains("drives replace confirm"),
        "Failover planned scenario must exercise the replacement workflow"
    );

    let emergency = read_existing_file(
        "tests/installation/scenarios/failover/failover-12-qmp-hot-remove-secondary.sh",
    );
    assert!(
        emergency.contains("qmp_device_del"),
        "Failover emergency scenario must hot-remove a disk for coverage"
    );
}

#[test]
fn test_qemu_uninstall_script_exists() {
    assert_qemu_wrapper_delegates(
        "tests/installation/qemu_uninstall_test.sh",
        "bundles/uninstall.sh",
    );

    let purge_scenario =
        read_existing_file("tests/installation/scenarios/uninstall/uninstall-03-purge.sh");
    assert!(
        purge_scenario.contains("apt-get purge -y bitprotector"),
        "Uninstall purge scenario must purge the package"
    );
    assert!(
        purge_scenario.contains("/var/lib/bitprotector"),
        "Uninstall purge scenario must assert package-owned data removal"
    );

    let create_data_scenario =
        read_existing_file("tests/installation/scenarios/uninstall/uninstall-02-create-data.sh");
    assert!(
        create_data_scenario.contains("database run"),
        "Uninstall create-data scenario must create a real backup artifact before purge"
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
