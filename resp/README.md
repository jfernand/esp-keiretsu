# resp-over-x

A tiny RESP-inspired (Redis-protocol-style) command/reply framing for a byte stream — BLE, UART,
TCP, or anything else that moves bytes. `no_std`, no-alloc by default.

Requests are inline commands: plain ASCII, space-separated, CRLF-terminated (the same "inline
command" fallback Redis itself accepts over telnet), so any dumb terminal can act as a client
with no protocol library of its own. Replies and pushes are RESP-typed so a real client can tell
them apart unambiguously instead of scraping text:

- `+OK\r\n` — a command was accepted
- `-ERR <message>\r\n` — a command was rejected
- `$<len>\r\n<payload>\r\n` — a bulk-string reply to a query command
- `><len>\r\n<payload>\r\n` — an unsolicited push (borrowed from RESP3's dedicated push type), so
  a client can tell "you asked for this" apart from "this just showed up"

This crate only knows about bytes — no BLE, no UART — so the same encoding/parsing is reusable
regardless of what carries them.

## Two halves

- **Command parsing + framing** (default, no-alloc): `Command::parse` splits an inline command
  line into tokens, and `write_ok`/`write_err`/`write_bulk`/`write_push` encode replies into a
  caller-supplied `&mut [u8]` buffer. This is the side a resource-constrained device (e.g. an
  embedded firmware target) uses to parse incoming commands and write replies without a heap.
- **Frame decoding** (`decode` feature, needs `alloc`): `RespDecoder`/`RespFrame` buffer incoming
  bytes and parse out `Ok`/`Error`/`Bulk`/`Push` frames as owned values. This is the side a host
  client uses to decode a device's reply/push stream.

```toml
[dependencies]
resp = { package = "resp-over-x", version = "0.1", features = ["decode"] }
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
