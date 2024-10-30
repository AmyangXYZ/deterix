# Deterix: Deterministic Network Emulator with Microsecond Timing Precision

Deterix is a lightweight network emulator written in Rust for building distributed applications that require precise timing control. It provides time-slotted communication with <1µs jitter on COTS hardware running Linux with PREEMPT_RT patch.

## Features

- Time-slotted communication (TDMA)
- Microsecond timing precision (<1µs jitter)
- No GC interruptions
- Zero-copy packet handling
- Distributed architecture
