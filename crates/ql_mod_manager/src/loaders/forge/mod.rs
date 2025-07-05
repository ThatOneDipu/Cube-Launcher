use std::{
    fmt::Write,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc::Sender,
};

use error::Is404NotFound;
use ql_core::{
    err, file_utils, info,
    json::{
        forge::{JsonDetails, JsonDetailsLibrary, JsonInstallProfile, JsonVersions},
        VersionDetails,
    },
    no_window, pt, GenericProgress, InstanceSelection, IntoIoError, IntoJsonError, IoError,
    Progress, CLASSPATH_SEPARATOR,
};
use ql_java_handler::{get_java_binary, JavaVersion, JAVA};

use crate::loaders::change_instance_type;

mod error;
mod server;
pub use server::install_server;
mod uninstall;

pub use error::ForgeInstallError;
pub use uninstall::{uninstall, uninstall_client, uninstall_server};

struct ForgeInstaller {
    f_progress: Option<Sender<ForgeInstallProgress>>,
    norm_forge_version: String,
    short_version: String,
    major_version: usize,
    instance_dir: PathBuf,
    forge_dir: PathBuf,
    is_server: bool,
    version_json: VersionDetails,
}

impl ForgeInstaller {
    pub async fn delete(&self, path: &str) -> Result<(), IoError> {
        let delete_path = self.forge_dir.join(path);
        if delete_path.exists() {
            tokio::fs::remove_file(&delete_path)
                .await
                .path(delete_path)?;
        }
        Ok(())
    }

    async fn remove_lock(&self) -> Result<(), ForgeInstallError> {
        let lock_path = self.instance_dir.join("forge.lock");
        tokio::fs::remove_file(&lock_path).await.path(lock_path)?;
        Ok(())
    }

    async fn new(
        forge_version: Option<String>, // example: "11.15.1.2318" for 1.8.9
        f_progress: Option<Sender<ForgeInstallProgress>>,
        instance: InstanceSelection,
    ) -> Result<Self, ForgeInstallError> {
        let instance_dir = instance.get_instance_path();
        let forge_dir = if instance.is_server() {
            instance_dir.clone()
        } else {
            get_forge_dir(&instance_dir).await?
        };

        let version_json = VersionDetails::load(&instance).await?;
        let minecraft_version = &version_json.id;

        create_mods_dir(&instance_dir).await?;
        create_lock_file(&instance_dir).await?;

        pt!("Downloading JSON");
        if let Some(progress) = &f_progress {
            progress
                .send(ForgeInstallProgress::P2DownloadingJson)
                .unwrap();
        }

        let version = if let Some(n) = forge_version {
            n
        } else {
            get_forge_version(minecraft_version).await?
        };

        info!("Forge version {version} is being installed");

        let norm_version = {
            let number_of_full_stops = minecraft_version.chars().filter(|c| *c == '.').count();
            if number_of_full_stops == 1 {
                format!("{minecraft_version}.0")
            } else {
                minecraft_version.clone()
            }
        };
        let short_version = format!("{minecraft_version}-{version}");
        let norm_forge_version = format!("{short_version}-{norm_version}");
        let major_version: usize = version.split('.').next().unwrap_or(&version).parse()?;

        Ok(Self {
            f_progress,
            norm_forge_version,
            short_version,
            major_version,
            instance_dir,
            forge_dir,
            is_server: instance.is_server(),
            version_json,
        })
    }

