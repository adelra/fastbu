# Fastbu

<p align="center">
  <img src="img/logo.jpg" alt="Fastbu Logo" width="300">
</p>

Fast Binary Ultracache (Fastbu). A quick, resilient, fault-tolerant, and on-disk caching system written in Rust. Mostly for me to learn Rust.

## Overview

Fastbu is a lightweight caching system that provides persistent storage with high performance and reliability. It's designed to be simple to use while offering robust features for data persistence and retrieval.

I did this project just to learn Rust, and it is probably not production-ready.

## Features

- **Fast Access**: In-memory index for quick lookups combined with efficient disk storage.
- **Binary Serialization**: Uses `bincode` for compact and fast serialization.
- **Thread Safety**: Mutex-protected operations for concurrent access.
- **REST API**: Simple HTTP interface for cache operations.
- **Configurable**: Customizable host and port settings.

### Features in Plan

- **Basic Cleanup Mechanism**: A periodic cleanup task to remove expired entries from the cache.
- **Advanced Fault Tolerance**: Implement robust recovery mechanisms for corrupted data or disk failures to ensure high reliability.
- **Metadata Tracking**: Add support for tracking creation time, update time, and size for each cache entry to improve cache management and analytics.
- **Comprehensive Cleanup**: Enhance the cleanup mechanism to handle inconsistencies in storage and metadata tracking, ensuring a more robust and efficient cache.
- **Production-Grade Testing**: Develop comprehensive unit tests and integration tests to ensure the system is production-ready and reliable under various conditions.

## Installation

### Prerequisites

- Rust 1.56.0 or later
- Cargo (comes with Rust)
- Make (for using the Makefile)

### Building from Source

```bash
# Clone the repository
git clone https://github.com/adelra/fastbu.git
cd fastbu

# Build the project
cargo build --release

# Or using Make
make build-release