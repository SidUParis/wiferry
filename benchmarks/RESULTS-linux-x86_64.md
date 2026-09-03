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
- Source revision: `a8f30131feebd61da4ec2c67faee512e7aa266ae`
- Legacy executable SHA-256: `40b715e5f3447acbdba02fde540407dce617c83f942d8f8e9df4a3bd65f08c58`
- Rust executable SHA-256: `d0ccd954a087cb4629e6bbe119101a8c1999cd1631b188867a242daad8ca6124`

## Result

| Metric | Legacy Python bundle | Rust core | Difference |
|---|---:|---:|---:|
| Binary size | 22,804,552 B | 2,585,416 B | Rust 88.7% smaller |
| Ready-to-HTTP | 783.5 ms | 2.7 ms | Rust about 287x faster |
| Idle process-tree RSS | 61,964,288 B | 4,870,144 B | Rust 92.1% lower |
| 256 MiB loopback throughput | 913.3 MiB/s | 988.1 MiB/s | Rust 8.2% higher in this run |

## Interpretation

For the exact binaries recorded in this run, Rust materially improves startup,
resident memory, and distribution size. Its median loopback throughput is 8.2%
higher here, but the individual samples overlap and vary: 645.2–1,039.2 MiB/s
for Rust and 825.8–983.3 MiB/s for the legacy bundle. Wiferry therefore describes
the Rust core as *lighter and faster to become ready*, not universally faster on
the network. Real-LAN results must be reported separately before making network
throughput claims.

Raw per-run data, execution order, binary hashes, and the reproducible runner are
committed beside this report:

- `results-linux-x86_64-256m.json`
- `compare_engines.py`
