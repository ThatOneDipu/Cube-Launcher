use std::collections::HashMap;

use crate::{file_utils, JsonDownloadError};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct JsonVersions {
    promos: HashMap<String, String>,
}

impl JsonVersions {
    /// Downloads the Forge versions JSON file from the Forge website.
    ///
    /// # Errors
    /// If the file cannot be:
    /// - Downloaded (maybe bad internet or server down).
    /// - Parsed into JSON.
    pub async fn download() -> Result<Self, JsonDownloadError> {
        const VERSIONS_JSON: &str =
            "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";

        file_utils::download_file_to_json(VERSIONS_JSON, false).await
    }

    /// Returns the Forge version for the given Minecraft version.
    #[must_use]
    pub fn get_forge_version(&self, minecraft_version: &str) -> Option<String> {
        self.promos
            .iter()
            .find(|(version_mc, _)| *version_mc == &format!("{minecraft_version}-latest"))
            .map(|n| n.1.clone())
    }
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct JsonInstallProfile {
    pub install: serde_json::Value,
    pub versionInfo: JsonDetails,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct JsonDetails {
    pub id: String,
    pub time: String,
    pub releaseTime: String,
    pub r#type: String,
    pub mainClass: String,
    pub inheritsFrom: Option<String>,
    pub logging: Option<serde_json::Value>,
    pub arguments: Option<JsonDetailsArguments>,
    pub libraries: Vec<JsonDetailsLibrary>,
    pub minecraftArguments: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct JsonDetailsArguments {
    pub game: Vec<String>,
    pub jvm: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
pub struct JsonDetailsLibrary {
    pub name: String,
    pub url: Option<String>,
    pub downloads: Option<JsonDetailsDownloads>,
    pub clientreq: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub struct JsonDetailsDownloads {
    pub artifact: JsonDetailsArtifact,
}

#[derive(Serialize, Deserialize)]
pub struct JsonDetailsArtifact {
    pub path: String,
    pub url: String,
    pub sha1: String,
    pub size: usize,
}
