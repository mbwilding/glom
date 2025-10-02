## glom

[![Crate Badge]][Crate] [![Deps.rs Badge]][Deps.rs]

![GitHub Pipelines](screenshots/github_pipelines.png)

[![GitHub Projects](screenshots/github_projects_thumbnail.png)](screenshots/github_projects.png)

[![Pipeline Actions](screenshots/pipeline_actions_thumbnail.png)](screenshots/pipeline_actions.png)

A terminal user interface (TUI) for monitoring GitHub CI/CD pipelines and projects.
Built with [ratatui](https://ratatui.rs/).
Forked from [glim](https://github/junkdog/glim).

### Prerequisites
- a terminal emulator with support for 24-bit color, e.g. [kitty](https://sw.kovidgoyal.net/kitty/)
- a GitHub personal access token (PAT) with `read_api` scope
- `libssl-dev` installed on your system

### Building
```
cargo build --release
```

### Installation

```
cargo install glom-tui
```

#### Arch Linux

```
pacman -S glom
```

### Running

To use glom, you'll need a GitHub personal access token (PAT) for authentication with the GitHub API.
Be aware that this PAT is stored in plain text within the configuration file. If you start glom
without any arguments and it hasn't been set up yet, the program will prompt you to enter the PAT
and the GitHub server URL.

```
$ glom -h
A TUI for monitoring GitHub CI/CD pipelines and projects

Usage: glom [OPTIONS]

Options:
  -c, --config <FILE>      Alternate path to the configuration file
  -p, --print-config-path  Print the path to the configuration file and exit
  -h, --help               Print help
  -V, --version            Print version
```

#### Multiple GitHub servers

There is currently no support for multiple GitHub servers in the configuration file. The interim
solution is to use the `--config` flag to specify a different configuration file, e.g. 
`glom --config glom-corporate.toml` or `glom --config glom-personal.toml`.



  [Crate Badge]: https://img.shields.io/crates/v/glom-tui.svg
  [Crate]: https://crates.io/crates/glom-tui
  [Deps.rs Badge]: https://deps.rs/repo/github/mbwilding/glom/status.svg
  [Deps.rs]: https://deps.rs/repo/github/mbwilding/glom
