use ql_java_handler::JavaInstallError;
use std::path::PathBuf;
use thiserror::Error;

use ql_core::{impl_3_errs_jri, json::VersionDetails, IoError, JsonError, RequestError};

use crate::{auth::ms::AuthError, download::DownloadError, jarmod::JarModError};

const GAME_ERR_PREFIX: &str = "while launching game:\n";

#[derive(Debug, Error)]
pub enum GameLaunchError {
    #[error("{GAME_ERR_PREFIX}{0}")]
    Io(#[from] IoError),
    #[error("{GAME_ERR_PREFIX}{0}")]
    Json(#[from] JsonError),
    #[error("{GAME_ERR_PREFIX}{0}")]
    Request(#[from] RequestError),

    #[error("username contains spaces")]
    UsernameHasSpaces,
    #[error("username is empty")]
    UsernameIsEmpty,
    #[error("{GAME_ERR_PREFIX}instance not found")]
    InstanceNotFound,
    #[error("{GAME_ERR_PREFIX}no arguments field in details.json")]
    VersionJsonNoArgumentsField(Box<VersionDetails>),

    #[error("{GAME_ERR_PREFIX}semver error: {0}")]
    Semver(#[from] semver::Error),
    #[error("{GAME_ERR_PREFIX}couldn't convert PathBuf to string: {0:?}")]
    PathBufToString(PathBuf),
    #[error("{GAME_ERR_PREFIX}couldn't run java command: {0}")]
    CommandError(std::io::Error),

    #[error("{GAME_ERR_PREFIX}{0}")]
    Download(#[from] DownloadError),
    #[error("{GAME_ERR_PREFIX}{0}")]
    JavaInstall(#[from] JavaInstallError),
    #[error("{GAME_ERR_PREFIX}{0}")]
    JarMod(#[from] JarModError),

    #[error("{GAME_ERR_PREFIX}{0}")]
    MsAuth(#[from] AuthError),
    #[error("{GAME_ERR_PREFIX}microsoft account token was not loaded\n\nTry logging out of your account and logging back in")]
    InvalidToken,

    #[error("{GAME_ERR_PREFIX}error upgrading forge install (transforming path)\n{FORGE_UPGRADE_MESSAGE}")]
    ForgeInstallUpgradeTransformPathError,
    #[error(
        "{GAME_ERR_PREFIX}error upgrading forge install (removing prefix)\n{FORGE_UPGRADE_MESSAGE}"
    )]
    ForgeInstallUpgradeStripPrefixError,
}

const FORGE_UPGRADE_MESSAGE: &str = r"outdated forge install. Please uninstall and reinstall.
Select your instance, go to Mods -> Uninstall Forge, then Install Forge.";

impl_3_errs_jri!(GameLaunchError, Json, Request, Io);