    async fn download_forge_installer(
        &self,
    ) -> Result<(Vec<u8>, String, PathBuf), ForgeInstallError> {
        let (file_type, file_type_flipped) = if self.major_version < 14 {
            ("universal", "installer")
        } else {
            ("installer", "universal")
        };

        pt!("Downloading Installer");
        self.send_progress(ForgeInstallProgress::P3DownloadingInstaller);

        let installer_file = self.try_downloading_from_urls(&[
            &format!("https://files.minecraftforge.net/maven/net/minecraftforge/forge/{ver}/forge-{ver}-{file_type}.jar", ver = self.short_version),
            &format!("https://files.minecraftforge.net/maven/net/minecraftforge/forge/{ver}/forge-{ver}-{file_type}.jar", ver = self.norm_forge_version),
            &format!("https://files.minecraftforge.net/maven/net/minecraftforge/forge/{ver}/forge-{ver}-{file_type_flipped}.jar", ver = self.short_version),
            &format!("https://files.minecraftforge.net/maven/net/minecraftforge/forge/{ver}/forge-{ver}-{file_type_flipped}.jar", ver = self.norm_forge_version),
            // Minecraft 1.1 to 1.5.1: Install as jarmod
            &format!("https://files.minecraftforge.net/maven/net/minecraftforge/forge/{}/forge-{}-client.zip", self.short_version, self.short_version),
            &format!("https://files.minecraftforge.net/maven/net/minecraftforge/forge/{}/forge-{}-client.zip", self.norm_forge_version, self.norm_forge_version),
            &format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{}/forge-{}-universal.zip", self.short_version, self.short_version),
            &format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{}/forge-{}-universal.zip", self.norm_forge_version, self.norm_forge_version),
        ]).await?;

        let installer_name = format!("forge-{}-{file_type}.jar", self.short_version);
        let installer_path = self.forge_dir.join(&installer_name);
        tokio::fs::write(&installer_path, &installer_file)
            .await
            .path(&installer_path)?;
        Ok((installer_file, installer_name, installer_path))
    }

    fn send_progress(&self, message: ForgeInstallProgress) {
        if let Some(progress) = &self.f_progress {
            progress.send(message).unwrap();
        }
    }

    async fn try_downloading_from_urls(&self, urls: &[&str]) -> Result<Vec<u8>, ForgeInstallError> {
        let num_urls = urls.len();
        for (i, url) in urls.iter().enumerate() {
            let result = file_utils::download_file_to_bytes(url, false).await;

            match result {
                Ok(file) => {
                    pt!("({url})");
                    return Ok(file);
                }
                Err(err) => {
                    let is_last_url = i + 1 == num_urls;
                    if err.is_not_found() && !is_last_url {
                        continue;
                    }
                    return Err(ForgeInstallError::Request(err));
                }
            }
        }
        panic!("Forge installer: Reached invalid state (while retrying downloads)")
    }

    async fn run_installer_and_get_classpath(
        &self,
        installer_name: &str,
        j_progress: Option<&Sender<GenericProgress>>,
    ) -> Result<(PathBuf, String), ForgeInstallError> {
        let libraries_dir = self.forge_dir.join("libraries");
        tokio::fs::create_dir_all(&libraries_dir)
            .await
            .path(&libraries_dir)?;

        let classpath = if self.major_version >= 14 {
            // 1.12+
            self.run_installer(j_progress, installer_name).await?;

            if self.major_version < 39 {
                // 1.12 - 1.18
                format!(
                    "../forge/libraries/net/minecraftforge/forge/{}/forge-{}.jar{CLASSPATH_SEPARATOR}",
                    self.short_version,
                    self.short_version
                )
            } else {
                // 1.18.1+
                String::new()
            }
        } else {
            // 1.1 - 1.11.2
            format!("../forge/{installer_name}{CLASSPATH_SEPARATOR}")
        };
        Ok((libraries_dir, classpath))
    }

