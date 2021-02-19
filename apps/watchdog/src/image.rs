use shiplift::{
    rep::{ContainerDetails, ImageDetails},
    Docker,
};

use failure::bail;

use async_trait::async_trait;

pub const TEZEDGE_DEBUGGER_PORT: u16 = 17732;

#[async_trait]
pub trait WatchdogContainer {
    const NAME: &'static str;

    async fn image() -> Result<String, failure::Error> {
        let docker = Docker::new();

        let ContainerDetails { config, .. } = docker
            .containers()
            .get(Self::NAME)
            .inspect()
            .await?;
        Ok(config.image)
    }
}

pub async fn remote_hash<T: WatchdogContainer + Sync + Send>() -> Result<String, failure::Error> {
    let image = T::image().await?;
    let image_split: Vec<&str> = image.split(':').collect();
    let repo = image_split.get(0).unwrap_or(&"");
    let url = format!(
        "https://hub.docker.com/v2/repositories/{}/tags/{}/?page_size=100",
        repo,
        image_split.get(1).unwrap_or(&""),
    );
    match reqwest::get(&url).await {
        Ok(result) => {
            let res_json: serde_json::Value = match result.json().await {
                Ok(json) => json,
                Err(e) => failure::bail!("Error converting result to json: {:?}", e),
            };
            let digest = res_json["images"][0]["digest"].to_string();
            let digest = digest.trim_matches('"');
            Ok(format!("{}@{}", repo, digest))
        }
        Err(e) => failure::bail!("Error getting latest image: {:?}", e),
    }
}

pub async fn local_hash<T: WatchdogContainer + Sync + Send>(docker: &Docker) -> Result<String, failure::Error> {
    let image = T::image().await?;
    let ImageDetails { repo_digests, .. } = docker.images().get(&image).inspect().await?;
    repo_digests
        .and_then(|v| v.first().cloned())
        .ok_or_else(|| failure::err_msg(format!("no such image {}", image)))
}

pub struct TezedgeDebugger;

impl WatchdogContainer for TezedgeDebugger {
    const NAME: &'static str = "watchdog-tezedge-debugger";
}

impl TezedgeDebugger {
    pub async fn collect_commit_hash() -> Result<String, failure::Error> {
        let commit_hash = match reqwest::get(&format!("http://localhost:{}/v2/version", TEZEDGE_DEBUGGER_PORT)).await {
            Ok(result) => result.text().await?,
            Err(e) => bail!("GET commit_hash error: {}", e),
        };

        Ok(commit_hash.trim_matches('"').to_string())
    }
}

pub struct OcamlDebugger;

impl WatchdogContainer for OcamlDebugger {
    const NAME: &'static str = "watchdog-ocaml-debugger";
}

pub struct Explorer;

impl WatchdogContainer for Explorer {
    const NAME: &'static str = "watchdog-explorer";
}

impl Explorer {
    pub async fn collect_commit_hash() -> Result<String, failure::Error> {
        let docker = Docker::new();
        let ContainerDetails { config, .. } = docker
            .containers()
            .get(Self::NAME)
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

impl WatchdogContainer for Sandbox {
    const NAME: &'static str = "watchdog-tezedge-sandbox-launcher";
}
