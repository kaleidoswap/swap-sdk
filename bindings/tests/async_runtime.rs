//! Every async UniFFI export must name the tokio runtime.
//!
//! UniFFI polls an exported `async fn` on its own executor. That executor has
//! no reactor, so the first reqwest/tokio I/O inside the future aborts the
//! host process with "there is no reactor running, must be called from the
//! context of a Tokio 1.x runtime" — a panic across the FFI boundary, not a
//! `Result` a caller can handle. `#[uniffi::export(async_runtime = "tokio")]`
//! is what supplies the reactor.
//!
//! Nothing in the type system enforces that, and the failure only shows up at
//! runtime on the first call that touches the network — which is why
//! `ChainClient::broadcast_tx` shipped without it: the binding tests that would
//! have caught it (`tests/bindings/*.py`) need an external regtest stack, so
//! they do not run in CI. This test needs nothing but the source.

use std::fs;
use std::path::Path;

/// Every `#[uniffi::export…] impl` block in the crate, as
/// `(file, line, attribute_args, body)`.
fn export_blocks(source: &str) -> Vec<(usize, String, String)> {
    const MARKER: &str = "#[uniffi::export";
    let mut blocks = Vec::new();
    let mut cursor = 0;

    while let Some(found) = source[cursor..].find(MARKER) {
        let attr_start = cursor + found;
        let Some(attr_end) = source[attr_start..].find("]\n") else {
            break;
        };
        let attr_end = attr_start + attr_end + 1;
        let args = source[attr_start + MARKER.len()..attr_end].to_owned();
        cursor = attr_end;

        // Only `impl` blocks carry methods; `#[uniffi::export]` on a free
        // function cannot be async in these bindings, and a `#[uniffi::remote]`
        // block has no bodies of ours at all.
        let rest = source[attr_end..].trim_start();
        if !rest.starts_with("impl") {
            continue;
        }
        let block_start = attr_end + source[attr_end..].find('{').expect("impl block brace");
        let mut depth = 0usize;
        let mut end = block_start;
        for (offset, byte) in source[block_start..].bytes().enumerate() {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = block_start + offset;
                        break;
                    }
                }
                _ => {}
            }
        }
        let line = source[..attr_start].lines().count() + 1;
        blocks.push((line, args, source[block_start..=end].to_owned()));
    }
    blocks
}

#[test]
fn every_async_export_runs_on_the_tokio_runtime() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut checked = 0usize;
    let mut offenders = Vec::new();

    for entry in fs::read_dir(&src_dir).expect("read bindings/src") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read binding source");
        for (line, args, body) in export_blocks(&source) {
            if !body.contains("async fn") {
                continue;
            }
            checked += 1;
            if !args.contains("async_runtime") {
                offenders.push(format!(
                    "{}:{line} — async export without `async_runtime = \"tokio\"`",
                    path.file_name().unwrap().to_string_lossy(),
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "async UniFFI exports missing the tokio runtime:\n  {}",
        offenders.join("\n  ")
    );
    // A parser that silently matched nothing would pass vacuously forever.
    assert!(
        checked >= 4,
        "expected to find the async export blocks (BoltzApiClientV2, BoltzWsApi, \
         BoltzWsUpdates, SwapScript, ChainClient); found {checked}"
    );
}