    async fn run_installer(
        &self,
        j_progress: Option<&Sender<GenericProgress>>,
        installer_name: &str,
    ) -> Result<(), ForgeInstallError> {
        let javac_path = get_java_binary(JavaVersion::Java21, "javac", j_progress).await?;
        let java_source_file = include_str!("../../../../../assets/installers/ForgeInstaller.java")
            .replace("CLIENT", if self.is_server { "SERVER" } else { "CLIENT" });
        let source_path = self.forge_dir.join("ForgeInstaller.java");
        tokio::fs::write(&source_path, java_source_file)
            .await
            .path(source_path)?;

        if !self.is_server {
            let launcher_profiles_json_path = self.forge_dir.join("launcher_profiles.json");
            tokio::fs::write(&launcher_profiles_json_path, "{}")
                .await
                .path(launcher_profiles_json_path)?;
            let launcher_profiles_json_microsoft_store_path = self
                .forge_dir
                .join("launcher_profiles_microsoft_store.json");
            tokio::fs::write(&launcher_profiles_json_microsoft_store_path, "{}")
                .await
                .path(launcher_profiles_json_microsoft_store_path)?;
        }

        pt!("Compiling Installer");
        self.send_progress(ForgeInstallProgress::P4RunningInstaller);
        let mut command = Command::new(&javac_path);
        command
            .args(["-cp", installer_name, "ForgeInstaller.java", "-d", "."])
            .current_dir(&self.forge_dir);
        no_window!(command);

        let output = command.output().path(javac_path)?;
        if !output.status.success() {
            return Err(ForgeInstallError::CompileError(
                String::from_utf8(output.stdout)?,
                String::from_utf8(output.stderr)?,
            ));
        }

        let java_path = get_java_binary(JavaVersion::Java21, JAVA, None).await?;
        pt!("Running Installer");
        let mut command = Command::new(&java_path);
        command
            .args([
                "-cp",
                &format!("{installer_name}{CLASSPATH_SEPARATOR}."),
                "ForgeInstaller",
            ])
            .current_dir(&self.forge_dir);

        let output = command.output().path(java_path)?;
        if !output.status.success() {
            return Err(ForgeInstallError::InstallerError(
                String::from_utf8(output.stdout)?,
                String::from_utf8(output.stderr)?,
            ));
        }
        Ok(())
    }

    fn get_forge_json(
        &self,
        installer_file: &[u8],
    ) -> Result<(JsonDetails, String), ForgeInstallError> {
        let mut zip =
            zip::ZipArchive::new(Cursor::new(installer_file)).map_err(ForgeInstallError::Zip)?;

        if let Ok(mut file) = zip.by_name("version.json") {
            let forge_json = std::io::read_to_string(&mut file);
            let forge_json = forge_json
                .map_err(|n| ForgeInstallError::ZipIoError(n, "version.json".to_owned()))?;

            let forge_json_parsed: JsonDetails =
                serde_json::from_str(&forge_json).json(forge_json.clone())?;

            return Ok((forge_json_parsed, forge_json));
        }
        if let Ok(mut file) = zip.by_name("install_profile.json") {
            let forge_json = std::io::read_to_string(&mut file);
            let forge_json = forge_json
                .map_err(|n| ForgeInstallError::ZipIoError(n, "install_profile.json".to_owned()))?;

            return if let Ok(forge_json_parsed) = serde_json::from_str(&forge_json) {
                let forge_json_parsed: JsonInstallProfile = forge_json_parsed;

                let to_string = serde_json::to_string(&forge_json_parsed.versionInfo)
                    .unwrap_or(forge_json.clone());
                Ok((forge_json_parsed.versionInfo, to_string))
            } else {
                let forge_json_parsed: JsonDetails =
                    serde_json::from_str(&forge_json).json(forge_json.clone())?;
                Ok((forge_json_parsed, forge_json))
            };
        }
        Err(ForgeInstallError::NoInstallJson(
            self.version_json.id.clone(),
        ))
    }

