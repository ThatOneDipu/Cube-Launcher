//! # A module for creating, managing and running Minecraft client instances
//!
//! This is a crate of
//! [Quantum Launcher](https://mrmayman.github.io/quantumlauncher)
//! for dealing with many operations of Minecraft instances.
//!
//! **Not recommended to use in your own projects!**
//!
//! This module contains functions to:
//! - Create an instance
//! - Launch the instance
//! - Update the launcher
//! - Read logs
//! - List versions available for download
//! - Authenticate with Microsoft Accounts
//!
//! # A rant about natives
//!
//! (probably not relevant for you)
//!
//! ## What are natives?
//! Natives are libraries that are platform-specific.
//! They are used by Minecraft to interface with the operating system.
//!
//! ## Types of natives
//! - `natives: *` - These are part of the main library
//!   but have a separate jar file included that is extracted to
//!   the `natives` folder.
//! - `name: *-natives-*` - They are a separate library
//!   whose jar file is extracted to the `natives` folder.
//! - `classifiers: *` - Once again, part of main library
//!   but have separate jar files for each OS. Just formatted
//!   differently in the json.
//!
//! These 3 separate types of natives make it a headache to
//! deal with all three correctly, WHILE JUGGLING ALONG
//! THIRD PARTY ARM64 SOURCES FOR LIBRARIES!!! (WTF: )
//!
//! ## The problem
//! Mojang has a habit of not including ARM64 natives in their
//! libraries (well they do sometimes but not always).
//!
//! This is a problem for ARM64 users as they can't
//! run the game without the natives.
//!
//! ## The solution
//! We download the ARM64 natives from two different sources:
//! - `./assets/lwjgl_arm64/*` - Providing natives for different LWJGL
//!   versions. Sourced from <https://github.com/theofficialgman/piston-meta-arm64>
//! - `./assets/minecraft_arm` - Providing natives for `LWJGL`,
//!   `Log4J`, `Oshi`, `JavaObjCBridge`, and `Slf4J`. Used less often.
//!   Sourced from <https://github.com/Kichura/Minecraft_ARM>
//!
//! Both of these complement each other and provide a complete
//! set of natives for ARM64 users.
//!
//! It's still a bit of a hack and it sometimes breaks but it works.

#![allow(clippy::missing_errors_doc)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]

pub mod auth;
mod download;
mod instance;
mod json_profiles;
mod launcher_update_detector;

pub use download::{constants::OS_NAME, DownloadError};
pub use instance::create::create_instance;
pub use instance::launch::launch;
pub use instance::list_versions::list_versions;
pub use instance::read_log::{read_logs, LogEvent, LogLine, ReadError};
pub use launcher_update_detector::{
    check_for_launcher_updates, install_launcher_update, UpdateCheckInfo, UpdateError,
};
pub use ql_core::jarmod;
pub use ql_java_handler::delete_java_installs;

use semver::{BuildMetadata, Prerelease};

const LAUNCHER_VERSION: semver::Version = semver::Version {
    major: 0,
    minor: 4,
    patch: 1,
    pre: Prerelease::EMPTY,
    build: BuildMetadata::EMPTY,
};
