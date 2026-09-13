//! bdd-netlink: Ultra-fast Linux Netlink process connector binary stream generator.
//!
//! Subscribes to the Linux kernel NETLINK_CONNECTOR (CN_IDX_PROC) multicast socket
//! and streams raw binary process lifecycle events directly to stdout.
//! Designed to pipe zero-copy binary data directly into `bdd` pipelines:
//!
//! ```bash
//! sudo ./bdd-netlink | bdd --preset netlink-proc-event --output-json
//! sudo ./bdd-netlink --filter exec | bdd "netlink-proc-event -> visual"
//! sudo ./bdd-netlink --count 10 | bdd "netlink-proc-event -> json:object"
//! ```

use std::env;
use std::io::{self, Write};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};

static RUNNING: AtomicBool = AtomicBool::new(true);

extern "C" fn sig_handler(_: libc::c_int) {
    RUNNING.store(false, Ordering::SeqCst);
}

// Netlink & Connector constants
const NETLINK_CONNECTOR: libc::c_int = 11;
const SOL_NETLINK: libc::c_int = 270;
const NETLINK_NO_ENOBUFS: libc::c_int = 5;

// Linux Kernel proc_event event types (proc_event.what)
#[allow(dead_code)]
const PROC_EVENT_NONE: u32 = 0x00000000;
const PROC_EVENT_FORK: u32 = 0x00000001;
const PROC_EVENT_EXEC: u32 = 0x00000002;
const PROC_EVENT_UID: u32 = 0x00000004;
const PROC_EVENT_GID: u32 = 0x00000040;
const PROC_EVENT_SID: u32 = 0x00000080;
const PROC_EVENT_PTRACE: u32 = 0x00000100;
const PROC_EVENT_COMM: u32 = 0x00000200;
const PROC_EVENT_COREDUMP: u32 = 0x00000400;
const PROC_EVENT_EXIT: u32 = 0x80000000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventFilter {
    All,
    Fork,
    Exec,
    Exit,
    Uid,
    Gid,
    Sid,
    Ptrace,
    Comm,
    Coredump,
}

impl EventFilter {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "all" => Some(Self::All),
            "fork" => Some(Self::Fork),
            "exec" => Some(Self::Exec),
            "exit" => Some(Self::Exit),
            "uid" => Some(Self::Uid),
            "gid" => Some(Self::Gid),
            "sid" => Some(Self::Sid),
            "ptrace" => Some(Self::Ptrace),
            "comm" => Some(Self::Comm),
            "coredump" => Some(Self::Coredump),
            _ => None,
        }
    }

    fn matches(&self, what: u32) -> bool {
        match self {
            Self::All => true,
            Self::Fork => what == PROC_EVENT_FORK,
            Self::Exec => what == PROC_EVENT_EXEC,
            Self::Exit => what == PROC_EVENT_EXIT,
            Self::Uid => what == PROC_EVENT_UID,
            Self::Gid => what == PROC_EVENT_GID,
            Self::Sid => what == PROC_EVENT_SID,
            Self::Ptrace => what == PROC_EVENT_PTRACE,
            Self::Comm => what == PROC_EVENT_COMM,
            Self::Coredump => what == PROC_EVENT_COREDUMP,
        }
    }
}

struct Config {
    raw: bool,
    count: u64,
    filter: EventFilter,
    buffer_size: usize,
}

fn print_help() {
    eprintln!(
        r#"bdd-netlink {}
Ultra-fast Linux Netlink process connector binary stream generator for bdd.

USAGE:
    sudo bdd-netlink [OPTIONS]

OPTIONS:
    -r, --raw               Output raw Netlink packets (including 36-byte framing header)
    -c, --count <N>         Stop after streaming N events (default: 0 = infinite)
    -f, --filter <EVENT>    Filter for specific event: all, fork, exec, exit, uid, gid, comm [default: all]
    -b, --buffer-size <B>   Socket receive buffer size in bytes [default: 8388608 (8 MiB)]
    -h, --help              Print help information
    -V, --version           Print version information

EXAMPLES:
    # Stream live kernel events into bdd with structured JSON output:
    sudo bdd-netlink | bdd --preset netlink-proc-event --output-json

    # Stream only process execution events (exec) to visual ANSI inspector:
    sudo bdd-netlink --filter exec | bdd "netlink-proc-event -> visual"

    # Capture exactly 10 process exit events:
    sudo bdd-netlink --filter exit --count 10 | bdd "netlink-proc-event -> json:object"
"#,
        env!("CARGO_PKG_VERSION")
    );
}

