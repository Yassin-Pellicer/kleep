use std::os::unix::fs::MetadataExt;

const MAX_RUNNING_PROGRAMS: usize = 120;

/// Lists the programs currently running for the current user. This is passed
/// back with every command result so the model knows what is open without
/// spending another tool round on `ps`.
pub(super) fn context() -> Option<String> {
    let uid = unsafe { libc::getuid() };
    let entries = std::fs::read_dir("/proc").ok()?;
    let mut names: Vec<String> = Vec::new();

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(pid) = file_name
            .to_str()
            .and_then(|value| value.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == 0 {
            continue;
        }

        let path = entry.path();
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.uid() != uid {
            continue;
        }

        let Ok(comm) = std::fs::read_to_string(path.join("comm")) else {
            continue;
        };
        let comm = comm.trim();
        if !comm.is_empty() {
            names.push(comm.to_string());
        }
    }

    if names.is_empty() {
        return None;
    }

    names.sort_unstable();
    names.dedup();
    let total = names.len();
    names.truncate(MAX_RUNNING_PROGRAMS);
    let mut list = names.join(", ");
    if total > names.len() {
        list.push_str(&format!(" … (+{} more)", total - names.len()));
    }

    Some(format!("programs currently running for this user:\n{list}"))
}
