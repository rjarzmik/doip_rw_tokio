# DoIP Reader Writer Async interface

The `doip_rw_tokio` crate provides an API on top of `doip_rw` crate to handle connections and message forDiagnostics Over Internet Protocol (DoIP) messages.

It enables implementing a DoIP tester with :
- DoIP TCP client for diagnostic message requesting.
- DoIP UDP client for discovery requests.

It enables implementing a DoIP entity with :
- DoIP TCP server for diagnostic message answering.
- DoIP UDP client for discovery responses and vehicle announcements.

This API is oriented for asynchronous IO, and is an API super-set of `doip_rw` crate, providing methods to send and receive DoIP messages.

## Features
- zero copy send/receive of messages
  For zero copy, special attention needs to be paid to `allocate_uds` method
  parameters as well as using as much as possible `read_replace()` methods in
  `doip_rw` crate
- deserialization "in place" to replace an existing DoIP payload
- for larger messages such as `DiagnosticMessage` both owned and borrowed buffer are available

## Installation
Add the following to your `Cargo.toml`:

```toml
[dependencies]
doip_rw_tokio = "0.1.0"
doip_rw = "0.1.0"
```

## Usage
A simple asynchronous server example is provided in [server](examples/server.rs)
It can be run against a DoIP server on localhost, tcp port 13400, by invoking:
```
cargo run --example server
```

A simple diagnostic read of DID 0xf012 would look like :
```rust
    use doip_rw_tokio::*;
    let mut tcp = connect_doip_tcp(
        "0.0.0.0:13400".parse().unwrap(),
        "192.168.11.57:13400".parse().unwrap(),
        0x00ed,
        Timings {
            tcp_connect: Duration::from_secs(1),
            routing_activation_rsp: Duration::from_secs(1),
        },
    )
    .await
    .unwrap();
    send_uds(
        &mut tcp,
        0x0010,
        UdsBuffer::Owned(vec![0x22, 0xf1, 0x80]),
        |_, _| vec![],
        Duration::from_secs(1),
    )
    .await
    .unwrap();
    let msg = tcp.receive_message(|_, _| vec![]).await.unwrap();
    if let DoIpTcpMessage::DiagnosticMessage(m) = msg {
        println!("Answer: {:2x?}", m.user_data.get_ref())
    }
```

## Documentation
Comprehensive API documentation is available on [docs.rs](https://docs.rs/doip_rw_tokio/).

## Contributing
Contributions are welcome! Feel free to open issues, submit pull requests, or suggest features. Please follow the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct) when contributing.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
