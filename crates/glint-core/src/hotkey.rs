//! Global hotkey registration.
//!
//! On KDE the native path is `org.kde.KGlobalAccel`; on other Wayland
//! compositors the standard `org.freedesktop.portal.GlobalShortcuts` portal is
//! the fallback. X11 sessions use the same KGlobalAccel path on KDE, and can
//! use XGrabKey later.

use std::sync::Arc;

use crate::session::is_kde;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    SelectionCapture,
    ToggleHud,
    ScreenCapture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyChord {
    pub key: char,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub meta: bool,
}

impl KeyChord {
    pub const fn alt(key: char) -> Self {
        Self {
            key,
            alt: true,
            ctrl: false,
            shift: false,
            meta: false,
        }
    }

    pub const fn alt_shift(key: char) -> Self {
        Self {
            key,
            alt: true,
            ctrl: false,
            shift: true,
            meta: false,
        }
    }

    /// Encode a chord in Qt's `QKeySequence` integer form, which is what
    /// KGlobalAccel stores in `kglobalshortcutsrc`.
    pub fn to_qt_key(self) -> i32 {
        const SHIFT: i32 = 0x0200_0000;
        const CTRL: i32 = 0x0400_0000;
        const ALT: i32 = 0x0800_0000;
        const META: i32 = 0x1000_0000;

        let mut code = self.key.to_ascii_uppercase() as i32;
        if self.shift {
            code |= SHIFT;
        }
        if self.ctrl {
            code |= CTRL;
        }
        if self.alt {
            code |= ALT;
        }
        if self.meta {
            code |= META;
        }
        code
    }
}

#[derive(Debug, Clone)]
pub struct HotkeyBinding {
    pub action: HotkeyAction,
    pub id: &'static str,
    pub description: &'static str,
    pub key: KeyChord,
}

pub fn default_bindings() -> Vec<HotkeyBinding> {
    vec![
        HotkeyBinding {
            action: HotkeyAction::SelectionCapture,
            id: "selection-capture",
            description: "划词透视",
            key: KeyChord::alt('Q'),
        },
        HotkeyBinding {
            action: HotkeyAction::ScreenCapture,
            id: "screen-capture",
            description: "截屏透视",
            key: KeyChord::alt('W'),
        },
        HotkeyBinding {
            action: HotkeyAction::ToggleHud,
            id: "toggle-hud",
            description: "显隐伴生卡片",
            key: KeyChord::alt_shift('D'),
        },
    ]
}

pub type HotkeyCallback = Arc<dyn Fn(HotkeyAction) + Send + Sync + 'static>;

#[derive(Debug, thiserror::Error)]
pub enum HotkeyError {
    #[error("hotkey backend unavailable: {0}")]
    Unavailable(String),
    #[error("hotkey registration failed: {0}")]
    Registration(String),
    #[error("hotkey backend is not implemented on this platform")]
    Unsupported,
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    /// `SetPresent | IsDefault` from KGlobalAccel::SetShortcutFlag.
    const SET_PRESENT_AND_DEFAULT: u32 = 2 | 4;

