# Developer Setup

These steps are required for building and running mirror code from source.

1) Install Rust, https://rustup.rs
2) (Linux/Ubuntu/Debian) Install Development Libraries, (TODO: Still working on this list)
    - `build-essential`
    - `pkg-config`
    - `libssl-dev`
    - `libxcb-composite0-dev`
    - `libxcursor-dev`
    - `libxrandr-dev`
    - `libxi-dev`
    - `libx11-dev`
    - `libx11-xcb-dev`
    - `cmake`
    - `fontconfig`

3) Run `cargo update` (You'll want to do this every once in a while to update deps)
4) Run `cargo build`

## Steps for setting up VSCode 

1) Install Rust extension, 
```
rust-analyzer
Id: rust-lang.rust-analyzer
Description: Rust language support for Visual Studio Code
Version: 0.3.1369
Publisher: The Rust Programming Language 
VS Marketplace Link: https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer
```

## Troubleshooting Common Issues

1) After running `cargo update` vscode can't find symbols
- This happens when updating dependencies. First run `cargo clean` and then reload vscode. 
