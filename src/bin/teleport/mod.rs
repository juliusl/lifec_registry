
mod teleport_settings;
pub use teleport_settings::TeleportSettings;

mod overlaybd;
pub use overlaybd::OverlayBD;

/// Template user's runmd mirror file,
///
pub static MIRROR_TEMPLATE: &'static str = r#"
# ACR Mirror 
- This file is generated per registry host
- It provides a mirror server that facilitates the teleport feature on the host machine
- This file can be edited to customize settings

## Control Settings 
- Engine sequence when the mirror starts

``` mirror
+ .engine
: .event install
: .event start
: .loop
```

## Install mirror components
- The overlaybd snapshotter is the current teleport provider,
- This section can be expanded, once new providers are available.

``` install mirror
+ .runtime
: .process lifec 
: .flag --runmd-path lib/overlaybd/setup_env
: .arg start
: .flag --engine-name {operating_system}
```

## Start the mirror server
- When this event is called it will start a server that will operate indefinitely,
- If an error occurs, it should restart the server after going through the setup process once more 

``` start mirror
: src_dir         .symbol .
: work_dir        .symbol .world/{registry_host}/{registry_name}
: file_src        .symbol .world/{registry_host}/{registry_name}/access_token
: teleport_format .symbol {teleport_format}
: artifact_type   .symbol {artifact_type}

+ .runtime
: .login-acr {registry_name}

: .install  access_token

: .mirror   {registry_name}.{registry_host}
: .host     {mirror_address}, resolve, pull
```
"#;