fn parse_args() -> Result<Config, String> {
    let mut config = Config {
        raw: false,
        count: 0,
        filter: EventFilter::All,
        buffer_size: 8 * 1024 * 1024,
    };

    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                process::exit(0);
            }
            "-V" | "--version" => {
                println!("bdd-netlink {}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            "-r" | "--raw" => {
                config.raw = true;
            }
            "-c" | "--count" => {
                i += 1;
                if i >= args.len() {
                    return Err("--count requires an integer argument".to_string());
                }
                config.count = args[i]
                    .parse()
                    .map_err(|e| format!("Invalid count '{}': {}", args[i], e))?;
            }
            "-f" | "--filter" => {
                i += 1;
                if i >= args.len() {
                    return Err("--filter requires an event name argument (all, fork, exec, exit, uid, gid, comm)".to_string());
                }
                config.filter = EventFilter::from_str(&args[i]).ok_or_else(|| {
                    format!(
                        "Unknown filter event '{}'. Valid: all, fork, exec, exit, uid, gid, comm",
                        args[i]
                    )
                })?;
            }
            "-b" | "--buffer-size" => {
                i += 1;
                if i >= args.len() {
                    return Err("--buffer-size requires an integer argument".to_string());
                }
                config.buffer_size = args[i]
                    .parse()
                    .map_err(|e| format!("Invalid buffer size '{}': {}", args[i], e))?;
            }
            arg if arg.starts_with("--count=") => {
                let val = &arg["--count=".len()..];
                config.count = val
                    .parse()
                    .map_err(|e| format!("Invalid count '{}': {}", val, e))?;
            }
            arg if arg.starts_with("--filter=") => {
                let val = &arg["--filter=".len()..];
                config.filter = EventFilter::from_str(val).ok_or_else(|| {
                    format!(
                        "Unknown filter event '{}'. Valid: all, fork, exec, exit, uid, gid, comm",
                        val
                    )
                })?;
            }
            arg if arg.starts_with("--buffer-size=") => {
                let val = &arg["--buffer-size=".len()..];
                config.buffer_size = val
                    .parse()
                    .map_err(|e| format!("Invalid buffer size '{}': {}", val, e))?;
            }
            unknown => {
                return Err(format!(
                    "Unknown option '{}'. Use --help for usage.",
                    unknown
                ));
            }
        }
        i += 1;
    }

    Ok(config)
}

#[cfg(not(target_os = "linux"))]
fn run() -> io::Result<()> {
    eprintln!("bdd-netlink: Linux Netlink process connector is only supported on Linux.");
    process::exit(1);
}

