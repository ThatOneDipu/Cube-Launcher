use iced::{widget, Length};
use ql_core::{InstanceSelection, Progress};

use crate::{
    config::LauncherConfig,
    icon_manager,
    state::{
        CreateInstanceMessage, InstallModsMessage, LauncherSettingsMessage, ManageModsMessage,
        MenuCreateInstance, MenuCurseforgeManualDownload, MenuLauncherSettings, MenuLauncherUpdate,
        MenuServerCreate, Message, ProgressBar,
    },
    stylesheet::{color::Color, styles::LauncherTheme},
};

pub mod changelog;
mod edit_instance;
mod launch;
mod log;
mod login;
mod mods;

pub const DISCORD: &str = "https://discord.gg/3QWbVheFaC";
pub const GITHUB: &str = "https://github.com/ThatOneDipu/";

pub type Element<'a> = iced::Element<'a, Message, LauncherTheme, iced::Renderer>;

fn center_x<'a>(e: impl Into<Element<'a>>) -> Element<'a> {
    widget::row![
        widget::horizontal_space(),
        e.into(),
        widget::horizontal_space(),
    ]
    .into()
}

pub fn tooltip<'a>(e: impl Into<Element<'a>>, tooltip: impl Into<Element<'a>>) -> Element<'a> {
    widget::tooltip(e, tooltip, widget::tooltip::Position::Bottom)
        .style(|n: &LauncherTheme| n.style_container_sharp_box(0.0, Color::ExtraDark))
        .into()
}

pub fn back_button<'a>() -> widget::Button<'a, Message, LauncherTheme, iced::Renderer> {
    button_with_icon(icon_manager::back_with_size(14), "Back", 14)
}

pub fn button_with_icon<'element>(
    icon: Element<'element>,
    text: &'element str,
    size: u16,
) -> iced::widget::Button<'element, Message, LauncherTheme, iced::Renderer> {
    widget::button(
        widget::row![icon, widget::text(text).size(size)]
            .align_y(iced::alignment::Vertical::Center)
            .spacing(10)
            .padding(3),
    )
}

pub fn shortcut_ctrl<'a>(key: &str) -> Element<'a> {
    #[cfg(target_os = "macos")]
    return widget::text!("Command + {key}").size(12).into();

    #[cfg(not(target_os = "macos"))]
    return widget::text!("Control + {key}").size(12).into();
}

impl MenuCreateInstance {
    pub fn view(&self) -> Element {
        match self {
            MenuCreateInstance::LoadingList { .. } => widget::column![
                widget::row![
                    back_button().on_press(Message::CreateInstance(CreateInstanceMessage::Cancel)),
                    button_with_icon(icon_manager::folder(), "Import Instance", 16)
                        .on_press(Message::CreateInstance(CreateInstanceMessage::Import)),
                ]
                .spacing(5),
                widget::text("Loading version list...").size(20),
            ]
            .padding(10)
            .spacing(10)
            .into(),
            MenuCreateInstance::Choosing {
                instance_name,
                selected_version,
                download_assets,
                combo_state,
                ..
            } => {
                widget::scrollable(
                    widget::column![
                        widget::row![
                            back_button()
                                .on_press(
                                    Message::LaunchScreenOpen {
                                        message: None,
                                        clear_selection: false
                                }),
                            button_with_icon(icon_manager::folder(), "Import Instance", 16)
                                .on_press(Message::CreateInstance(CreateInstanceMessage::Import)),
                        ]
                        .spacing(5),
                        widget::combo_box(combo_state, "Select a version...", selected_version.as_ref(), |version| {
                            Message::CreateInstance(CreateInstanceMessage::VersionSelected(version))
                        }),
                        widget::text_input("Enter instance name...", instance_name)
                            .on_input(|n| Message::CreateInstance(CreateInstanceMessage::NameInput(n))),
                        tooltip(
                            widget::checkbox("Download assets?", *download_assets).on_toggle(|t| Message::CreateInstance(CreateInstanceMessage::ChangeAssetToggle(t))),
                            widget::text("If disabled, creating instance will be MUCH faster, but no sound or music will play in-game").size(12),
                        ),
                        widget::button(widget::row![icon_manager::create(), "Create Instance"]
                                .spacing(10)
                                .padding(5)
                        ).on_press_maybe((selected_version.is_some() && !instance_name.is_empty()).then(|| Message::CreateInstance(CreateInstanceMessage::Start))),
                        widget::text("To install Fabric/Forge/OptiFine/Quilt, click on Mods after installing the instance").size(12),
                    ].push_maybe(
                        {
                            let real_platform = if cfg!(target_arch = "x86") { "x86_64" } else { "aarch64" };
                            (cfg!(target_os = "linux") && (cfg!(target_arch = "x86") || cfg!(target_arch = "arm")))
                            .then_some(
                                widget::column![
                                    // WARN: Linux i686 and arm32
                                    widget::text("Warning: On your platform (Linux 32 bit) only Minecraft 1.16.5 and below are supported.").size(20),
                                    widget::text!("If your computer isn't outdated, you might have wanted to download QuantumLauncher 64 bit ({real_platform})"),
                                ]
                            )})
                    .spacing(10)
                    .padding(10),
                )
                .style(LauncherTheme::style_scrollable_flat_dark)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
            }
            MenuCreateInstance::DownloadingInstance(progress) => widget::column![
                widget::text("Downloading Instance..").size(20),
                progress.view()
            ]
            .padding(10)
            .spacing(5)
            .into(),
            MenuCreateInstance::ImportingInstance(progress) => widget::column![
                widget::text("Importing Instance..").size(20),
                progress.view()
            ]
            .padding(10)
            .spacing(5)
            .into(),
        }
    }
}

