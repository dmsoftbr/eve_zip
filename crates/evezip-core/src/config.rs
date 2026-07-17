use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub ultima_pasta: Option<PathBuf>,
    pub nivel_compressao: u8,
    pub formato_padrao: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ultima_pasta: None,
            nivel_compressao: 5,
            formato_padrao: "7z".into(),
        }
    }
}

impl Config {
    fn path() -> Option<PathBuf> {
        directories::ProjectDirs::from("br.com", "dmsoft", "evezip")
            .map(|d| d.config_dir().join("config.toml"))
    }

    pub(crate) fn parse(s: &str) -> Config {
        toml::from_str(s).unwrap_or_default()
    }

    pub fn load() -> Config {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|s| Self::parse(&s))
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(p) = Self::path() else {
            return Ok(());
        };
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(
            p,
            toml::to_string_pretty(self).expect("config serializável"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_quando_nao_existe() {
        let c = Config::default();
        assert_eq!(c.nivel_compressao, 5);
        assert_eq!(c.formato_padrao, "7z");
        assert!(c.ultima_pasta.is_none());
    }

    #[test]
    fn roundtrip_toml() {
        let c = Config {
            ultima_pasta: Some("/tmp".into()),
            nivel_compressao: 9,
            formato_padrao: "zip".into(),
        };
        let s = toml::to_string(&c).unwrap();
        let de: Config = toml::from_str(&s).unwrap();
        assert_eq!(de.nivel_compressao, 9);
        assert_eq!(de.formato_padrao, "zip");
    }

    #[test]
    fn toml_corrompido_vira_default() {
        assert_eq!(Config::parse("isto não é toml [[[").nivel_compressao, 5);
    }
}