#[cfg(target_os = "linux")]
fn run() -> io::Result<()> {
    let config = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Use --help for usage instructions.");
            process::exit(2);
        }
    };

    unsafe {
        // Install signal handlers for clean unregistration
        libc::signal(
            libc::SIGINT,
            sig_handler as *const () as usize as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            sig_handler as *const () as usize as libc::sighandler_t,
        );
        // Ignore SIGPIPE so broken pipes close gracefully
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
    }

    // Open Netlink socket
    let fd = unsafe { libc::socket(libc::AF_NETLINK, libc::SOCK_DGRAM, NETLINK_CONNECTOR) };
    if fd < 0 {
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EPERM) || err.raw_os_error() == Some(libc::EACCES) {
            eprintln!(
                "bdd-netlink: Permission denied opening NETLINK_CONNECTOR socket.\nRun with 'sudo' or grant CAP_NET_ADMIN capability:\n    sudo ./bdd-netlink"
            );
        } else {
            eprintln!(
                "bdd-netlink: Failed to open Netlink connector socket: {}",
                err
            );
        }
        process::exit(1);
    }

    // Configure socket buffer size
    let rcvbuf_size = config.buffer_size as libc::c_int;
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVBUF,
            &rcvbuf_size as *const _ as *const libc::c_void,
            std::mem::size_of_val(&rcvbuf_size) as libc::socklen_t,
        );

        // Avoid packet drops under heavy burst
        let opt_no_enobufs: libc::c_int = 1;
        libc::setsockopt(
            fd,
            SOL_NETLINK,
            NETLINK_NO_ENOBUFS,
            &opt_no_enobufs as *const _ as *const libc::c_void,
            std::mem::size_of_val(&opt_no_enobufs) as libc::socklen_t,
        );

        // Bind to Netlink process group
        let mut addr: libc::sockaddr_nl = std::mem::zeroed();
        addr.nl_family = libc::AF_NETLINK as libc::sa_family_t;
        addr.nl_pid = libc::getpid() as u32;
        addr.nl_groups = 1; // CN_IDX_PROC

        if libc::bind(
            fd,
            &addr as *const _ as *const libc::sockaddr,
            std::mem::size_of_val(&addr) as libc::socklen_t,
        ) < 0
        {
            let err = io::Error::last_os_error();
            libc::close(fd);
            eprintln!("bdd-netlink: Failed to bind socket to CN_IDX_PROC: {}", err);
            process::exit(1);
        }

        // Send PROC_CN_MCAST_LISTEN registration (40 bytes)
        let mut reg_msg = Vec::with_capacity(40);
        let nlmsg_len: u32 = 40;
        let nlmsg_type: u16 = 0x3; // NLMSG_DONE
        let nlmsg_flags: u16 = 0;
        let nlmsg_seq: u32 = 0;
        let nlmsg_pid: u32 = libc::getpid() as u32;

        reg_msg.extend_from_slice(&nlmsg_len.to_ne_bytes());
        reg_msg.extend_from_slice(&nlmsg_type.to_ne_bytes());
        reg_msg.extend_from_slice(&nlmsg_flags.to_ne_bytes());
        reg_msg.extend_from_slice(&nlmsg_seq.to_ne_bytes());
        reg_msg.extend_from_slice(&nlmsg_pid.to_ne_bytes());

        // cn_msg header (20 bytes)
        reg_msg.extend_from_slice(&1u32.to_ne_bytes()); // id.idx = CN_IDX_PROC (1)
        reg_msg.extend_from_slice(&1u32.to_ne_bytes()); // id.val = CN_VAL_PROC (1)
        reg_msg.extend_from_slice(&0u32.to_ne_bytes()); // seq
        reg_msg.extend_from_slice(&0u32.to_ne_bytes()); // ack
        reg_msg.extend_from_slice(&4u16.to_ne_bytes()); // len = 4 (sizeof op)
        reg_msg.extend_from_slice(&0u16.to_ne_bytes()); // flags

        // proc_cn_mcast_op: PROC_CN_MCAST_LISTEN (1)
        reg_msg.extend_from_slice(&1u32.to_ne_bytes());

        let sent = libc::send(
            fd,
            reg_msg.as_ptr() as *const libc::c_void,
            reg_msg.len(),
            0,
        );
        if sent < 0 {
            let err = io::Error::last_os_error();
            libc::close(fd);
            eprintln!("bdd-netlink: Failed to send PROC_CN_MCAST_LISTEN: {}", err);
            process::exit(1);
        }

        // Drain initial ACK / response
        let mut ack_buf = [0u8; 1024];
        libc::recv(
            fd,
            ack_buf.as_mut_ptr() as *mut libc::c_void,
            ack_buf.len(),
            0,
        );
    }

    let stdout = io::stdout();
    let mut out = io::BufWriter::with_capacity(64 * 1024, stdout.lock());
    let mut recv_buf = vec![0u8; 65536];
    let mut emitted: u64 = 0;

    while RUNNING.load(Ordering::Relaxed) {
        let n = unsafe {
            libc::recv(
                fd,
                recv_buf.as_mut_ptr() as *mut libc::c_void,
                recv_buf.len(),
                0,
            )
        };

        if n <= 0 {
            if !RUNNING.load(Ordering::Relaxed) {
                break;
            }
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            break;
        }

        let total = n as usize;
        let payload = if config.raw {
            &recv_buf[..total]
        } else if total >= 36 {
            // Strip 36-byte framing header (16B nlmsghdr + 20B cn_msg)
            let body = &recv_buf[36..total];
            // If filter is active and body has proc_event.what (at least 4 bytes)
            if config.filter != EventFilter::All && body.len() >= 4 {
                let what = u32::from_ne_bytes(body[0..4].try_into().unwrap());
                if !config.filter.matches(what) {
                    continue;
                }
            }
            body
        } else {
            // Too short for standard Netlink process connector event
            continue;
        };

        if out.write_all(payload).is_err() {
            // Downstream process closed pipe (e.g. bdd terminated)
            break;
        }

        emitted += 1;
        if config.count > 0 && emitted >= config.count {
            break;
        }
    }

    let _ = out.flush();

    // Clean unregistration with PROC_CN_MCAST_IGNORE
    unsafe {
        let mut reg_msg = Vec::with_capacity(40);
        let nlmsg_len: u32 = 40;
        let nlmsg_type: u16 = 0x3;
        let nlmsg_flags: u16 = 0;
        let nlmsg_seq: u32 = 0;
        let nlmsg_pid: u32 = libc::getpid() as u32;

        reg_msg.extend_from_slice(&nlmsg_len.to_ne_bytes());
        reg_msg.extend_from_slice(&nlmsg_type.to_ne_bytes());
        reg_msg.extend_from_slice(&nlmsg_flags.to_ne_bytes());
        reg_msg.extend_from_slice(&nlmsg_seq.to_ne_bytes());
        reg_msg.extend_from_slice(&nlmsg_pid.to_ne_bytes());

        reg_msg.extend_from_slice(&1u32.to_ne_bytes());
        reg_msg.extend_from_slice(&1u32.to_ne_bytes());
        reg_msg.extend_from_slice(&0u32.to_ne_bytes());
        reg_msg.extend_from_slice(&0u32.to_ne_bytes());
        reg_msg.extend_from_slice(&4u16.to_ne_bytes());
        reg_msg.extend_from_slice(&0u16.to_ne_bytes());

        // proc_cn_mcast_op: PROC_CN_MCAST_IGNORE (2)
        reg_msg.extend_from_slice(&2u32.to_ne_bytes());

        libc::send(
            fd,
            reg_msg.as_ptr() as *const libc::c_void,
            reg_msg.len(),
            0,
        );
        libc::close(fd);
    }

    Ok(())
}

fn main() {
    if let Err(err) = run() {
        if err.kind() != io::ErrorKind::BrokenPipe {
            eprintln!("bdd-netlink: {}", err);
            process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_filter_parsing() {
        assert_eq!(EventFilter::from_str("all"), Some(EventFilter::All));
        assert_eq!(EventFilter::from_str("fork"), Some(EventFilter::Fork));
        assert_eq!(EventFilter::from_str("EXEC"), Some(EventFilter::Exec));
        assert_eq!(EventFilter::from_str("exit"), Some(EventFilter::Exit));
        assert_eq!(EventFilter::from_str("comm"), Some(EventFilter::Comm));
        assert_eq!(EventFilter::from_str("invalid"), None);
    }

    #[test]
    fn test_event_filter_matching() {
        let f_exec = EventFilter::Exec;
        assert!(f_exec.matches(PROC_EVENT_EXEC));
        assert!(!f_exec.matches(PROC_EVENT_FORK));

        let f_all = EventFilter::All;
        assert!(f_all.matches(PROC_EVENT_EXEC));
        assert!(f_all.matches(PROC_EVENT_FORK));
        assert!(f_all.matches(PROC_EVENT_EXIT));
    }
}
