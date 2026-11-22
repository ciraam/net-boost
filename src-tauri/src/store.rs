use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;

use crate::security::{dpapi_decrypt, dpapi_encrypt};
use crate::types::{AppConfig, BackupState, BoostOptions, PartialAppConfig, ServerPreset, WireGuardProfile};

pub fn default_presets() -> Vec<ServerPreset> {
    vec![
        ServerPreset {
            id: "preset-cf".into(),
            name: "Cloudflare Anycast".into(),
            region: "Cloudflare".into(),
            host: "1.1.1.1".into(),
            description: "Point de présence Cloudflare le plus proche (référence latence fibre/WAN)".into(),
        },
        ServerPreset {
            id: "preset-eu-paris".into(),
            name: "UE Ouest — Paris".into(),
            region: "UE".into(),
            host: "par-fr-ping.vultr.com".into(),
            description: "Datacenter Paris, France (serveurs compétitifs Europe de l'Ouest)".into(),
        },
        ServerPreset {
            id: "preset-eu-fra".into(),
            name: "UE Centre — Francfort".into(),
            region: "UE".into(),
            host: "fra-de-ping.vultr.com".into(),
            description: "Hub Francfort, Allemagne (Valorant, LoL, CS2, Apex EU Central)".into(),
        },
        ServerPreset {
            id: "preset-eu-lon".into(),
            name: "UE Nord — Londres".into(),
            region: "UE".into(),
            host: "lon-gb-ping.vultr.com".into(),
            description: "Datacenter Londres, Royaume-Uni".into(),
        },
        ServerPreset {
            id: "preset-na-east".into(),
            name: "NA Est — New Jersey".into(),
            region: "NA".into(),
            host: "nj-us-ping.vultr.com".into(),
            description: "Côte Est Amérique du Nord (New York / Virginie)".into(),
        },
        ServerPreset {
            id: "preset-na-west".into(),
            name: "NA Ouest — Los Angeles".into(),
            region: "NA".into(),
            host: "lax-ca-us-ping.vultr.com".into(),
            description: "Côte Ouest Amérique du Nord (Californie)".into(),
        },
        ServerPreset {
            id: "preset-google".into(),
            name: "Google Public DNS".into(),
            region: "DNS".into(),
            host: "8.8.8.8".into(),
            description: "Serveur DNS global Google".into(),
        },
        ServerPreset {
            id: "preset-quad9".into(),
            name: "Quad9 DNS".into(),
            region: "DNS".into(),
            host: "9.9.9.9".into(),
            description: "Serveur DNS global Quad9 sécurisé".into(),
        },
    ]
}

pub fn default_config() -> AppConfig {
    AppConfig {
        games: Vec::new(),
        presets: default_presets(),
        selected_game_id: None,
        selected_host: "1.1.1.1".into(),
        boost_options: BoostOptions::default(),
        tunnel_mode: "wiresock".into(),
        wireguard_profile: None,
    }
}

pub fn strip_wireguard_secrets(profile: Option<WireGuardProfile>) -> Option<WireGuardProfile> {
    profile.map(|mut p| {
        let was_enc = p.encrypted_conf.is_some() || p.is_encrypted;
        p.raw_conf = None;
        p.encrypted_conf = None;
        p.is_encrypted = was_enc;
        p
    })
}

#[derive(Clone)]
pub struct AppStore {
    user_data_dir: PathBuf,
    config_path: PathBuf,
    backup_path: PathBuf,
}

