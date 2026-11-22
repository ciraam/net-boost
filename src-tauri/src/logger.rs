use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use base64::Engine;
use chrono::Local;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::types::CommandLogEntry;

const MAX_LOGS: usize = 400;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Default)]
pub struct CommandLogger {
    entries: Arc<Mutex<Vec<CommandLogEntry>>>,
    app: Arc<Mutex<Option<AppHandle>>>,
}

impl CommandLogger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attach_app(&self, handle: AppHandle) {
        if let Ok(mut guard) = self.app.lock() {
            *guard = Some(handle);
        }
    }

    pub fn emit(&self, entry: CommandLogEntry) {
        if let Ok(mut list) = self.entries.lock() {
            if let Some(existing) = list.iter_mut().find(|e| e.id == entry.id) {
                *existing = entry.clone();
            } else {
                list.push(entry.clone());
                if list.len() > MAX_LOGS {
                    list.remove(0);
                }
            }
        }
        if let Ok(guard) = self.app.lock() {
            if let Some(app) = guard.as_ref() {
                let _ = app.emit("event:log", &entry);
            }
        }
    }

    pub fn info(&self, category: &str, command: &str, stdout: &str, status: &str) {
        let exit_code = if status == "error" { Some(1) } else { Some(0) };
        let stderr = if status == "error" {
            stdout.to_string()
        } else {
            String::new()
        };
        self.emit(CommandLogEntry {
            id: Uuid::new_v4().to_string(),
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            category: category.to_string(),
            command: command.to_string(),
            exit_code,
            stdout: stdout.to_string(),
            stderr,
            duration_ms: 0,
            status: status.to_string(),
        });
    }

    pub fn get_logs(&self) -> Vec<CommandLogEntry> {
        self.entries
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    pub fn clear(&self) {
        if let Ok(mut g) = self.entries.lock() {
            g.clear();
        }
    }

    /// Runs a PowerShell command only for Windows subsystems without a direct Win32 C API
    /// (NetQosPolicy, NetAdapterAdvancedProperty, on-demand ETW NetEventSession).
    /// Never used for polling in gameplay!
    pub fn run_powershell(
        &self,
        script: &str,
        category: &str,
        display_command: &str,
        silent: bool,
    ) -> ExecResult {
        let start = Instant::now();
        let id = Uuid::new_v4().to_string();
        let shown = format!("powershell> {}", display_command);

        if !silent {
            self.emit(CommandLogEntry {
                id: id.clone(),
                timestamp: Local::now().format("%H:%M:%S").to_string(),
                category: category.to_string(),
                command: shown.clone(),
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                duration_ms: 0,
                status: "running".to_string(),
            });
        }

        let wrapped = format!(
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8\n$ErrorActionPreference = 'Stop'\n{}",
            script
        );
        let utf16: Vec<u8> = wrapped
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);

        let output = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-EncodedCommand",
                &encoded,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        let duration_ms = start.elapsed().as_millis() as u64;
        let res = match output {
            Ok(out) => {
                let code = out.status.code().unwrap_or(1);
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                ExecResult {
                    ok: out.status.success(),
                    exit_code: code,
                    stdout,
                    stderr,
                    duration_ms,
                }
            }
            Err(e) => ExecResult {
                ok: false,
                exit_code: 1,
                stdout: String::new(),
                stderr: e.to_string(),
                duration_ms,
            },
        };

        if !silent {
            self.emit(CommandLogEntry {
                id,
                timestamp: Local::now().format("%H:%M:%S").to_string(),
                category: category.to_string(),
                command: shown,
                exit_code: Some(res.exit_code),
                stdout: res.stdout.chars().take(2000).collect(),
                stderr: res.stderr.chars().take(2000).collect(),
                duration_ms: res.duration_ms,
                status: if res.ok { "ok" } else { "error" }.to_string(),
            });
        }

        res
    }

    pub fn run_exe(
        &self,
        exe: &str,
        args: &[&str],
        category: &str,
        silent: bool,
    ) -> ExecResult {
        let start = Instant::now();
        let id = Uuid::new_v4().to_string();
        let shown = format!("{} {}", exe, args.join(" "));

        if !silent {
            self.emit(CommandLogEntry {
                id: id.clone(),
                timestamp: Local::now().format("%H:%M:%S").to_string(),
                category: category.to_string(),
                command: shown.clone(),
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                duration_ms: 0,
                status: "running".to_string(),
            });
        }

        let output = Command::new(exe)
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        let duration_ms = start.elapsed().as_millis() as u64;
        let res = match output {
            Ok(out) => {
                let code = out.status.code().unwrap_or(1);
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                ExecResult {
                    ok: out.status.success(),
                    exit_code: code,
                    stdout,
                    stderr,
                    duration_ms,
                }
            }
            Err(e) => ExecResult {
                ok: false,
                exit_code: 1,
                stdout: String::new(),
                stderr: e.to_string(),
                duration_ms,
            },
        };

        if !silent {
            self.emit(CommandLogEntry {
                id,
                timestamp: Local::now().format("%H:%M:%S").to_string(),
                category: category.to_string(),
                command: shown,
                exit_code: Some(res.exit_code),
                stdout: res.stdout.chars().take(2000).collect(),
                stderr: res.stderr.chars().take(2000).collect(),
                duration_ms: res.duration_ms,
                status: if res.ok { "ok" } else { "error" }.to_string(),
            });
        }

        res
    }
}

#[derive(Debug, Clone)]
pub struct ExecResult {
    pub ok: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

pub fn ps_single_quote(val: &str) -> String {
    format!("'{}'", val.replace('\'', "''"))
}
