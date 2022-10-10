
mod teleport_settings;
pub use teleport_settings::TeleportSettings;
pub use teleport_settings::Commands;
pub use teleport_settings::Init;

/// Template user's runmd mirror file,
///
pub static MIRROR_TEMPLATE: &'static str = r#"
# ACR Mirror 
- This file is generated per registry host
- It provides a mirror server that facilitates the teleport feature on the host machine
- This file can be edited to customize settings

## Control Settings 
- Engine sequence when the mirror starts

```md
<``` mirror>
+ .engine           <This is the engine definition for the mirror server>
: .event install    <This first step will login to acr and setup credentials>
: .event start      <This will start the mirror server>
: .event recover    <If the mirror crashes for some reason, this step will attempt to recover>
: .loop             <If a step crashes, this ensures the engine loops>
```

## Install mirror components
- This will login to acr as an admin user, then
- Copy credentials over to overlaybd's credential file

```md
<``` install mirror>
: src_dir         .symbol .
: work_dir        .symbol .world/{registry_host}/{registry_name}
: file_src        .symbol .world/{registry_host}/{registry_name}/access_token

+ .runtime
: .login-acr        {registry_name}
: .admin
: .login-overlaybd  /opt/overlaybd/cred.json
: .registry         {registry_name}.azurecr.io
```

## Start the mirror server
- When this event is called it will start a server that will operate indefinitely,
- If an error occurs, it should restart the server after going through the setup process once more 

```md
<``` start mirror>
: src_dir         .symbol .
: work_dir        .symbol .world/{registry_host}/{registry_name}
: file_src        .symbol .world/{registry_host}/{registry_name}/access_token
: registry_host   .symbol {registry_host}
: registry_name   .symbol {registry_name}

# Uncomment below to skip checking the hosts dir, this is useful when testing with curl
# : skip_hosts_dir_check .true

+ .runtime
: .login-acr {registry_name}
: .install   access_token
: .mirror    {registry_name}.{registry_host}
: .host      localhost:8578, resolve, pull

# Proxy settings 
- The below is the config for how the mirror will handle incoming requests

+ .proxy  localhost:8578 <The proxy server will be listening on this address>

## Resolve manifest handler (/v2/../manifests/..)
- This handler will resolve the requested reference with the upstream server, 
- Subsequent plugins will noww have the digest and manifest for the original image
- Using the resolved digest, we call the referrer's api to `.discover` links to streamable formats

: .manifests      get, head
: .login          access_token
: .authn          oauth2
: .resolve        application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.docker.distribution.manifest.v2+json, application/vnd.oci.image.manifest.v1+json, */*     
: .discover       teleport.link.v1
: .teleport       overlaybd

## Example of a "manual" teleport
- This configures the handler to swap a specific digest

# : .teleport     manual
# : .from         sha256:820582b05253c2b968442b8af31d791ae64478bcc18e04826c5ce42f974d3272
# : .to           sha256:b0622f86e3d078425d9e2964e48952d2ffa8b5258b836b159405dbc77c2ac4aa

# Download blob handler (/v2/../blobs/..)
: .blobs          get
: .login          access_token
: .authn          oauth2
: .continue      

```

## Recovery Settings 
- This is a really simple stage designed to handle intermittent network issues that may stop the server,

```md
<``` recover mirror> 
+ .runtime
: .println  Waiting for 10 seconds
: .timer    10 s
: .println  Looping back to the start of the engine 
```
"#;