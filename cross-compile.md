# Cross-Compilation Guide for A2A Test Suite

This document provides instructions for building the A2A Test Suite for multiple platforms (Windows, macOS, and Linux) and using the resulting binaries to test your A2A server implementation.

## Prerequisites

- Rust and Cargo installed (https://rustup.rs)
- Depending on your host OS, you might need additional tools:
  - On Linux: `build-essential` (or equivalent), cross-compilation toolchains
  - On macOS: Xcode Command Line Tools
  - On Windows: Microsoft Visual C++ Build Tools

## Installing Cross-Compilation Targets

Add the targets for the platforms you want to build for:

```bash
# For 64-bit Windows
rustup target add x86_64-pc-windows-msvc

# For 64-bit macOS
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin  # For Apple Silicon (M1/M2)

# For 64-bit Linux
rustup target add x86_64-unknown-linux-gnu
```

## Cross-Compilation Tools

For easier cross-compilation, you can use one of these tools:

### Option 1: Using `cross` (Recommended for Linux hosts)

Install the `cross` tool:

```bash
cargo install cross
```

Build for different targets:

```bash
# For Windows
cross build --release --target x86_64-pc-windows-msvc

# For macOS (may require macOS host)
cross build --release --target x86_64-apple-darwin

# For Linux
cross build --release --target x86_64-unknown-linux-gnu
```

### Option 2: Using `cargo-zigbuild` (Good for macOS/Linux hosts)

Install the `cargo-zigbuild` tool:

```bash
cargo install cargo-zigbuild
```

Build for different targets:

```bash
# For Windows
cargo zigbuild --release --target x86_64-pc-windows-msvc

# For macOS
cargo zigbuild --release --target x86_64-apple-darwin
cargo zigbuild --release --target aarch64-apple-darwin  # For Apple Silicon

# For Linux
cargo zigbuild --release --target x86_64-unknown-linux-gnu
```

### Option 3: Native Cargo

If you have the appropriate cross-compilation tools installed, you can use cargo directly:

```bash
# For Windows
cargo build --release --target x86_64-pc-windows-msvc

# For macOS
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin  # For Apple Silicon

# For Linux
cargo build --release --target x86_64-unknown-linux-gnu
```

## Building on GitHub Actions (Recommended)

The simplest approach for distributing cross-platform binaries is to use GitHub Actions:

1. Create a new file in your repository: `.github/workflows/release.yml`
2. Use the workflow shown at the end of this document

## Output Binaries

After compilation, the binaries will be located at:

- Windows: `target/x86_64-pc-windows-msvc/release/a2a-test-suite.exe`
- macOS (Intel): `target/x86_64-apple-darwin/release/a2a-test-suite`
- macOS (Apple Silicon): `target/aarch64-apple-darwin/release/a2a-test-suite`
- Linux: `target/x86_64-unknown-linux-gnu/release/a2a-test-suite`

## Creating Distribution Packages

To create packages for distribution:

```bash
# Create release directories
mkdir -p releases/{windows,macos-intel,macos-apple-silicon,linux}

# Copy binaries
cp target/x86_64-pc-windows-msvc/release/a2a-test-suite.exe releases/windows/
cp target/x86_64-apple-darwin/release/a2a-test-suite releases/macos-intel/
cp target/aarch64-apple-darwin/release/a2a-test-suite releases/macos-apple-silicon/
cp target/x86_64-unknown-linux-gnu/release/a2a-test-suite releases/linux/

# Create archives
cd releases
zip -r a2a-test-suite-windows.zip windows/
zip -r a2a-test-suite-macos-intel.zip macos-intel/
zip -r a2a-test-suite-macos-apple-silicon.zip macos-apple-silicon/
tar -czvf a2a-test-suite-linux.tar.gz linux/
```

# Testing Your A2A Server Implementation

The A2A Test Suite provides comprehensive tools to test your A2A server implementation. Here's how to use the precompiled binaries:

## Running Basic Tests

On Windows:
```cmd
a2a-test-suite.exe run-tests --url http://your-server-url
```

On macOS/Linux:
```bash
./a2a-test-suite run-tests --url http://your-server-url
```

## Test Options

- Basic testing with default settings:
  ```bash
  ./a2a-test-suite run-tests --url http://your-server-url
  ```

- Testing with unofficial tests (mock server features):
  ```bash
  ./a2a-test-suite run-tests --url http://your-server-url --run-unofficial
  ```

- Customizing test timeout:
  ```bash
  ./a2a-test-suite run-tests --url http://your-server-url --timeout 30
  ```

## Interpreting Test Results

The test runner will:

1. Fetch the agent card to determine supported capabilities
2. Run tests for core protocol features
3. Run capability-dependent tests based on agent card information
4. Skip tests for unsupported features
5. Provide a summary of test results at the end

A successful test run will show mostly green "Success" messages and a summary showing all tests passed.

## Troubleshooting

If tests are failing:

1. Check that your server's URL is correctly specified and accessible
2. Verify your server implements the A2A protocol correctly
3. Check for any authentication requirements (the test suite uses "Bearer test-token" by default)
4. Look at specific test failures to understand what parts of the protocol need fixing

## Other Useful Commands

The A2A Test Suite includes other useful tools:

- Validate A2A messages:
  ```bash
  ./a2a-test-suite validate --file path/to/message.json
  ```

- Run property-based tests:
  ```bash
  ./a2a-test-suite test --cases 1000
  ```

- Start the mock server for learning/development:
  ```bash
  ./a2a-test-suite server --port 8080
  ```

- Run specific client operations (e.g., send a task):
  ```bash
  ./a2a-test-suite client send-task --url http://your-server-url --message "Test task"
  ```

---

## GitHub Actions Workflow for Release

```yaml
name: Build and Release

on:
  push:
    tags:
      - 'v*'
  workflow_dispatch:

jobs:
  build:
    name: Build ${{ matrix.target }}
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
            artifact_name: a2a-test-suite
            asset_name: a2a-test-suite-linux-amd64

          - target: x86_64-apple-darwin
            os: macos-latest
            artifact_name: a2a-test-suite
            asset_name: a2a-test-suite-macos-intel

          - target: aarch64-apple-darwin
            os: macos-latest
            artifact_name: a2a-test-suite
            asset_name: a2a-test-suite-macos-arm64

          - target: x86_64-pc-windows-msvc
            os: windows-latest
            artifact_name: a2a-test-suite.exe
            asset_name: a2a-test-suite-windows-amd64.exe

    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          target: ${{ matrix.target }}
          override: true
          
      - name: Build
        uses: actions-rs/cargo@v1
        with:
          command: build
          args: --release --target ${{ matrix.target }}
          
      - name: Upload artifact
        uses: actions/upload-artifact@v3
        with:
          name: ${{ matrix.asset_name }}
          path: target/${{ matrix.target }}/release/${{ matrix.artifact_name }}
          
  create-release:
    name: Create Release
    needs: build
    runs-on: ubuntu-latest
    if: startsWith(github.ref, 'refs/tags/')
    
    steps:
      - name: Create Release
        id: create_release
        uses: actions/create-release@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tag_name: ${{ github.ref }}
          release_name: Release ${{ github.ref }}
          draft: false
          prerelease: false
          
      - name: Output Release URL
        run: echo "Release URL ${{ steps.create_release.outputs.upload_url }}"
          
  upload-release:
    name: Upload Release Asset
    needs: create-release
    runs-on: ubuntu-latest
    if: startsWith(github.ref, 'refs/tags/')
    
    strategy:
      matrix:
        include:
          - asset_name: a2a-test-suite-linux-amd64
            asset_path: a2a-test-suite
            asset_content_type: application/octet-stream
            
          - asset_name: a2a-test-suite-macos-intel
            asset_path: a2a-test-suite
            asset_content_type: application/octet-stream
            
          - asset_name: a2a-test-suite-macos-arm64
            asset_path: a2a-test-suite
            asset_content_type: application/octet-stream
            
          - asset_name: a2a-test-suite-windows-amd64.exe
            asset_path: a2a-test-suite.exe
            asset_content_type: application/octet-stream
    
    steps:
      - name: Download artifact
        uses: actions/download-artifact@v3
        with:
          name: ${{ matrix.asset_name }}
          
      - name: Upload Release Asset
        id: upload-release-asset
        uses: actions/upload-release-asset@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          upload_url: ${{ needs.create-release.outputs.upload_url }}
          asset_path: ./${{ matrix.asset_path }}
          asset_name: ${{ matrix.asset_name }}
          asset_content_type: ${{ matrix.asset_content_type }}
```