    #[zbus::proxy(
        interface = "org.kde.KGlobalAccel",
        default_service = "org.kde.kglobalaccel",
        default_path = "/kglobalaccel"
    )]
    trait KGlobalAccel {
        #[zbus(name = "doRegister")]
        fn do_register(&self, action_ids: &[String]) -> zbus::Result<()>;

        #[zbus(name = "setShortcut")]
        fn set_shortcut(
            &self,
            action_ids: &[String],
            keys: &[i32],
            flags: u32,
        ) -> zbus::Result<Vec<i32>>;

        #[zbus(name = "unRegister")]
        fn un_register(&self, action_ids: &[String]) -> zbus::Result<()>;
    }

    #[zbus::proxy(interface = "org.kde.kglobalaccel.Component", default_service = "org.kde.kglobalaccel")]
    trait KGlobalAccelComponent {
        #[zbus(signal, name = "globalShortcutPressed")]
        fn global_shortcut_pressed(
            &self,
            component: String,
            action: String,
            timestamp: i64,
        ) -> zbus::Result<()>;
    }

    pub struct KGlobalAccelEngine {
        /// KGlobalAccel component id. It must NOT end in `.desktop`:
        /// kglobalacceld treats such names as desktop-file service actions,
        /// looks the file up through KService, and silently drops the
        /// registration when no matching `.desktop` file exists.
        app_id: String,
        display_name: String,
    }

    impl KGlobalAccelEngine {
        pub fn new(app_id: impl Into<String>, display_name: impl Into<String>) -> Self {
            Self {
                app_id: app_id.into(),
                display_name: display_name.into(),
            }
        }

        /// `com.glint.desktop` → `com.glint.Glint`, preserving the reverse-DNS
        /// prefix but avoiding the special `.desktop` suffix.
        pub fn component_id_for(app_id: &str) -> String {
            if app_id.ends_with(".desktop") {
                format!("{}.Glint", app_id.trim_end_matches(".desktop"))
            } else {
                app_id.to_string()
            }
        }

        fn action_id(&self, binding: &HotkeyBinding) -> Vec<String> {
            vec![
                Self::component_id_for(&self.app_id),
                binding.id.to_string(),
                self.display_name.clone(),
                binding.description.to_string(),
            ]
        }

        fn component_path(&self) -> String {
            format!(
                "/component/{}",
                Self::component_id_for(&self.app_id).replace('.', "_")
            )
        }

        pub async fn register(
            &self,
            bindings: Vec<HotkeyBinding>,
            callback: HotkeyCallback,
        ) -> Result<(), HotkeyError> {
            let connection = zbus::Connection::session()
                .await
                .map_err(|err| HotkeyError::Unavailable(err.to_string()))?;

            let kg = KGlobalAccelProxy::new(&connection)
                .await
                .map_err(|err| HotkeyError::Unavailable(err.to_string()))?;

            // KGlobalAccel's `as` argument is ONE four-field action id, not a
            // flat list of many actions. Register and bind one at a time.
            for binding in &bindings {
                let action_id = self.action_id(binding);
                kg.do_register(&action_id)
                    .await
                    .map_err(|err| HotkeyError::Registration(err.to_string()))?;
                kg.set_shortcut(
                    &action_id,
                    &[binding.key.to_qt_key()],
                    SET_PRESENT_AND_DEFAULT,
                )
                .await
                .map_err(|err| HotkeyError::Registration(err.to_string()))?;
            }

            let path = self.component_path();
            let component = {
                let mut last_err = None;
                let mut proxy = None;
                for _ in 0..10 {
                    match KGlobalAccelComponentProxy::builder(&connection)
                        .path(path.clone())
                        .and_then(|builder| Ok(builder))
                    {
                        Ok(builder) => match builder.build().await {
                            Ok(value) => {
                                proxy = Some(value);
                                break;
                            }
                            Err(err) => last_err = Some(err.to_string()),
                        },
                        Err(err) => last_err = Some(err.to_string()),
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                proxy.ok_or_else(|| {
                    HotkeyError::Unavailable(
                        last_err.unwrap_or_else(|| "component proxy not ready".to_string()),
                    )
                })?
            };

            let mut signal_stream = component
                .receive_global_shortcut_pressed()
                .await
                .map_err(|err| HotkeyError::Unavailable(err.to_string()))?;

            let bindings_for_task = bindings.clone();
            tokio::spawn(async move {
                use futures_util::StreamExt;
                while let Some(signal) = signal_stream.next().await {
                    let Ok(args) = signal.args() else { continue };
                    if let Some(binding) = bindings_for_task.iter().find(|b| b.id == args.action) {
                        callback(binding.action);
                    }
                }
            });

            Ok(())
        }

        pub async fn unregister(&self, bindings: &[HotkeyBinding]) -> Result<(), HotkeyError> {
            let connection = zbus::Connection::session()
                .await
                .map_err(|err| HotkeyError::Unavailable(err.to_string()))?;
            let kg = KGlobalAccelProxy::new(&connection)
                .await
                .map_err(|err| HotkeyError::Unavailable(err.to_string()))?;
            for binding in bindings {
                let action_id = self.action_id(binding);
                kg.un_register(&action_id)
                    .await
                    .map_err(|err| HotkeyError::Registration(err.to_string()))?;
            }
            Ok(())
        }
    }

    pub struct PortalHotkeyEngine {
        app_id: String,
    }

    impl PortalHotkeyEngine {
        pub fn new(app_id: impl Into<String>) -> Self {
            Self {
                app_id: app_id.into(),
            }
        }

        pub async fn register(
            &self,
            bindings: Vec<HotkeyBinding>,
            callback: HotkeyCallback,
        ) -> Result<(), HotkeyError> {
            use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
            use futures_util::StreamExt;

            let app_id = ashpd::AppID::try_from(self.app_id.as_str())
                .map_err(|err| HotkeyError::Unavailable(err.to_string()))?;
            ashpd::register_host_app(app_id)
                .await
                .map_err(|err| HotkeyError::Unavailable(err.to_string()))?;

            let proxy = GlobalShortcuts::new()
                .await
                .map_err(|err| HotkeyError::Unavailable(err.to_string()))?;
            let session = proxy
                .create_session(Default::default())
                .await
                .map_err(|err| HotkeyError::Registration(err.to_string()))?;

            let shortcuts: Vec<NewShortcut> = bindings
                .iter()
                .map(|binding| NewShortcut::new(binding.id, binding.description))
                .collect();

            proxy
                .bind_shortcuts(&session, &shortcuts, None, Default::default())
                .await
                .map_err(|err| HotkeyError::Registration(err.to_string()))?;

            let mut activated = proxy
                .receive_activated()
                .await
                .map_err(|err| HotkeyError::Unavailable(err.to_string()))?;

            let bindings_for_task = bindings.clone();
            tokio::spawn(async move {
                while let Some(event) = activated.next().await {
                    let id = event.shortcut_id().to_string();
                    if let Some(binding) = bindings_for_task.iter().find(|b| b.id == id) {
                        callback(binding.action);
                    }
                }
            });

            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
pub enum PlatformHotkeyEngine {
    KGlobalAccel(linux::KGlobalAccelEngine),
    Portal(linux::PortalHotkeyEngine),
}

#[cfg(target_os = "linux")]
impl PlatformHotkeyEngine {
    pub fn new(app_id: impl Into<String>, display_name: impl Into<String>) -> Self {
        let app_id = app_id.into();
        if is_kde() {
            Self::KGlobalAccel(linux::KGlobalAccelEngine::new(app_id, display_name))
        } else {
            Self::Portal(linux::PortalHotkeyEngine::new(app_id))
        }
    }

    pub async fn register(
        &self,
        bindings: Vec<HotkeyBinding>,
        callback: HotkeyCallback,
    ) -> Result<(), HotkeyError> {
        match self {
            Self::KGlobalAccel(engine) => engine.register(bindings, callback).await,
            Self::Portal(engine) => engine.register(bindings, callback).await,
        }
    }

    pub async fn unregister(&self, bindings: &[HotkeyBinding]) -> Result<(), HotkeyError> {
        match self {
            Self::KGlobalAccel(engine) => engine.unregister(bindings).await,
            Self::Portal(_) => Ok(()),
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub struct PlatformHotkeyEngine;

#[cfg(not(target_os = "linux"))]
impl PlatformHotkeyEngine {
    pub fn new(_app_id: impl Into<String>, _display_name: impl Into<String>) -> Self {
        Self
    }

    pub async fn register(
        &self,
        _bindings: Vec<HotkeyBinding>,
        _callback: HotkeyCallback,
    ) -> Result<(), HotkeyError> {
        Err(HotkeyError::Unsupported)
    }

    pub async fn unregister(&self, _bindings: &[HotkeyBinding]) -> Result<(), HotkeyError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_alt_q_like_kde() {
        assert_eq!(KeyChord::alt('Q').to_qt_key(), 134_217_809);
        assert_eq!(KeyChord::alt('W').to_qt_key(), 134_217_815);
        assert_eq!(KeyChord::alt_shift('D').to_qt_key(), 167_772_228);
    }

    #[test]
    fn kglobalaccel_component_never_ends_in_desktop() {
        assert_eq!(
            linux::KGlobalAccelEngine::component_id_for("com.glint.desktop"),
            "com.glint.Glint"
        );
        assert_eq!(
            linux::KGlobalAccelEngine::component_id_for("com.glint.Glint"),
            "com.glint.Glint"
        );
    }

    #[test]
    fn default_bindings_are_distinct() {
        let bindings = default_bindings();
        let mut ids: Vec<_> = bindings.iter().map(|b| b.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), bindings.len());
    }
}