    async fn download_library(
        &self,
        library: &JsonDetailsLibrary,
        library_i: usize,
        num_libraries: usize,
        libraries_dir: &Path,
        classpath: &mut String,
        clean_classpath: &mut String,
    ) -> Result<(), ForgeInstallError> {
        let parts: Vec<&str> = library.name.split(':').collect();
        let class = parts[0];
        let lib = parts[1];
        let ver = parts[2];

        _ = writeln!(clean_classpath, "{class}:{lib}");

        let (file, path) = Self::get_filename_and_path(lib, ver, library, class)?;

        if class == "net.minecraftforge" && lib == "forge" {
            if self.major_version > 48 {
                Self::add_to_classpath(classpath, &path, &file);
            }
            pt!("Built in forge library, skipping...");
            return Ok(());
        }

        let url = if let Some(downloads) = &library.downloads {
            downloads.artifact.url.clone()
        } else {
            let baseurl = if let Some(url) = &library.url {
                url.to_owned()
            } else {
                "https://libraries.minecraft.net/".to_owned()
            };
            format!("{baseurl}{path}/{file}")
        };

        let lib_dir_path = libraries_dir.join(&path);
        tokio::fs::create_dir_all(&lib_dir_path)
            .await
            .path(&lib_dir_path)?;

        let dest = lib_dir_path.join(&file);

        self.send_progress(ForgeInstallProgress::P5DownloadingLibrary {
            num: library_i + 1,
            out_of: num_libraries,
        });

        if dest.exists() {
            pt!(
                "Skipping library ({}/{num_libraries}): {} (already exists)",
                library_i + 1,
                library.name
            );
        } else {
            pt!(
                "Downloading library ({}/{num_libraries}): {}",
                library_i + 1,
                library.name
            );

            let result = file_utils::download_file_to_path(&url, false, &dest).await;
            if result.is_not_found() {
                err!("Error 404 not found. Skipping...");
                return Ok(());
            }
            result?;
        }

        Self::add_to_classpath(classpath, &path, &file);

        Ok(())
    }

    fn add_to_classpath(classpath: &mut String, path: &str, file: &str) {
        let classpath_item = format!("../forge/libraries/{path}/{file}{CLASSPATH_SEPARATOR}");
        // println!("adding library to classpath {classpath_item}");
        classpath.push_str(&classpath_item);
    }

    fn get_filename_and_path(
        lib: &str,
        ver: &str,
        library: &JsonDetailsLibrary,
        class: &str,
    ) -> Result<(String, String), ForgeInstallError> {
        let (file, path) = if let Some(downloads) = &library.downloads {
            let parent = PathBuf::from(&downloads.artifact.path)
                .parent()
                .ok_or(ForgeInstallError::LibraryParentError)?
                .to_owned();
            (
                PathBuf::from(&downloads.artifact.path)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
                parent
                    .to_str()
                    .ok_or(ForgeInstallError::PathBufToStr(parent.clone()))?
                    .to_owned(),
            )
        } else {
            (
                format!("{lib}-{ver}.jar"),
                format!("{}/{lib}/{ver}", class.replace('.', "/")),
            )
        };
        Ok((file, path))
    }
}

async fn get_forge_version(minecraft_version: &str) -> Result<String, ForgeInstallError> {
    let json = JsonVersions::download().await?;
    let version = json
        .get_forge_version(minecraft_version)
        .ok_or(ForgeInstallError::NoForgeVersionFound)?;
    Ok(version)
}

async fn get_forge_dir(instance_dir: &Path) -> Result<PathBuf, ForgeInstallError> {
    let forge_dir = instance_dir.join("forge");
    tokio::fs::create_dir_all(&forge_dir)
        .await
        .path(&forge_dir)?;
    Ok(forge_dir)
}

async fn create_mods_dir(instance_dir: &Path) -> Result<(), ForgeInstallError> {
    let mods_dir_path = instance_dir.join(".minecraft/mods");
    tokio::fs::create_dir_all(&mods_dir_path)
        .await
        .path(mods_dir_path)?;
    Ok(())
}

async fn create_lock_file(instance_dir: &Path) -> Result<(), ForgeInstallError> {
    let lock_path = instance_dir.join("forge.lock");
    if lock_path.exists() {
        err!("Previously incomplete installation of forge found! (not a problem)");
    } else {
        tokio::fs::write(
            &lock_path,
            "If you see this, forge was not installed correctly.",
        )
        .await
        .path(lock_path)?;
    }
    Ok(())
}

pub async fn install(
    forge_version: Option<String>, // example: "11.15.1.2318" for 1.8.9
    instance: InstanceSelection,
    f_progress: Option<Sender<ForgeInstallProgress>>,
    j_progress: Option<Sender<GenericProgress>>,
) -> Result<(), ForgeInstallError> {
    match instance {
        InstanceSelection::Instance(name) => {
            install_client(forge_version, name, f_progress, j_progress).await
        }
        InstanceSelection::Server(name) => {
            install_server(forge_version, name, j_progress, f_progress).await
        }
    }
}

