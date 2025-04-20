# Using cargo-typify: JSON Schema to Rust Type Generator

`cargo-typify` is a powerful command-line tool that converts JSON Schema definitions into Rust types with full serialization/deserialization support. Here's how to use it effectively:

## Installation

```bash
cargo install cargo-typify
```

## Basic Usage

Convert a JSON Schema file into Rust types:

```bash
cargo typify -o output.rs input-schema.json
```

## Key Features

- **Automatic Type Generation**: Creates Rust structs from JSON Schema definitions
- **Serde Integration**: Includes all necessary serde attributes for serialization/deserialization
- **Builder Pattern**: Generates builder implementations for easy object construction
- **Field Renaming**: Automatically handles camelCase to snake_case conversion
- **Default Implementations**: Creates sensible defaults for all types
- **Error Handling**: Robust conversion error handling for all types

## Common Options

- `-o, --output <FILE>`: Specify output file (use `-` for stdout)
- `-b, --builder`: Include builder pattern (default)
- `-B, --no-builder`: Disable builder pattern generation
- `-a, --additional-derive <DERIVE>`: Add custom derive macros
- `--map-type <MAP_TYPE>`: Specify type for maps (default: HashMap)
- `--crate <CRATES>`: Specify crate@version for x-rust-type extension

## Example Workflow

1. **Install the tool**:
   ```bash
   cargo install cargo-typify
   ```

2. **Create your project structure**:
   ```bash
   mkdir -p my_project/src/generated
   ```

3. **Generate types from schema**:
   ```bash
   cargo typify -o my_project/src/generated/types.rs schema.json
   ```

4. **Create a module file**:
   ```bash
   echo '//! Generated types
   pub use types::*;
   mod types;' > my_project/src/generated/mod.rs
   ```

5. **Use the generated types**:
   ```rust
   mod generated;
   use generated::MyTypeBuilder;
   
   fn main() {
       let my_obj = MyTypeBuilder::default()
           .field1("value")
           .field2(123)
           .build()
           .unwrap();
   }
   ```

## Best Practices

- Group generated types in a dedicated module
- Consider adding custom traits to extend functionality
- Use the builder pattern for creating complex objects
- Update generated types when your schema changes

For more details, run `cargo typify --help`