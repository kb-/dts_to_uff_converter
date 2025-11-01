# DTS to UFF Converter

A Rust workspace that converts DTS data acquisition exports into [Universal File Format (UFF)](https://en.wikipedia.org/wiki/Universal_File_Format) Type 58 files. The project provides both a command-line interface (CLI) for local conversions and a Model Context Protocol (MCP) server that exposes the same workflow as a tool for AI assistants.

## Features

- Parses DTS `.dts` and `.chn` files and writes UFF Type 58 output in ASCII or binary formats.
- Progress-aware CLI that reports channel discovery and conversion status.
- Reusable conversion library for integration in other tools.
- MCP stdio server exposing a `convert_dts_to_uff` tool for conversational clients.

## Command-line usage

The CLI binary is named `dts_to_uff_converter`.

```bash
cargo run -- \
  --input-dir /path/to/dts/folder \
  --tracks /path/to/track_names.txt \
  --output /path/to/output.uff \
  --format ascii
```

Arguments:

- `--input-dir` (`-i`): Directory containing the DTS export (`.dts`/`.chn` files).
- `--tracks` (`-t`): Text file listing channel names (one per line or comma separated).
- `--output` (`-o`): Destination path for the generated UFF file.
- `--format` (`-f`): Either `ascii` (default) or `binary`.
- `--slice`: Limit the exported samples for every track using zero-based `start:end` indices (end-exclusive). The same slice is applied to every selected track, expressed in native sample units for that track. Leave the flag unset to export all samples.
- `--track-list-output`: Comma-separated list of track names to include in the UFF output.

Use `dts_to_uff_converter --help` to view the full CLI reference.

## MCP server usage

The MCP server binary is built at `target/release/mcp_server` (or `mcp_server.exe` on Windows). It communicates over stdio so it can be launched as a subprocess by MCP-compatible clients.

```bash
cargo run --bin mcp_server
```

To view the available options and description, run:

```bash
mcp_server --help
```

When the server starts it registers a single tool, `convert_dts_to_uff`. Provide absolute paths that are accessible to the server process when invoking the tool from an MCP client.

The tool expects the following parameters:

- `input_dir`: Absolute path to the DTS export directory containing `.dts`/`.chn` files (must be a directory).
- `tracks_file`: Absolute path to a text file listing track names, separated by newlines or commas (must be a file).
- `output_path`: Absolute path, including filename, where the generated `.uff` file will be written (must be a file path; the parent directory should already exist).
- `format`: Optional output format, either `ascii` (default) or `binary`.

### MCP client configuration example

Most MCP-compatible clients (Claude Desktop, Cursor, VS Code, etc.) read a JSON configuration that follows the MCP JSON standard introduced in FastMCP 2.4.0. Your configuration file should **only** contain the top-level `mcpServers` object—do not wrap it inside legacy schemas like a `servers` array, or the client will reject the entry.

```json
{
  "mcpServers": {
    "dts-to-uff": {
      "command": "/absolute/path/to/mcp_server",
      "args": [],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

Update the `command` field to point to the compiled `mcp_server` binary (or replace it with a command such as `cargo run --bin mcp_server --release` if you prefer to launch via Cargo) and remove the `env` block if no additional environment variables are needed. Copy this object into the location your MCP client expects—e.g., `~/.claude/claude_desktop_config.json`, `~/.cursor/mcp.json`, or `.vscode/mcp.json` inside a workspace. If your client requires an explicit transport specification (for example, `"transport": "http"` for a remote server), add it directly inside the server object shown above.

## Development

- Format code with `cargo fmt --all`.
- Lint with `cargo clippy --workspace --all-targets -- -D warnings`.
- Run tests with `cargo test`.

These commands should be run (and succeed) before submitting changes.
