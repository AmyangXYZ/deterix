# Deterix: Deterministic Network Emulator with Microsecond Timing Precision

Deterix is a lightweight network emulator written in Rust for building distributed applications that require precise timing control. It provides time-slotted communication with <1µs jitter on COTS hardware running Linux with PREEMPT_RT patch.

## Features

- Time-slotted communication (TDMA)
- Microsecond timing precision (<1µs jitter)
- No GC interruptions
- Zero-copy packet handling
- Distributed architecture

## Network Architecture

### Overview
The network operates on a Time Division Multiple Access (TDMA) protocol with Node 0 serving as the network orchestrator. Key architectural features include:
- Distributed time synchronization with <1µs precision
- Deterministic slot-based communication
- Dynamic network formation and management

### Slot Structure
```
|<------- SLOTFRAME (N slots) ------->|
+------+------+------+------+------+--+
| Slot | Slot | Slot | Slot | Slot |..|
| 0    | 1    | 2    | 3    | 4    |  |
+------+------+------+------+------+--+

|<------ Single Slot Structure ------>|
+-----------+------------+------------+
| Packet TX | ACK Window | Guard Band |
+-----------+------------+------------+
```

### Time Synchronization
- **Node 0 (Orchestrator)**
  - Provides network-wide time reference using system clock
  - Manages slot timing and synchronization
  - Broadcasts periodic sync messages

- **Other Nodes**
  - Maintain synchronized local clocks
  - Update timing based on sync messages
  - Calculate slot boundaries locally

## Network Operations

### Network Formation
1. **Initialization Phase**
   - Node 0 starts as orchestrator
   - Dedicated join slots are reserved in each slotframe
   - New nodes begin in unsynchronized state

2. **Join Process**
   ```
   New Node                  Node 0 (Orchestrator)
      |                              |
      |------ Join Request --------->| (in shared slot)
      |                              | - Records node
      |                              | - Prepares timing info
      |<----- Join Response ---------)
      |                              |
      |- Synchronize local clock     |
      |- Start normal operation      |
   ```

### Communication Protocol
1. **Slot Assignment**
   - Deterministic sender/receiver pairs per slot
   - Scheduled transmission windows
   - Guard bands prevent overlap

2. **Transmission Process**
   - Reliable delivery with ACK mechanism
   - Automatic retransmission
   - Sequence number tracking

### Implementation Details
- **Core Components**
  - TX/RX Queue Management
  - Slot Ticker System
  - Zero-copy Packet Pool
  - RT Thread Scheduling

- **Performance Features**
  - Thread affinity optimization
  - RT scheduler prioritization
  - Efficient buffer management
  - Microsecond-precision timing