impl AppStore {
    pub fn new(user_data_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&user_data_dir);
        let config_path = user_data_dir.join("netboost-config.json");
        let backup_path = user_data_dir.join("backup-state.json");
        Self {
            user_data_dir,
            config_path,
            backup_path,
        }
    }

    pub fn user_data_dir(&self) -> &Path {
        &self.user_data_dir
    }

    pub fn backup_path(&self) -> &Path {
        &self.backup_path
    }

    fn encrypt_profile_for_disk(profile: Option<WireGuardProfile>) -> Result<Option<WireGuardProfile>, String> {
        let Some(mut p) = profile else {
            return Ok(None);
        };
        if let Some(raw) = p.raw_conf.take() {
            let enc = dpapi_encrypt(raw.as_bytes())?;
            p.encrypted_conf = Some(base64::engine::general_purpose::STANDARD.encode(enc));
            p.is_encrypted = true;
        } else if p.encrypted_conf.is_some() {
            p.is_encrypted = true;
        }
        Ok(Some(p))
    }

    pub fn load_config(&self) -> AppConfig {
        if !self.config_path.exists() {
            let cfg = default_config();
            let _ = self.write_raw_config(&cfg);
            return cfg;
        }
        let raw = match fs::read_to_string(&self.config_path) {
            Ok(r) => r,
            Err(_) => return default_config(),
        };
        let mut cfg: AppConfig = match serde_json::from_str(&raw) {
            Ok(c) => c,
            Err(_) => return default_config(),
        };
        if cfg.presets.is_empty() {
            cfg.presets = default_presets();
        }
        // Auto-migrate any legacy plaintext WireGuard rawConf to DPAPI encryption
        if cfg
            .wireguard_profile
            .as_ref()
            .map(|p| p.raw_conf.is_some() && p.encrypted_conf.is_none())
            .unwrap_or(false)
        {
            if let Ok(enc_profile) = Self::encrypt_profile_for_disk(cfg.wireguard_profile.clone()) {
                cfg.wireguard_profile = enc_profile;
                let _ = self.write_raw_config(&cfg);
            }
        }
        cfg
    }

    pub fn load_public_config(&self) -> AppConfig {
        let mut cfg = self.load_config();
        cfg.wireguard_profile = strip_wireguard_secrets(cfg.wireguard_profile);
        cfg
    }

    pub fn get_decrypted_wireguard_conf(&self) -> Result<Option<(WireGuardProfile, String)>, String> {
        let cfg = self.load_config();
        let Some(profile) = cfg.wireguard_profile else {
            return Ok(None);
        };
        if let Some(enc_b64) = &profile.encrypted_conf {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(enc_b64)
                .map_err(|e| format!("Base64 invalide pour encryptedConf : {}", e))?;
            let plain = dpapi_decrypt(&bytes)?;
            let text = String::from_utf8(plain)
                .map_err(|e| format!("UTF-8 invalide après déchiffrement DPAPI : {}", e))?;
            return Ok(Some((profile, text)));
        }
        if let Some(raw) = &profile.raw_conf {
            return Ok(Some((profile.clone(), raw.clone())));
        }
        Ok(None)
    }

    fn write_raw_config(&self, cfg: &AppConfig) -> Result<(), String> {
        let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
        fs::write(&self.config_path, json).map_err(|e| e.to_string())
    }

    pub fn save_config(&self, partial: PartialAppConfig) -> Result<AppConfig, String> {
        let mut current = self.load_config();
        if let Some(games) = partial.games {
            current.games = games;
        }
        if let Some(presets) = partial.presets {
            current.presets = presets;
        }
        if let Some(sel) = partial.selected_game_id {
            current.selected_game_id = sel;
        }
        if let Some(host) = partial.selected_host {
            current.selected_host = host;
        }
        if let Some(opts) = partial.boost_options {
            current.boost_options = opts;
        }
        if let Some(mode) = partial.tunnel_mode {
            current.tunnel_mode = if mode == "wireguard" {
                "wireguard".into()
            } else {
                "wiresock".into()
            };
        }
        if let Some(wg_opt) = partial.wireguard_profile {
            current.wireguard_profile = Self::encrypt_profile_for_disk(wg_opt)?;
        }

        self.write_raw_config(&current)?;
        let mut public = current;
        public.wireguard_profile = strip_wireguard_secrets(public.wireguard_profile);
        Ok(public)
    }

    pub fn has_backup(&self) -> bool {
        self.backup_path.exists()
    }

    pub fn read_backup(&self) -> Option<BackupState> {
        if !self.backup_path.exists() {
            return None;
        }
        let raw = fs::read_to_string(&self.backup_path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn write_backup(&self, backup: &BackupState) -> Result<(), String> {
        let json = serde_json::to_string_pretty(backup).map_err(|e| e.to_string())?;
        fs::write(&self.backup_path, json).map_err(|e| e.to_string())
    }

    pub fn delete_backup(&self) {
        if self.backup_path.exists() {
            let _ = fs::remove_file(&self.backup_path);
        }
    }
}
