# bdd-netlink

Ultra-fast Linux Netlink process connector (`AF_NETLINK` / `CN_IDX_PROC`) binary stream generator for [`bdd`](https://github.com/e-t-u/bdd).

Subscribes directly to kernel process lifecycle multicast notifications (`PROC_CN_MCAST_LISTEN`) and streams raw binary event structs (`proc_event`: fork, exec, exit, uid, gid, comm) straight to stdout.

## Features

- **Zero-copy & ultra-fast**: Direct `libc` socket syscalls with up to 8 MiB socket receive buffer and 64 KiB buffered I/O.
- **Kernel framing stripping**: Automatically strips the 36-byte framing headers (`nlmsghdr` 16B + `cn_msg` 20B) by default, outputting clean binary `proc_event` records (16-byte header + event payload).
- **In-flight event filtering**: `--filter <EVENT>` directly drops non-matching events before serialization (options: `fork`, `exec`, `exit`, `uid`, `gid`, `comm`, `all`).
- **Clean signal handling**: Sends `PROC_CN_MCAST_IGNORE` to unregister from the kernel multicast group on `SIGINT` / `SIGTERM`.

## Usage

```bash
# Build
cargo build --release

# Stream live process events directly into bdd for structured JSON inspection:
sudo ./target/release/bdd-netlink | bdd --preset netlink-proc-event --output-json

# Stream only exec (new process execution) events to terminal visual inspector:
sudo ./target/release/bdd-netlink --filter exec | bdd "netlink-proc-event -> visual"

# Capture 10 process exit events into JSON objects:
sudo ./target/release/bdd-netlink --filter exit --count 10 | bdd "netlink-proc-event -> json:object"
```

## Options

- `-r, --raw`: Emit raw Netlink packets including the 36-byte framing headers.
- `-c, --count <N>`: Stop after emitting N events (0 = infinite).
- `-f, --filter <EVENT>`: Filter for `all`, `fork`, `exec`, `exit`, `uid`, `gid`, `comm`.
- `-b, --buffer-size <BYTES>`: Socket receive buffer size in bytes (default: 8 MiB).
- `-h, --help`: Display help and examples.
