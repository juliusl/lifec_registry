use std::{
    path::PathBuf,
};

use clap::Args;
use serde::Serialize;
use tinytemplate::TinyTemplate;
use tracing::{event, Level};

/// Initializes a runmd file for importing and formatting an image for teleport,
///
/// Creates an artifact manifest to link the original image and the formatted image. 
/// 
#[derive(Args, Clone, Serialize)]
pub struct Init {
    /// The tags to initialize this template with
    ///
    #[arg(last = true)]
    pub tags: Vec<String>,
    /// The source repo that images will be imported from,
    /// 
    /// The listed tags must also be present in the source,
    ///
    #[clap(long)]
    pub source: String,

    #[clap(long, default_value_t=String::from("all"))]
    pub platform: String, 
    #[clap(skip)]
    pub format: String,
    #[clap(skip)]
    pub repo: String,
    #[clap(skip)]
    pub registry_name: String,
    #[clap(skip)]
    pub registry_host: String,
    #[clap(skip)]
    tag: String,
}

impl Init {
    /// Initialize teleport format settings,
    ///
    pub async fn init(&self) {
        let repo_dir = PathBuf::from(format!(
            ".world/{}/{}/{}",
            self.registry_host, self.registry_name, self.repo
        ));

        tokio::fs::create_dir_all(&repo_dir)
            .await
            .expect("should be able to create repo folder");

        let mut tt = TinyTemplate::new();
        tt.add_template("format", FORMAT_TELEPORT_TEMPLATE)
            .expect("Should be able to add template");

        for tag in self.tags.iter() {
            self.init_tag(&repo_dir, tag, &tt).await 
        }
    }

    async fn init_tag<'a>(
        &'a self,
        repo_dir: &'a PathBuf,
        tag: impl AsRef<str>,
        tt: &TinyTemplate<'a>,
    ) {
        let mut init_tag_settings = self.clone();

        init_tag_settings.tag = tag.as_ref().to_string();

        let rendered = tt
            .render("format", &init_tag_settings)
            .expect("Should be able to render template");

        let tag_dir = repo_dir.join(tag.as_ref()); 

        tokio::fs::create_dir_all(&tag_dir).await.expect("should be able to create dirs");
        
        let format_file = tag_dir.join(format!("{}.runmd", self.format));

        if format_file.exists() {
            event!(Level::WARN, "Overwriting existing file {:?}", format_file);
        }

        match tokio::fs::write(format_file, rendered).await {
            Ok(_) => {
                event!(
                    Level::INFO,
                    "Wrote runmd file, recommend tracking the .world dir with source control"
                );
            }
            Err(err) => {
                event!(Level::ERROR, "Could not initialize format settings, {err}");
            }
        }
    }
}

/// Engine template for formatting an image to a teleportable format
/// 
pub const FORMAT_TELEPORT_TEMPLATE: &'static str = r#"
# Format {repo} for {format}
- This files defines an engine for formatting {repo} into a streamable image

## Control Settings 
- These are the control settings for the below engine,

``` {format}
: registry_host     .symbol {registry_host}
: registry_name     .symbol {registry_name}
: repo              .symbol {repo}
: reference         .symbol {tag}
: ns                .symbol {registry_name}.{registry_host}
: api               .symbol https://{registry_name}.{registry_host}/v2/{repo}/manifests/{tag}
: method            .symbol PUT
: file_src          .symbol .world/{registry_host}/{registry_name}/access_token
: work_dir          .symbol .world/{registry_host}/{registry_name}
: src_dir           .symbol .


+ .engine
: .event    setup       <Login to ACR and setup login info for {format}>
: .event    convert     <Convert an image in registry to {format}>
: .event    link        <Link {format} image to source image in registry>
: .exit
```

## Setup the environment
``` setup {format}
+ .runtime
: .login-acr        {registry_name}
: .login-{format}   
```

## Convert an image to a streamable format
- Will convert the image and push to {registry_name}.{registry_host}/{repo}:{tag}-{format}

``` convert {format}
+ .runtime
: .login-acr            {registry_name}
: .install              access_token
: .login                access_token
: .authn                oauth2
: .format-{format}      
```

## Create links from the original images to their streamable format
- Will create a link between {registry_name}.{registry_host}/{repo}:{tag} and {registry_name}.{registry_host}/{repo}:{tag}-{format}

``` link {format}
+ .runtime
: .login-acr    {registry_name}
: .install      access_token
: .login        access_token
: .authn        oauth2
: .artifact     teleport.link.v1
: .subject      https://{registry_name}.{registry_host}/v2/{repo}/manifests/{tag}
: .blob         https://{registry_name}.{registry_host}/v2/{repo}/manifests/{tag}-{format}
```
"#;
