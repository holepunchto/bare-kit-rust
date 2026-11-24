use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use bare_kit::{Worklet, start};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn read_to_end(mut worklet: &Worklet) -> Vec<u8> {
    let mut data = Vec::new();
    worklet.read_to_end(&mut data).unwrap();
    data
}

#[test]
fn echoes_ipc_data() {
    let mut worklet = start(fixture("echo.js")).unwrap();

    worklet.write_all(b"Hello world").unwrap();

    let mut buf = [0; 11];
    worklet.read_exact(&mut buf).unwrap();

    assert_eq!(&buf, b"Hello world");
}

#[test]
fn ends_ipc_data() {
    let mut worklet = start(fixture("collect.js")).unwrap();

    worklet.write_all(b"Hello ").unwrap();
    worklet.write_all(b"world").unwrap();
    worklet.end();

    assert_eq!(read_to_end(&worklet), b"Hello world");
    assert_eq!(worklet.wait(), 0);
}

#[test]
fn passes_arguments() {
    let worklet = Worklet::builder()
        .args(["foo", "bar"])
        .start(fixture("argv.js"))
        .unwrap();

    assert_eq!(read_to_end(&worklet), br#"["foo","bar"]"#);
}

#[test]
fn runs_source() {
    let worklet = Worklet::builder()
        .source("Bare.IPC.end('Hello world')")
        .start("/app.js")
        .unwrap();

    assert_eq!(read_to_end(&worklet), b"Hello world");
}

#[test]
fn reports_the_exit_code() {
    let worklet = start(fixture("exit.js")).unwrap();

    assert_eq!(worklet.wait(), 3);
    assert_eq!(read_to_end(&worklet), b"");
    assert_eq!(worklet.exit_code(), Some(3));
}

#[test]
fn terminates() {
    let worklet = start(fixture("sleep.js")).unwrap();

    worklet.terminate();
    worklet.wait();

    assert_eq!(read_to_end(&worklet), b"");
}

#[test]
fn suspends_and_resumes() {
    let mut worklet = start(fixture("lifecycle.js")).unwrap();

    let mut events = Vec::new();
    let mut buf = [0; 64];

    while events.len() < 3 {
        let n = worklet.read(&mut buf).unwrap();
        let event = String::from_utf8(buf[..n].to_vec()).unwrap();

        match event.as_str() {
            "ready" => worklet.suspend(Some(Duration::ZERO)),
            "suspend" => worklet.resume(),
            _ => {}
        }

        events.push(event);
    }

    assert_eq!(events, ["ready", "suspend", "resume"]);
}

#[test]
fn streams_large_data() {
    let data: Vec<u8> = (0..=255).cycle().take(256 * 4096).collect();

    let worklet = start(fixture("collect.js")).unwrap();

    std::thread::scope(|scope| {
        scope.spawn(|| {
            (&worklet).write_all(&data).unwrap();
            worklet.end();
        });

        assert_eq!(read_to_end(&worklet), data);
    });
}

#[test]
fn loads_dynamic_addons() {
    let worklet = start(fixture("addon.js")).unwrap();

    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        os => os,
    };

    assert_eq!(read_to_end(&worklet), platform.as_bytes());
}
