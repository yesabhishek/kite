use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AppZoom(u16);

impl AppZoom {
    pub const ALL: [Self; 7] = [
        Self(75),
        Self(80),
        Self(90),
        Self(100),
        Self(110),
        Self(125),
        Self(150),
    ];

    pub const fn percent(self) -> u16 {
        self.0
    }

    pub const fn scale_factor(self) -> f32 {
        self.percent() as f32 / 100.0
    }

    pub fn step(self, delta: i32) -> Self {
        let current = Self::ALL.iter().position(|zoom| *zoom == self).unwrap_or(3) as i32;
        let next = current
            .saturating_add(delta)
            .clamp(0, Self::ALL.len() as i32 - 1);
        Self::ALL[next as usize]
    }
}

impl Default for AppZoom {
    fn default() -> Self {
        Self(100)
    }
}

impl std::fmt::Display for AppZoom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}%", self.0)
    }
}

impl<'de> Deserialize<'de> for AppZoom {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let percent = i64::deserialize(deserializer)?;
        Ok(Self::ALL
            .into_iter()
            .min_by_key(|zoom| percent.abs_diff(i64::from(zoom.0)))
            .unwrap_or_default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub folder: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    pub font_size: f32,
    pub app_zoom: AppZoom,
    pub sidebar: bool,
    pub terminal: bool,
    pub theme: crate::appearance::Appearance,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            folder: None,
            files: vec![],
            font_size: 14.0,
            app_zoom: AppZoom::default(),
            sidebar: true,
            terminal: true,
            theme: Default::default(),
        }
    }
}
impl Settings {
    fn path() -> Option<PathBuf> {
        directories::ProjectDirs::from("io", "kite", "kite")
            .map(|d| d.config_dir().join("session.json"))
    }
    pub fn load() -> Self {
        let legacy = directories::ProjectDirs::from("io", "weyescode", "weyescode")
            .map(|d| d.config_dir().join("session.json"));
        Self::load_from(Self::path(), legacy)
    }
    fn load_from(current: Option<PathBuf>, legacy: Option<PathBuf>) -> Self {
        current
            .into_iter()
            .chain(legacy)
            .find_map(|p| {
                let bytes = std::fs::read(p).ok()?;
                serde_json::from_slice(&bytes).ok()
            })
            .unwrap_or_default()
    }
    pub fn store(&self) -> Result<(), String> {
        use std::io::Write;
        let path = Self::path().ok_or("Cannot locate the configuration directory")?;
        let parent = path.parent().ok_or("Invalid configuration path")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rebrand_restores_legacy_preferences_and_prefers_kite_preferences() {
        let directory = tempfile::tempdir().unwrap();
        let current = directory.path().join("kite.json");
        let legacy = directory.path().join("legacy.json");
        std::fs::write(&legacy, r#"{"font_size":18,"terminal":false}"#).unwrap();
        let restored = Settings::load_from(Some(current.clone()), Some(legacy.clone()));
        assert_eq!(restored.font_size, 18.);
        assert!(!restored.terminal);
        std::fs::write(&current, r#"{"font_size":16,"terminal":true}"#).unwrap();
        let restored = Settings::load_from(Some(current), Some(legacy));
        assert_eq!(restored.font_size, 16.);
        assert!(restored.terminal);
    }
    #[test]
    fn old_sessions_get_the_default_theme_and_new_sessions_roundtrip() {
        let old: Settings = serde_json::from_str(r#"{"terminal":false}"#).unwrap();
        assert_eq!(old.theme, crate::appearance::Appearance::Dark);
        assert_eq!(old.app_zoom, AppZoom::default());
        for theme in crate::appearance::Appearance::ALL {
            for app_zoom in AppZoom::ALL {
                let settings = Settings {
                    theme,
                    app_zoom,
                    ..Settings::default()
                };
                let restored: Settings =
                    serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
                assert_eq!(restored.theme, theme);
                assert_eq!(restored.app_zoom, app_zoom);
            }
        }
    }

    #[test]
    fn stored_zoom_normalizes_to_a_bounded_preset() {
        for (stored, expected) in [
            (i64::MIN, 75),
            (0, 75),
            (78, 80),
            (85, 80),
            (87, 90),
            (120, 125),
            (999, 150),
            (i64::MAX, 150),
        ] {
            let zoom: AppZoom = serde_json::from_str(&stored.to_string()).unwrap();
            assert_eq!(zoom.percent(), expected);
        }
    }

    #[test]
    fn zoom_steps_through_presets_and_clamps_at_each_end() {
        let zoom = AppZoom::default();
        assert_eq!(zoom.to_string(), "100%");
        assert_eq!(zoom.scale_factor(), 1.0);
        assert_eq!(zoom.step(-1).percent(), 90);
        assert_eq!(zoom.step(1).percent(), 110);
        assert_eq!(zoom.step(2).percent(), 125);
        assert_eq!(zoom.step(i32::MIN).percent(), 75);
        assert_eq!(zoom.step(i32::MAX).percent(), 150);
        assert_eq!(AppZoom::ALL[0].step(-1), AppZoom::ALL[0]);
        assert_eq!(AppZoom::ALL[6].step(1), AppZoom::ALL[6]);
    }
}
