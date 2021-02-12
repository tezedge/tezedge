use shiplift::{
    rep::{ContainerDetails, ImageDetails},
    Docker,
};
use std::env;

use failure::bail;

pub const TEZEDGE_NODE_CONTAINER_NAME: &str = "watchdog-tezedge-node";
pub const TEZEDGE_DEBUGGER_CONTAINER_NAME: &str = "watchdog-tezedge-debugger";
pub const SANDBOX_CONTAINER_NAME: &str = "watchdog-tezedge-sandbox-launcher";
pub const OCAML_NODE_CONTAINER_NAME: &str = "watchdog-ocaml-node";
pub const OCAML_DEBUGGER_CONTAINER_NAME: &str = "watchdog-ocaml-debugger";
pub const EXPLORER_CONTAINER_NAME: &str = "watchdog-explorer";

pub trait Image {
    const TAG_ENV_KEY: &'static str;
    const IMAGE_NAME: &'static str;

    fn tag() -> String {
        env::var(Self::TAG_ENV_KEY).unwrap_or_else(|_| "latest".to_string())
    }

    fn name() -> String {
        format!("{}:{}", Self::IMAGE_NAME, Self::tag())
    }
}

pub async fn remote_hash<T: Image>() -> Result<String, failure::Error> {
    let url = format!(
        "https://hub.docker.com/v2/repositories/{}/tags/{}/?page_size=100",
        T::IMAGE_NAME,
        T::tag(),
    );
    match reqwest::get(&url).await {
        Ok(result) => {
            let res_json: serde_json::Value = match result.json().await {
                Ok(json) => json,
                Err(e) => failure::bail!("Error converting result to json: {:?}", e),
            };
            let digest = res_json["images"][0]["digest"].to_string();
            let digest = digest.trim_matches('"');
            Ok(format!("{}@{}", T::IMAGE_NAME, digest))
        }
        Err(e) => failure::bail!("Error getting latest image: {:?}", e),
    }
}

pub async fn local_hash<T: Image>(docker: &Docker) -> Result<String, failure::Error> {
    let ImageDetails { repo_digests, .. } = docker.images().get(&T::name()).inspect().await?;
    repo_digests
        .and_then(|v| v.first().cloned())
        .ok_or_else(|| failure::err_msg(format!("no such image {}", T::name())))
}

pub struct Debugger;

impl Image for Debugger {
    const TAG_ENV_KEY: &'static str = "TEZEDGE_DEBUGGER_IMAGE_TAG";
    const IMAGE_NAME: &'static str = "simplestakingcom/tezedge-debugger";
}

impl Debugger {
    pub async fn collect_commit_hash() -> Result<String, failure::Error> {
        let commit_hash = match reqwest::get("http://localhost:17732/v2/version").await {
            Ok(result) => result.text().await?,
            Err(e) => bail!("GET commit_hash error: {}", e),
        };

        Ok(commit_hash.trim_matches('"').to_string())
    }
}

pub struct Explorer;

impl Image for Explorer {
    const TAG_ENV_KEY: &'static str = "TEZEDGE_EXPLORER_IMAGE_TAG";
    const IMAGE_NAME: &'static str = "simplestakingcom/tezedge-explorer";
}

impl Explorer {
    pub async fn collect_commit_hash() -> Result<String, failure::Error> {
        let docker = Docker::new();
        let ContainerDetails { config, .. } = docker
            .containers()
            .get(EXPLORER_CONTAINER_NAME)
            .inspect()
            .await?;
        let env = config.env();

        if let Some(commit_hash) = env.get("COMMIT") {
            Ok(commit_hash.to_owned())
        } else {
            bail!("COMMIT env var not found in explorer contianer")
        }
    }
}

pub struct Sandbox;

impl Image for Sandbox {
    const TAG_ENV_KEY: &'static str = "TEZEDGE_SANDBOX_IMAGE_TAG";
    const IMAGE_NAME: &'static str = "simplestakingcom/tezedge";
}
