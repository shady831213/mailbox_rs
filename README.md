# mailbox_rs

`mailbox_rs` is a Rust mailbox protocol and RPC framework for communication between verification firmware and host-side simulation / verification services.

It provides the shared protocol layer used by both sides of the connection:

- `no_std` target-side access for firmware
- `std` host-side async channels and servers
- C-compatible request/response queue layouts
- extensible RPC dispatch
- shared-memory access and pointer resolution
- host-backed printing, memory operations, file access, generic calls, and exit/status reporting
- configurable pointer width and cache-line layout

It is a protocol/runtime layer rather than a simulator-specific transport.

## Where it fits

```text
        Target / Firmware                    Host / Simulator
        -----------------                    ----------------

            vfw_rs                              vhost
               |                                  |
          no_std sender                       std server
               |                                  |
               +--------- mailbox_rs -------------+
                         shared protocol

                         request queue
                         response queue
                         RPC actions
                         shared memory
```

The same protocol can therefore be used by bare-metal verification firmware on one side and a Rust/SystemVerilog/Python-enabled host environment on the other.

## Core protocol

The base channel uses C-compatible request and response entries:

```text
MBReqEntry
  action
  words
  args[]

MBRespEntry
  words
  rets
```

Requests and responses are carried through bounded producer/consumer queues. Queue state uses producer and consumer indices plus wrap flags to distinguish full and empty states.

The shared structures are accessed with volatile reads/writes because they are intended to represent communication-visible memory rather than ordinary compiler-private data.

Pointer width can be selected as:

- `ptr32`
- `ptr64`
- `ptrhost`

Queue structures can also be aligned to configurable cache-line sizes (`32`, `64`, `128`, or `256` bytes).

## RPC model

`mailbox_rs` models operations as RPCs through the `MBRpc` trait:

```rust
pub trait MBRpc {
    type REQ;
    type RESP;

    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry);
    fn get_resp(&self, entry: &MBRespEntry) -> Self::RESP;
}
```

Built-in actions include:

- exit/status reporting
- string and formatted printing
- `memmove`, `memset`, and `memcmp`
- generic method calls
- host-backed file open/close/read/write/seek

The protocol also reserves an `OTHER` action space so host environments can register project-specific or environment-specific RPCs without changing the base queue mechanism.

## `no_std` target side

Enable the `no_std` feature for firmware-side use.

The target implementation provides non-blocking mailbox channels and RPC helpers, with weak read/write fence hooks that a platform can override when communication memory requires explicit synchronization.

This is the layer consumed by [`vfw_rs`](https://github.com/shady831213/vfw_rs), where `vfw_mailbox` exposes mailbox-backed services to verification firmware.

## `std` host side

Enable the `std` feature for host-side use.

The host implementation includes:

- async mailbox channels
- mailbox servers
- RPC dispatch
- mailbox builders/configuration
- pointer resolvers
- shared-memory abstractions
- host-backed filesystem services
- custom RPC registration

This side is suitable for connecting the mailbox protocol to a simulator, executable model, testbench, or other verification host.

[`vhost`](https://github.com/shady831213/vhost) builds on this layer to integrate mailbox services with SystemVerilog DPI/UVM, configurable memory backends, optional Python callbacks, and other host-side infrastructure.

## Protocol separation

A major design goal is to keep the target test program independent of a specific simulator integration.

```text
verification firmware
        |
   mailbox RPC
        |
  mailbox_rs protocol
        |
 host-side service implementation
        |
  +-----+------+----------------+
  |            |                |
 UVM/DPI   executable model   Python/host code
```

This makes it possible to keep the firmware-visible service contract stable while changing the environment behind it.

The open-source implementation provides the generic protocol and runtime pieces; real verification environments can layer additional project-specific RPCs, memory mappings, and host integrations on top.

## Features

From `Cargo.toml`:

| Feature | Purpose |
| --- | --- |
| `std` | Host-side async/server/filesystem/shared-memory support |
| `no_std` | Firmware/target-side non-blocking support |
| `ptr32` | 32-bit mailbox pointer representation |
| `ptr64` | 64-bit mailbox pointer representation |
| `ptrhost` | Native host pointer representation |
| `cache_line_32` | 32-byte queue alignment |
| `cache_line_64` | 64-byte queue alignment |
| `cache_line_128` | 128-byte queue alignment |
| `cache_line_256` | 256-byte queue alignment |

## Toolchain

The crate declares Rust 1.80 and uses a repository `rust-toolchain` file selecting nightly because the implementation currently relies on nightly features such as `linkage`.

Typical checks should therefore use the repository-selected nightly toolchain.

## Repository layout

```text
mailbox_rs/
|-- src/mb_channel.rs      # shared queue/channel layout and state
|-- src/mb_rpcs.rs         # common RPC actions and wire types
|-- src/mb_no_std/         # target-side non-blocking implementation
|-- src/mb_std/            # host-side async/server/shared-memory implementation
|-- Cargo.toml             # feature matrix
`-- rust-toolchain         # nightly toolchain selection
```

## Related projects

- [`vfw_rs`](https://github.com/shady831213/vfw_rs) — firmware-side verification runtime using `mailbox_rs` in `no_std` mode
- [`vhost`](https://github.com/shady831213/vhost) — host-side verification integration using `mailbox_rs` services
- [`terminus_cosim`](https://github.com/shady831213/terminus_cosim) — ISA/RTL co-simulation environment built around the same broader verification stack
