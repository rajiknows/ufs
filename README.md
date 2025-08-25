# UFS - Universal File Storage

![Rust](https://img.shields.io/badge/rust-1.79.0-orange.svg)
![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)

A decentralized file storage system built with Rust, leveraging a Kademlia-based DHT for peer-to-peer file sharing.

## Features

- **Decentralized File Storage:** Store and retrieve files from a distributed network of nodes.
- **Peer-to-Peer Networking:** Nodes communicate directly with each other to share files.
- **Content-Addressable Storage:** Files are identified by the hash of their content, ensuring data integrity.
- **Command-Line Interface:** Easy-to-use CLI for interacting with the network.

## Table of Contents

- [Architecture](#architecture)
- [Getting Started](#getting-started)
  - [Prerequisites](#prerequisites)
  - [Building and Running](#building-and-running)
- [Usage](#usage)
  - [Server Mode](#server-mode)
  - [CLI Mode](#cli-mode)
- [Contributing](#contributing)
- [License](#license)

## Architecture

The project is designed as a distributed file storage network. It consists of the following key components:

- **Nodes:** Each node in the network is a peer that can store and retrieve file chunks.
- **Distributed Hash Table (DHT):** A Kademlia-based DHT is used to locate file chunks across the network.
- **gRPC API:** Nodes communicate with each other using a gRPC API for actions like storing, retrieving, and replicating data.
- **Command-Line Interface (CLI):** A `clap`-based CLI allows users to manage the node and interact with the network.

Data is serialized using Protocol Buffers for efficient storage and network transmission.

**Logging:** [Tracing](https://github.com/tokio-rs/tracing)

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable version)
- [Protocol Buffers Compiler](https://grpc.io/docs/protoc-installation/)

### Building and Running

1. **Clone the repository:**
   ```bash
   git clone <repository-url>
   cd ufs
   ```

2. **Build the project:**
   ```bash
   cargo build --release
   ```

3. **Run the node:**
   ```bash
   ./target/release/ufs
   ```

## Usage

The program has two main modes: `server` and `cli`.

### Server Mode

Run a node on a given port:

```bash
./target/release/ufs server --port 42069
```

Join an existing network by providing a bootstrap peer:

```bash
./target/release/ufs server --port 42070 --bootstrap-peer http://127.0.0.1:42069
```

### CLI Mode

Interact with a running node:

```bash
./target/release/ufs cli --node-addr http://127.0.0.1:42069 <COMMAND>
```

**Upload a file:**

```bash
./target/release/ufs cli --node-addr http://127.0.0.1:42069 upload --path ./myfile.txt
```

**Download a file by hash:**

```bash
./target/release/ufs cli --node-addr http://127.0.0.1:42069 download --hash <file_hash> --output ./downloaded.txt
```

**List stored files:**

```bash
./target/release/ufs cli listfiles
```

**List connected peers:**

```bash
./target/release/ufs cli listpeers
```

**Show stored chunks:**

```bash
./target/release/ufs cli showchunks
```

## Contributing

Contributions are welcome! Please feel free to submit a pull request or open an issue.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.