pub enum ForgeInstallProgress {
    P1Start,
    P2DownloadingJson,
    P3DownloadingInstaller,
    P4RunningInstaller,
    P5DownloadingLibrary { num: usize, out_of: usize },
    P6Done,
}

impl Default for ForgeInstallProgress {
    fn default() -> Self {
        Self::P1Start
    }
}

impl Progress for ForgeInstallProgress {
    fn get_num(&self) -> f32 {
        match self {
            ForgeInstallProgress::P1Start => 0.0,
            ForgeInstallProgress::P2DownloadingJson => 1.0,
            ForgeInstallProgress::P3DownloadingInstaller => 2.0,
            ForgeInstallProgress::P4RunningInstaller => 3.0,
            ForgeInstallProgress::P5DownloadingLibrary { num, out_of } => {
                3.0 + (*num as f32 / *out_of as f32)
            }
            ForgeInstallProgress::P6Done => 4.0,
        }
    }

    fn get_message(&self) -> Option<String> {
        Some(match self {
            ForgeInstallProgress::P1Start => "Installing forge...".to_owned(),
            ForgeInstallProgress::P2DownloadingJson => "Downloading JSON".to_owned(),
            ForgeInstallProgress::P3DownloadingInstaller => "Downloading installer".to_owned(),
            ForgeInstallProgress::P4RunningInstaller => {
                "Running Installer (this might take a while)".to_owned()
            }
            ForgeInstallProgress::P5DownloadingLibrary { num, out_of } => {
                format!("Downloading Library ({num}/{out_of})")
            }
            ForgeInstallProgress::P6Done => "Done!".to_owned(),
        })
    }

    fn total() -> f32 {
        4.0
    }
}

pub async fn install_client(
    forge_version: Option<String>,
    instance_name: String,
    f_progress: Option<Sender<ForgeInstallProgress>>,
    j_progress: Option<Sender<GenericProgress>>,
) -> Result<(), ForgeInstallError> {
    info!("Started installing forge");

    if let Some(progress) = &f_progress {
        _ = progress.send(ForgeInstallProgress::P1Start);
    }

    let mut installer = ForgeInstaller::new(
        forge_version,
        f_progress,
        InstanceSelection::Instance(instance_name.clone()),
    )
    .await?;

    let (installer_file, installer_name, _) = installer.download_forge_installer().await?;

    if installer.version_json.is_legacy_version() && installer.version_json.id != "1.5.2" {
        ql_core::jarmod::insert(
            InstanceSelection::Instance(instance_name.clone()),
            installer_file,
            "Forge",
        )
        .await?;

        return Ok(());
    }

    let (libraries_dir, mut classpath) = installer
        .run_installer_and_get_classpath(&installer_name, j_progress.as_ref())
        .await?;

    let mut clean_classpath = String::new();

    let (forge_json, forge_json_str) = installer.get_forge_json(&installer_file)?;

    let libs: Vec<JsonDetailsLibrary> = forge_json
        .libraries
        .into_iter()
        .filter(|library| !matches!(library.clientreq, Some(false)))
        .collect();
    let num_libraries = libs.len();

    for (library_i, library) in libs.into_iter().enumerate() {
        installer
            .download_library(
                &library,
                library_i,
                num_libraries,
                &libraries_dir,
                &mut classpath,
                &mut clean_classpath,
            )
            .await?;
    }

    let classpath_path = installer.forge_dir.join("classpath.txt");
    tokio::fs::write(&classpath_path, &classpath)
        .await
        .path(classpath_path)?;

    let clean_classpath_path = installer.forge_dir.join("clean_classpath.txt");
    tokio::fs::write(&clean_classpath_path, &clean_classpath)
        .await
        .path(clean_classpath_path)?;

    let json_path = installer.forge_dir.join("details.json");
    tokio::fs::write(
        &json_path,
        serde_json::to_string(&forge_json_str).json_to()?,
    )
    .await
    .path(json_path)?;

    change_instance_type(&installer.instance_dir, "Forge".to_owned()).await?;

    installer.remove_lock().await?;
    info!("Finished installing forge");
    Ok(())
}
