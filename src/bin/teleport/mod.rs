
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

# Uncomment below to skip checking the hosts dir, this is useful when testing with curl
# : skip_hosts_dir_check .true

+ .runtime
: .login-acr {registry_name}

: .install  access_token

: .mirror   {registry_name}.{registry_host}
: .host     {mirror_address}, resolve, pull

+ .proxy    {mirror_address}
# Resolve manifests sequence 
: .manifests    head, get
:   .login        access_token
:   .authn        oauth2
:   .resolve      application/vnd.oci.image.manifest.v1+json
# You can update this to customize what formats to resolve
# : .resolve      application/vnd.docker.distribution.manifest.list.v2+json
:   .discover     {artifact_type}
:   .teleport     {teleport_format}

# Download blob sequence
: .blobs          get
:   .login        access_token
:   .authn        oauth2
:   .pull

# Push blobs example
# : .blobs          post
# : .login          access_token
# : .authn          oauth2
#  
```
"#;