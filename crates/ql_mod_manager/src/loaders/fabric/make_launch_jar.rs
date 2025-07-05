use ql_core::{info, IntoIoError};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use zip::{write::FileOptions, ZipWriter};

use super::error::FabricInstallError;

const MANIFEST_PATH: &str = "META-INF/MANIFEST.MF";
const SERVICES_DIR: &str = "META-INF/services/";
const MAIN_CLASS_MANIFEST: &str = "net.fabricmc.loader.impl.launch.server.FabricServerLauncher";

/// Makes a jar file that launches the Minecraft Fabric server,
/// essentially acting as a glorified launch script.
///
/// You just do `java -jar this_generated_file.jar` and it
/// automatically sets up the main class and the classpath
/// via its `META-INF/MANIFEST.MF` file. So essentially
/// this is done for ease of use.
///
/// # Errors
/// - If the server installation (the directory) doesn't exist
/// - If the user lacks permission to modify the server installation
///
/// Note: It will generate invalid classpath data if
/// the library filenames contains invalid character encodings.
pub async fn make_launch_jar(
    file: &Path,
    launch_main_class: &str,
    library_files: &[PathBuf],
    shade_libraries: bool,
) -> Result<(), FabricInstallError> {
    if file.exists() {
        tokio::fs::remove_file(file).await.path(file)?;
    }

    let zip_file = File::create(file).path(file)?;
    let mut zip_writer = ZipWriter::new(BufWriter::new(zip_file));
    let mut added_entries = HashSet::new();

    let mut manifest_content = ManifestBuilder::new();

    if !shade_libraries {
        let class_path = library_files
            .iter()
            .map(|library| {
                // Note: best to have relative paths to
                // libraries instead of absolute paths:
                //
                // - This avoids problems with spaces in paths
                //   (real bug fixed in v0.4)
                // - This makes the fabric server jar file cross platform
                library
                    .parent()
                    .and_then(|parent| library.strip_prefix(parent).ok())
                    .unwrap_or(library)
                    .to_string_lossy()
                    .to_string()
                    .replace('\\', "/")
            })
            .collect::<Vec<_>>()
            .join(" ");
        manifest_content.add_line(&format!("Class-Path: {class_path}"));
    }
    manifest_content.add_line(&format!("Main-Class: {MAIN_CLASS_MANIFEST}"));
    let manifest_content = manifest_content.build();

    zip_writer.start_file(MANIFEST_PATH, FileOptions::<()>::default())?;
    zip_writer
        .write_all(manifest_content.as_bytes())
        .map_err(|n| FabricInstallError::ZipEntryWriteError(n, MANIFEST_PATH.to_owned()))?;
    added_entries.insert(MANIFEST_PATH.to_string());

    // Write the fabric server launch properties
    let launch_properties = format!("launch.mainClass={launch_main_class}\n");
    let launch_properties_path = "fabric-server-launch.properties";
    zip_writer.start_file(launch_properties_path, FileOptions::<()>::default())?;
    zip_writer
        .write_all(launch_properties.as_bytes())
        .map_err(|n| {
            FabricInstallError::ZipEntryWriteError(n, launch_properties_path.to_owned())
        })?;
    added_entries.insert("fabric-server-launch.properties".to_string());

    // Shade libraries if required
    if shade_libraries {
        info!("Shading libraries");
        let mut services = HashMap::<String, HashSet<String>>::new();

        let library_files_len = library_files.len();

        let regex = regex::Regex::new(r"META-INF/[^/]+\.(SF|DSA|RSA|EC)").unwrap();

        for (i, library_path) in library_files.iter().enumerate() {
            info!("({i}/{library_files_len}) {library_path:?}");
            let library_file = File::open(library_path).path(library_path)?;
            let mut jar_reader = zip::read::ZipArchive::new(BufReader::new(library_file))?;

            for i in 0..jar_reader.len() {
                let mut entry = jar_reader.by_index(i)?;
                let name = entry.name().to_string();

                if entry.is_dir() {
                    continue;
                }

                if name.starts_with(SERVICES_DIR) && name[SERVICES_DIR.len()..].find('/').is_none()
                {
                    // Parse and merge service definitions
                    let data = std::io::read_to_string(&mut entry)
                        .map_err(|n| FabricInstallError::ZipEntryReadError(n, name.clone()))?;
                    parse_service_definition(&name, &data, &mut services);
                } else if regex.is_match(&name) {
                    // Ignore signature files
                } else if !added_entries.insert(name.clone()) {
                    // Duplicate entry, ignore
                } else {
                    // Write the entry to the output jar
                    zip_writer.start_file(&name, FileOptions::<()>::default())?;
                    io::copy(&mut entry, &mut zip_writer)
                        .map_err(|err| FabricInstallError::ZipEntryWriteError(err, name.clone()))?;
                }
            }
        }

        // Write the merged service definitions
        for (service_name, definitions) in services {
            zip_writer.start_file(&service_name, FileOptions::<()>::default())?;
            for definition in &definitions {
                writeln!(zip_writer, "{definition}").map_err(|err| {
                    FabricInstallError::ZipEntryWriteError(err, service_name.clone())
                })?;
            }
        }
    }

    zip_writer.finish()?;
    Ok(())
}

fn parse_service_definition(
    name: &str,
    data: &str,
    services: &mut HashMap<String, HashSet<String>>,
) {
    for line in data.lines() {
        let trimmed_line = line.split('#').next().unwrap_or("").trim();

        if !trimmed_line.is_empty() {
            services
                .entry(name.to_string())
                .or_default()
                .insert(trimmed_line.to_string());
        }
    }
}

struct ManifestBuilder {
    lines: Vec<String>,
}

impl ManifestBuilder {
    fn new() -> Self {
        Self {
            lines: vec!["Manifest-Version: 1.0".to_owned()],
        }
    }

    fn add_line(&mut self, line: &str) {
        let split = split_string(line);
        self.lines.extend(split);
    }

    fn build(self) -> String {
        let mut lines = self.lines.join("\n");
        lines.push('\n');
        lines
    }
}

fn split_string(s: &str) -> Vec<String> {
    let mut result = Vec::new();

    if s.len() <= 70 {
        result.push(s.to_string());
    } else {
        // Take the first 70 characters
        let first_part = s.chars().take(70).collect::<String>();
        result.push(first_part);

        // Get the remaining characters
        let remaining = s.chars().skip(70).collect::<String>();

        if remaining.len() <= 69 {
            result.push(format!(" {remaining}"));
        } else {
            // Split the remaining string into chunks of 69 characters
            result.extend(
                remaining
                    .chars()
                    .collect::<Vec<_>>()
                    .chunks(69)
                    .map(|chunk| format!(" {}", chunk.iter().collect::<String>())),
            );
        }
    }

    result
}
