/// Mirror template,
/// 
pub static MIRROR_TEMPLATE: &'static str = r#"
# Configure mirror settings
```
+ .config               start.mirror
: app_host              .symbol         localhost:8567
: teleport              .symbol         overlaybd
: skip_hosts_dir_check  .false

# Debug config
+ debug .config         start.mirror
: .branch
: skip_hosts_dir_check  .true
```

# Resolve manifest handler (/v2/../manifests/..)
- This handler will resolve the requested reference with the upstream server, 
- Subsequent plugins will noww have the digest and manifest for the original image
- Using the resolved digest, we call the referrer's api to `.discover` links to streamable formats

```
+ .operation        manifests.resolve
: .login            access_token
: .authn            oauth2
: .resolve          application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.docker.distribution.manifest.v2+json, application/vnd.oci.image.manifest.v1+json, */*     
: .discover         teleport.link.v1
: .teleport         

## Example of a "manual" teleport
- This configures the handler to swap a specific digest
# : .teleport     manual
# : .from         sha256:820582b05253c2b968442b8af31d791ae64478bcc18e04826c5ce42f974d3272
# : .to           sha256:b0622f86e3d078425d9e2964e48952d2ffa8b5258b836b159405dbc77c2ac4aa
```

# Download blob handler (/v2/../blobs/..)
```
+ .operation      blobs.download
: .login          access_token
: .authn          oauth2
: .continue      
```
"#;

/// Template user's runmd mirror file,
///
pub static MIRROR_ENGINE_TEMPLATE: &'static str = r#"
# ACR Mirror 
- This file is generated per registry host
- It provides a mirror server that facilitates the teleport feature on the host machine
- This file can be edited to customize settings

## Control Settings 
- Engine sequence when the mirror starts

```
+ .engine           <This is the engine definition for the mirror server>
: .once  install    <This first step will login to acr and setup credentials>
: .start start      <This will start the mirror server>
: .start recover    <If the mirror crashes for some reason, this step will attempt to recover>
: .loop             <If a step crashes, this ensures the engine loops>
```

## Install mirror components
- This will login to acr as an admin user, then
- Copy credentials over to overlaybd's credential file

``` install
+ .runtime
: .login-acr        
: .admin            
: .login-overlaybd  
: .registry         
```

## Start the mirror server
- When this event is called it will start a server that will operate indefinitely,
- If an error occurs, it should restart the server after going through the setup process once more 

``` start
# Uncomment below to skip checking the hosts dir, this is useful when testing with curl
# : skip_hosts_dir_check .true

+ .runtime
: .login-acr    
: .install      access_token
: .mirror       {registry_name}.{registry_host}
: .host         localhost:8578, resolve, pull

# Proxy settings
+ .proxy        localhost:8578
: .manifests    
: .get
: .head

: .blobs        get
"#;