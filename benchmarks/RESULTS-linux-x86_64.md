# Engine comparison: Linux x86-64

Date: 2026-09-04

This result compares Wiferry's legacy Python/PyInstaller host with the Rust
`0.2.0-alpha.1` host. It is a local framework-ceiling benchmark, not a Wi-Fi
speed claim.

## Environment

- Ubuntu Linux kernel 6.8.0-138-generic, x86-64
- Intel Core i7-1165G7, 8 logical CPUs
- ZFS source filesystem
- 256 MiB deterministic file
- Loopback HTTP, one client, five runs per engine
- Engines alternate order between runs to reduce time-order bias
- Reported values are medians; every download was SHA-256 verified
- The runner does not measure cancellation or active-stream revocation
- Source revision: `d883a428899c9bfbb2e6139955138cde13663cde`
- Legacy executable SHA-256: `40b715e5f3447acbdba02fde540407dce617c83f942d8f8e9df4a3bd65f08c58`
- Rust executable SHA-256: `4f604b7cc853e7ac4ce101497f8081d9883d68d5090fb013bd27f3c1d0c13e38`

## Result

| Metric | Legacy Python bundle | Rust core | Difference |
|---|---:|---:|---:|
| Binary size | 22,804,552 B | 2,585,416 B | Rust 88.7% smaller |
| Ready-to-HTTP | 918.0 ms | 2.7 ms | Rust about 342x faster |
| Idle process-tree RSS | 61,911,040 B | 4,182,016 B | Rust 93.2% lower |
| 256 MiB loopback throughput | 922.0 MiB/s | 1,018.5 MiB/s | Rust 10.5% higher in this run |

## Interpretation

For the exact binaries recorded in this run, Rust materially improves startup,
resident memory, and distribution size. Its median loopback throughput is 10.5%
higher here, but the individual samples overlap and vary: 858.9–1,037.2 MiB/s
for Rust and 543.9–987.7 MiB/s for the legacy bundle. Wiferry therefore describes
the Rust core as *lighter and faster to become ready*, not universally faster on
the network. Real-LAN results must be reported separately before making network
throughput claims.

Raw per-run data, execution order, binary hashes, and the reproducible runner are
committed beside this report:

- `results-linux-x86_64-256m.json`
- `compare_engines.py`