impl MenuLauncherUpdate {
    pub fn view(&self) -> Element {
        if let Some(progress) = &self.progress {
            widget::column!("Updating CubeLauncher...", progress.view())
        } else {
            widget::column!(
                "A new launcher update has been found! Do you want to download it?",
                widget::row!(
                    button_with_icon(icon_manager::download(), "Download", 16)
                        .on_press(Message::UpdateDownloadStart),
                    back_button().on_press(
                        Message::LaunchScreenOpen {
                            message: None,
                            clear_selection: false
                        }
                    ),
                    button_with_icon(icon_manager::page(), "Open Website", 16)
                        .on_press(Message::CoreOpenLink("".to_owned())),
                ).push_maybe((cfg!(target_os = "linux")).then_some(
                    widget::column!(
                        // WARN: Package manager
                        "Note: If you installed this launcher from a package manager (flatpak/apt/dnf/pacman/..) it's recommended to update from there",
                        "If you just downloaded it from the website then continue from here."
                    )
                )).push_maybe((cfg!(target_os = "macos")).then_some(
                    // WARN: macOS updater
                    "Note: The updater may be broken on macOS, so download the new version from the website"
                ))
                .spacing(5),
            )
        }
        .padding(10)
        .spacing(10)
        .into()
    }
}

impl MenuLauncherSettings {
    pub fn view<'a>(&'a self, config: &'a LauncherConfig) -> Element<'a> {
        let (theme_list, style_list) = get_themes_and_styles(config);

        let config_view = widget::row!(
            widget::container(
                widget::column!(
                    "Select theme:",
                    theme_list,
                )
                .padding(10)
                .spacing(10)
            ),
            widget::container(
                widget::column!(
                    "Select style:",
                    style_list
                )
                .padding(10)
                .spacing(10)
            ),
            widget::container(
                widget::column![
                    "Change UI Scaling: (warning: slightly buggy)",
                    widget::slider(0.5..=2.0, self.temp_scale, |n| Message::LauncherSettings(
                        LauncherSettingsMessage::UiScale(n)
                    ))
                    .step(0.1),
                    widget::text!("Scale: {:.2}x", self.temp_scale),
                    widget::button("Apply").on_press(Message::LauncherSettings(
                        LauncherSettingsMessage::UiScaleApply
                    ))
                ]
                .padding(10)
                .spacing(10)
            ),
            widget::container(widget::column![
                button_with_icon(icon_manager::delete(), "Clear Java installs", 16)
                    .on_press(Message::LauncherSettings(LauncherSettingsMessage::ClearJavaInstalls)),
                widget::text("Might fix any problems with Java. Should be safe, you just need to redownload the Java Runtime").size(12),
            ].padding(10).spacing(10))
        )
        .spacing(10)
        .wrap();

        widget::scrollable(
            widget::column!(
                back_button().on_press(
                    Message::LaunchScreenOpen {
                        message: None,
                        clear_selection: false
                    }
                ),
                config_view,
                widget::container(
                    widget::column!(
                        widget::row![
                            widget::button("Changelog").on_press(Message::CoreOpenChangeLog),
                            widget::button("Welcome Screen").on_press(Message::CoreOpenIntro),
                        ].spacing(5).wrap(),
                        widget::row![
                            button_with_icon(icon_manager::page(), "Website", 16).on_press(
                                Message::CoreOpenLink(
                                    "".to_owned()
                                )
                            ),
                            button_with_icon(icon_manager::github(), "Github", 16).on_press(
                                Message::CoreOpenLink(
                                    GITHUB.to_owned()
                                )
                            ),
                            button_with_icon(icon_manager::chat(), "Discord", 16).on_press(
                                Message::CoreOpenLink(DISCORD.to_owned())
                            ),
                        ].spacing(5).wrap(),
                        widget::column![
                            widget::text(r"CubeLauncher is software under the GNU GPLv3 license.
No warranty is provided for this software.
You're free to share, modify, and redistribute it under the same license.").size(12),
                            widget::button("View License").on_press(
                                Message::CoreOpenLink("https://www.gnu.org/licenses/gpl-3.0.en.html".to_owned())
                            ),
                        ].spacing(5),
                        widget::text(r"If you like this launcher, consider sharing it with your friends.
Every new user motivates me to keep working on this :)").size(12)
                    )
                    .padding(10)
                    .spacing(10)
                ),
            )
            .padding(10)
            .spacing(10),
        )
        .style(LauncherTheme::style_scrollable_flat_extra_dark)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

fn get_themes_and_styles(config: &LauncherConfig) -> (Element, Element) {
    // HOOK: Add more themes
    let themes = ["Dark".to_owned(), "Light".to_owned()];
    let styles = [
        "Brown".to_owned(),
        "Purple".to_owned(),
        "Sky Blue".to_owned(),
        "Catppuccin".to_owned(),
        "Teal".to_owned(),
    ];

    let theme_list = widget::pick_list(themes, config.theme.clone(), |n| {
        Message::LauncherSettings(LauncherSettingsMessage::ThemePicked(n))
    })
    .into();

    let style_list = widget::pick_list(styles, config.style.clone(), |n| {
        Message::LauncherSettings(LauncherSettingsMessage::StylePicked(n))
    })
    .into();
    (theme_list, style_list)
}

fn back_to_launch_screen(
    selected_instance: &InstanceSelection,
    message: Option<String>,
) -> Message {
    match selected_instance {
        InstanceSelection::Server(selected_server) => Message::ServerManageOpen {
            selected_server: Some(selected_server.clone()),
            message,
        },
        InstanceSelection::Instance(_) => Message::LaunchScreenOpen {
            message: None,
            clear_selection: false,
        },
    }
}

impl<T: Progress> ProgressBar<T> {
    pub fn view(&self) -> Element {
        let total = T::total();
        if let Some(message) = &self.message {
            widget::column!(
                widget::progress_bar(0.0..=total, self.num),
                widget::text(message)
            )
        } else {
            widget::column!(widget::progress_bar(0.0..=total, self.num),)
        }
        .spacing(10)
        .into()
    }
}

impl MenuCurseforgeManualDownload {
    pub fn view(&self) -> Element {
        widget::column![
            "Some Curseforge mods have blocked this launcher!\nYou need to manually download the files and add them to your mods",

            widget::scrollable(
                widget::column(self.unsupported.iter().map(|entry| {
                    let url = format!(
                        "https://www.curseforge.com/minecraft/{}/{}/download/{}",
                        entry.project_type,
                        entry.slug,
                        entry.file_id
                    );

                    widget::row![
                        widget::button(widget::text("Open link").size(14)).on_press(Message::CoreOpenLink(url)),
                        widget::text(&entry.name)
                    ]
                    .align_y(iced::Alignment::Center)
                    .spacing(10)
                    .into()
                }))
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(LauncherTheme::style_scrollable_flat_extra_dark),

            "Warning: Ignoring this may lead to crashes!",
            widget::row![
                widget::button("+ Select above downloaded files").on_press(Message::ManageMods(ManageModsMessage::AddFile)),
                widget::button("Continue").on_press(if self.is_store {
                    Message::InstallMods(InstallModsMessage::Open)
                } else {
                    Message::ManageMods(ManageModsMessage::ScreenOpenWithoutUpdate)
                }),
            ].spacing(5)
        ]
        .padding(10)
        .spacing(10)
        .into()
    }
}

impl MenuServerCreate {
    pub fn view(&self) -> Element {
        match self {
            MenuServerCreate::LoadingList => {
                widget::column!(widget::text("Loading version list...").size(20),)
            }
            MenuServerCreate::Loaded {
                name,
                versions,
                selected_version,
                ..
            } => {
                widget::column!(
                    back_button().on_press(Message::ServerManageOpen {
                        selected_server: None,
                        message: None
                    }),
                    widget::text("Create new server").size(20),
                    widget::combo_box(
                        versions,
                        "Select a version...",
                        selected_version.as_ref(),
                        Message::ServerCreateVersionSelected
                    ),
                    widget::text_input("Enter server name...", name)
                        .on_input(Message::ServerCreateNameInput),
                    widget::button("Create Server").on_press_maybe(
                        (selected_version.is_some() && !name.is_empty())
                            .then(|| Message::ServerCreateStart)
                    ),
                )
            }
            MenuServerCreate::Downloading { progress } => {
                widget::column!(widget::text("Creating Server...").size(20), progress.view())
            }
        }
        .padding(10)
        .spacing(10)
        .into()
    }
}
