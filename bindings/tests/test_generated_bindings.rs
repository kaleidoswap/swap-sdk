use serial_test::serial;

fn run_test(script: &str) {
    uniffi::python_test::run_test(
        std::env!("CARGO_TARGET_TMPDIR"),
        std::env!("CARGO_PKG_NAME"),
        script,
    )
    .unwrap();
}

#[test]
#[serial]
fn reverse() {
    run_test("tests/bindings/reverse.py");
}

#[test]
#[serial]
fn chain() {
    run_test("tests/bindings/chain.py");
}

#[test]
#[serial]
fn submarine() {
    run_test("tests/bindings/submarine.py");
}

#[test]
#[serial]
fn typed_swap_dtos() {
    run_test("tests/bindings/typed_swap_dtos.py");
}

#[test]
#[serial]
fn swap_script_restore() {
    run_test("tests/bindings/swap_script_restore.py");
}

#[test]
#[serial]
fn kaleido_auth() {
    run_test("tests/bindings/kaleido_auth.py");
}

#[test]
#[serial]
fn auth_recovery() {
    run_test("tests/bindings/auth_recovery.py");
}

#[test]
#[serial]
fn lightning_address() {
    run_test("tests/bindings/lightning_address.py");
}

#[test]
#[serial]
fn corridor_card() {
    run_test("tests/bindings/corridor_card.py");
}

/// RGB create and spend bindings use a local HTTP stub, without regtest daemons.
#[test]
#[serial]
fn rgb() {
    run_test("tests/bindings/rgb.py");
}

#[test]
#[serial]
fn transaction_restore() {
    run_test("tests/bindings/transaction_restore.py");
}
