use std::env;
use shiplift::{Docker, rep::ImageDetails};
use slog::Logger;

pub trait Image {
    const TAG_ENV_KEY: &'static str;
    const IMAGE_NAME: &'static str;

    fn tag() -> String {
        env::var(Self::TAG_ENV_KEY).unwrap_or("latest".to_string())
    }

    fn name() -> String {
        format!("{}:{}", Self::IMAGE_NAME, Self::tag())
    }
}

async fn remote_hash<T: Image>() -> Result<String, failure::Error> {
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

async fn local_hash<T: Image>(docker: &Docker) -> Result<String, failure::Error> {
    let ImageDetails { repo_digests, .. } = docker.images()
        .get(&T::name())
        .inspect()
        .await?;
    repo_digests.and_then(|v| v.first().cloned())
        .ok_or(failure::err_msg(format!("no such image {}", T::name())))
}

pub async fn changed<T: Image>(docker: &Docker, log: &Logger) -> Result<bool, failure::Error> {
    let local_image_hash = local_hash::<T>(docker).await?;
    let remote_image_hash = remote_hash::<T>().await?;

    if local_image_hash != remote_image_hash {
        slog::info!(
            log,
            "Image changed, local: {} != remote: {}",
            local_image_hash,
            remote_image_hash
        );
        Ok(true)
    } else {
        Ok(false)
    }
}

pub struct Node;

impl Image for Node {
    const TAG_ENV_KEY: &'static str = "TEZEDGE_IMAGE_TAG";
    const IMAGE_NAME: &'static str = "simplestakingcom/tezedge";
}

pub struct Debugger;

impl Image for Debugger {
    const TAG_ENV_KEY: &'static str = "TEZEDGE_DEBUGGER_IMAGE_TAG";
    const IMAGE_NAME: &'static str = "simplestakingcom/tezedge-debugger";
}

pub struct Explorer;

impl Image for Explorer {
    const TAG_ENV_KEY: &'static str = "TEZEDGE_EXPLORER_IMAGE_TAG";
    const IMAGE_NAME: &'static str = "simplestakingcom/tezedge-explorer";
}
