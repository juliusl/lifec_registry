# Lifec Registry 

This project provides tools to extend registries on the client side. It includes a mirror/proxy server, as well as additional tools for extending what a registry can do.

## Getting Started w/ the Mirror 

*Note*, currently only `acr` registries are supported, with additional registry support planned.
*Note*, also, currently only Linux/Macos are supported w/ Windows support on the way.

### Installing from source

To install from source, first you'll need to install Rust. The best way to do that is here: https://rustup.rs

Next, clone this repo and from the root run, 

```
cargo install --path .
```

This will install a binary called `acr` on your PATH. 

## (TODO) Install dependencies

## Setting up an environment for the mirror

Next, pick a directory you would like to work with. This directory will be the "home" directory, and config files
will be "installed" here. 

Once you've done that, run the command `acr --registry {your-registry-name} init`. 

This will output a `mirror.runmd` file which has all of the config. You can open this file and render it in Markdown if you'd like a better viewing experience.

## Starting the mirror

Now to start the mirror, run this command, `acr --registry {your-registry-name} mirror start`, 

(If you'd like to run this in the background, `acr --registry {your-registry-name} mirror start &`)

### Troubleshooting start

It might be the case you'd like to customize the engine sequence, and in that case you might want to start at a different place. In that case, if you run `acr --registry {your-registry-name} dump`, you will get a map that looks like this: 

```
1: mirror
  2: install mirror
    3: process "lifec"
      arg List([Symbol("--runmd-path"), Symbol("lib/overlaybd/setup_env"), Symbol("start"), Symbol("--engine-name"), Symbol("ubuntu")])


  4: start mirror
        # file_src: Symbol(".world/azurecr.io/example/access_token")::Reference(17620018548555559471)
        # src_dir: Symbol(".")::Reference(4027234349407905632)
        # work_dir: Symbol(".world/azurecr.io/example")::Reference(10460137960410825781)
    5: login-acr "example"

    6: install "access_token"

    7: mirror "example.azurecr.io"
      app_host Single(Symbol("localhost:8578"))
      feature_localhost:8578 List([Symbol("resolve"), Symbol("pull")])



Engine control block: ``` mirror @ Entity(1, Generation(1))
  Loop(Entity(1, Generation(1)))
```

If you pass the `--id {id}` flag with `start`, you can start at any of the plugin calls. For example to start at the `.install` plugin, you would use `acr --registry {your-registry-name} mirror start --id 6`.

## Customizing the mirror

The `mirror.runmd` is self explanatory, and will have the most up to date information on how to configure the mirror. However, here are some high-level concepts to understand.

The runtime for this code is based on `lifec` which is a plugin based runtime. The only thing that is important to know, is that plugins typically are declared with a `dot` + `name of the plugin`, and then some input to the plugin. 

For example, `.println hello world`,

Here is an example mirror/proxy configuration,

```
+ .runtime
: .login-acr    example
: .install      access_token
: .mirror       example.azurecr.io
: .host         localhost:8578, resolve, pull

+ .proxy          localhost:8578
: .manifests      head, get
:   .login        access_token
:   .authn        oauth2
:   .resolve      application/vnd.docker.distribution.manifest.list.v2+json
:   .discover     dadi.image.v1
:   .teleport     overlaybd
: .blobs          get
:   .login        access_token
:   .authn        oauth2
:   .pull         
```

Just to re-iterate, the generated `mirror.runmd` is a better illustration of all the pieces put together. This is to give context to the following list of plugins this repository provides and a short explanation about each, 

* `.login-acr` - This plugin will use az cli to get an access token for the mirror to use for authn.
    - The input should be the name of the registry (`acr init` will handle this)

* `.login` - This plugin will add credentials to the plugin state for subsequent calls to use. 
    - The input should be the name of the credential to use. This credential is expected to be a file stored in the same directory as the `mirror.runmd` file. (This handled for you by default). 

* `.authn` - This plugin will authenticate with the registry with the original api that was received by the mirror.
    - The input is expected to be the authn type, (*TODO* Only `oauth2` is currently supported, but this will be updated in the future to support other types of authn)

* `.mirror` - This plugin will setup the mirror config w/ containerd. This includes generating a `hosts.toml` and copying it over to the correct directory. The `hosts.toml` file is generated before the mirror starts, and outputs to `.work/etc/containerd/certs.d/{registry-name}.{registry-host}/hosts.toml` 
    - The expected input is the full registry host name, i.e. {registry-name}.{registry-host} (`acr init` will handle this)
    - This plugin also installs a custom attribute called `.host`. This is attribute can be used to configure each mirror and their capabilities. Example usage is, `.host localhost:8555, resolve, pull`. (`acr init` will provide a default setting as an example)
    - TODO - There are additional custom attributes available such as `https` and `server`, need to document these.

* `.proxy` - This isn't exactly a plugin, it's more of a sub-engine that the `.mirror` plugin will use in order to customize each api that is being mirrored. 
    - The input it expects is the address of the proxy server. TODO (document how to enable tls)
    - This attribute will install 3 custom attributes, `.manifests`, `.blobs`, `.tags`. Each custom attribute expects the REST methods to implement, for example `.manifests head, get`. (`acr init` will create a default setting)

* `.resolve` - This plugin will call the upstream server from the original api, and save the response to state for subsequent plugins to operate on. 
    - The input is the media type(s) to accept (TODO currently this will override the original accept, WIP), it will be a 1:1 mapping to the Accept header, so that is the format the input should be in.

* `.discover` - This will use the ORAS referrer's api to search for artifacts for the current subject `digest` in state. If an artifact is found, it will attach it to state as the name of the artifact. 
    - The expected input is the name of the artifact type 

* `.teleport` - This is an experimental plugin that will resolve image references to a streamable format if such a reference exists. It will check for artifacts added by `.discover`
    - The expected input is the name of the format, currently only `overlaybd` is supported. 
    - This plugin also expects the current snapshotter pulling the image is capable of streaming the image in the first place. 

* `.pull` - This plugin will call the upstream server to download a blob. Only used in the context of the proxy.

## Getting started w/ Teleport 

(TODO)