# LogNorm

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)](#)
[![License](https://img.shields.io/badge/license-MIT-blue)](#)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange)](#)

LogNorm is a **high-performance log parsing and normalization system** written in Rust, designed to efficiently process massive log files (millions of lines) with minimal memory usage. It supports advanced features such as **SIMD acceleration**, **custom memory allocation**, and **parallel processing** to maximize throughput while maintaining safety and reliability.

## Features

- **High-performance log parsing**  
  Optimized to handle large-scale log files efficiently, including Python, Django, Flask, and system logs.

- **SIMD-accelerated parsing (optional)**  
  Leverages Rust’s `simd` features to speed up pattern matching and string processing.

- **Parallel processing**  
  Uses the `rayon` crate to process log chunks in parallel, fully utilizing multi-core CPUs.

- **Custom memory allocator (optional)**  
  Supports `bumpalo` for efficient, low-overhead memory management during parsing.

- **Flexible log entry schema**  
  Supports structured logs with optional fields, easily extendable for custom formats.

- **Chunked processing**  
  Handles very large files by processing in configurable chunks (default: 4MB per chunk).

- **Cross-platform**  
  Runs on Linux, macOS, and Windows.

## Currently Supported Systems

LogNorm currently supports parsing logs from the following systems and frameworks. Each system has a dedicated parser optimized for its typical log format.

| System / Framework         | Description                                                            | Parser Type  | Notes                                                              |
| -------------------------- | ---------------------------------------------------------------------- | ------------ | ------------------------------------------------------------------ |
| **Python**                 | Standard Python logging module output                                  | `python`     | Handles timestamped messages, log levels, and multiline exceptions |
| **Django**                 | Web framework logs, often including HTTP requests and database queries | `django`     | Extracts request info, status codes, and traceback details         |
| **Flask**                  | Lightweight web framework logs                                         | `flask`      | Supports timestamped log lines and custom app messages             |
| **Systemd / Linux syslog** | Linux system logs and journal entries                                  | `system`     | Supports host, service, priority/level extraction                  |
| **NGINX**                  | Web server access and error logs                                       | `nginx`      | Parses HTTP requests, status codes, and IPs                        |
| **Apache**                 | Apache HTTP Server logs                                                | `apache`     | Supports combined and common log formats                           |
| **Journalctl**             | Systemd journal entries                                                | `journalctl` | Supports timestamped log lines and custom app messages             |
| **Custom / Generic**       | Any log following `timestamp level message` pattern                    | `generic`    | Flexible, for logs without a pre-defined parser                    |

> **Note:** Support for additional systems (e.g., Kubernetes, Docker, or Windows Event Logs) is planned for future releases. Users can also define custom parsers if their log format is not listed.

## Table of Contents

1. [Installation](#installation)
2. [Usage](#usage)
3. [Configuration](#configuration)
4. [Log Entry Format](#log-entry-format)
5. [Performance](#performance)
6. [Examples](#examples)
7. [Contributing](#contributing)
8. [License](#license)

---

## Installation

### Prerequisites

- Rust **1.70+**
- Cargo (comes with Rust)
- Optional features:
  - `simd` for SIMD acceleration
  - `custom_alloc` for custom allocator support
  - `parallel` for multithreaded processing

### Build

Clone the repository:

```bash
git clone https://github.com/MrPrincerawat/LogNorm.git
cd lognorm
```

Build with default features:

```bash
cargo build --release
```

Build with optional features:

```bash
cargo build --release --features parallel
```

This will produce a binary in `target/release/lognorm`.

---

## Usage

### Basic Example

```rust
use lognorm::parser::parse;
use anyhow::Result;

fn main() -> Result<()> {
    let log_file = "/path/to/logfile.log";
    let parsed_entries = parse("default", log_file)?;
    for entry in parsed_entries {
        println!("{:?}", entry);
    }
    Ok(())
}
```

### Command-Line Tool (if compiled as executable)

```bash
./lognorm -p <parser> -o <output> --batch-size <size> --benchmark <file>
```

#### Options

- `-p` – log parser type (`python`, `django`, `flask`, `system`, etc.)
- `-o` – output file path
- `--batch-size` – custom batch size
- `--benchmark` – enable benchmark mode (prints throughput and parse time)

#### Currently Supported Parsers

| Parser Type  | Description                           |
| ------------ | ------------------------------------- |
| `syslog`     | Linux system logs and journal entries |
| `nginx`      | Web server access and error logs      |
| `apache`     | Apache HTTP Server logs               |
| `journalctl` | Systemd journal entries               |
| `python_web` | Python logging module output          |

#### Output Format

By default, the output is a JSON in the stdout. You can also specify a file path to write the output to. Possible formats are `json`, `jsonl`, `ndjson`, `csv`, `tsv`.

## Configuration

LogNorm allows configuring parsing behavior via Rust constants or environment variables:

| Option             | Default       | Description                                      |
| ------------------ | ------------- | ------------------------------------------------ |
| `CHUNK_BYTES`      | `4*1024*1024` | Size of each chunk for parsing                   |
| `MIN_LINE_LEN`     | `20`          | Minimum line length to consider a valid log line |
| `USE_SIMD`         | `false`       | Enable SIMD-based string parsing                 |
| `USE_CUSTOM_ALLOC` | `false`       | Enable bump allocator for memory optimization    |

---

## Log Entry Format

LogNorm uses a structured log entry format:

```rust
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LogEntry {
    pub timestamp: Option<String>,
    pub host: Option<String>,
    pub service: Option<String>,
    pub level: Option<String>,
    pub message: Option<String>,
}
```

All fields are optional to accommodate heterogeneous log formats.

---

## Performance

LogNorm is designed for **high throughput**:

- Efficient memory handling via **custom allocator**
- Multi-core parsing with **rayon**
- Vectorized string search with **SIMD**
- Handles **10M+ line logs** in seconds (depending on hardware)

Benchmarks indicate **2–5x speedup** on large logs when using SIMD + parallel features compared to naive single-threaded parsing.

---

## Examples

### Generating a Large Sample Log (Python/Flask/Django)

```bash
python generate_sample_logs.py --lines 10000000 --output sample.log
```

### Parsing Logs

```rust
let logs = parse("flask", "/tmp/sample.log")?;
println!("Parsed {} entries", logs.len());
```

---

## Contributing

Contributions are welcome!

1. Fork the repository
2. Create a new branch (`git checkout -b feature/awesome-feature`)
3. Commit your changes (`git commit -am 'Add new feature'`)
4. Push to the branch (`git push origin feature/awesome-feature`)
5. Open a Pull Request

---

## License

This project is licensed under the **MIT License** – see the [LICENSE](LICENSE) file for details.

---

## Contact

Created and maintained by **Prince Rawat** – [GitHub](https://github.com/princerawat) | [Email](mailto:princerawatformal@gmail.com)

---

### ⚡ Acknowledgements

- [Rayon](https://crates.io/crates/rayon) – for parallelism
- [Bumpalo](https://crates.io/crates/bumpalo) – custom allocator support
- Rust community – for making high-performance programming safe and fun
