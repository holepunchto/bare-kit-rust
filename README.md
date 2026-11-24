# bare-kit-rust

Rust bindings for [Bare Kit](https://github.com/holepunchto/bare-kit), which runs Bare in a thread of its own within the application, known as a worklet, with an IPC to it exposed as `Bare.IPC` inside the worklet.

```
cargo add bare-kit
```

## Usage

```rust
use std::io::{Read, Write};

fn main() -> std::io::Result<()> {
    let mut worklet = bare_kit::start("app.bundle")?;

    worklet.write_all(b"Hello world")?;

    let mut data = String::new();
    worklet.read_to_string(&mut data)?;

    println!("{data}");

    Ok(())
}
```

Where `app.bundle` is a Bare bundle using `Bare.IPC`:

```js
Bare.IPC.on('data', (data) => Bare.IPC.end(data))
```

Reading a worklet yields the data that it writes to `Bare.IPC` until it ends its side. As `Bare.IPC` keeps the worklet alive, `end()` ends the data sent to it so that it may exit on its own, and `wait()` returns its exit code. Writes fail with `io::ErrorKind::BrokenPipe` once the data sent has ended or the worklet has closed its side. Dropping a worklet terminates it, so keep it alive for as long as it should run.

These bindings are a thin layer over [Bare Kit](https://github.com/holepunchto/bare-kit), whose README covers the [lifecycle](https://github.com/holepunchto/bare-kit#lifecycle), [IPC](https://github.com/holepunchto/bare-kit#ipc), [suspension](https://github.com/holepunchto/bare-kit#suspension), [addons](https://github.com/holepunchto/bare-kit#addons), and [error handling](https://github.com/holepunchto/bare-kit#error-handling) of a worklet. In Rust terms:

- Without `Builder::source()`, `start()` reads `filename` at runtime, so the worklet may be a module next to its `node_modules` rather than a bundle.
- The crate links Bare Kit statically, so native addons resolve the symbols of Bare from the executable, which must export them as described in [addons](https://github.com/holepunchto/bare-kit#addons).
- Reads and writes block the calling thread until the IPC is ready, and as `&Worklet` reads and writes too, one thread may read while another writes.
- An uncaught error in the worklet aborts the process, the Rust program included, so report errors over `Bare.IPC` as described in [error handling](https://github.com/holepunchto/bare-kit#error-handling).

## Platforms

The crate supports Android, iOS, Linux, macOS, and Windows, and links Bare Kit statically. Its build script downloads the Bare Kit prebuild for the target from the release of the crate version. To use prebuilds of your own, set `BARE_KIT_DIR` to a directory of them, or `BARE_KIT_MIRROR` to a URL serving `v<version>/<target>.tar.gz`. A relative filename names a resource of the application. On Android, the Java virtual machine and application context are passed with `Builder::android()`.

Bare Kit links the static C runtime on Windows, so the application must too. Enable it in `.cargo/config.toml`:

```toml
[target.'cfg(all(windows, target_env = "msvc"))']
rustflags = ["-C", "target-feature=+crt-static"]
```

## License

Apache-2.0
