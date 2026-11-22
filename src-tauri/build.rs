fn main() {
    #[cfg(target_os = "windows")]
    {
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let scripts_dir = std::path::PathBuf::from(manifest_dir).join("scripts");
            if scripts_dir.exists() {
                let current_path = std::env::var_os("PATH").unwrap_or_default();
                let mut paths = vec![scripts_dir];
                paths.extend(std::env::split_paths(&current_path));
                if let Ok(new_path) = std::env::join_paths(paths) {
                    // Append fallback windres.cmd for gnullvm environments without MinGW windres
                    std::env::set_var("PATH", new_path);
                }
            }
        }
    }
    tauri_build::build();
}
