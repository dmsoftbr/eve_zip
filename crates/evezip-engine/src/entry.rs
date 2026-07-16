#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
    pub path: String,
    pub size: u64,
    pub packed_size: u64,
    /// Formato do 7zz: "2026-07-01 10:00:00" (mantido como string).
    pub modified: Option<String>,
    pub is_dir: bool,
    pub encrypted: bool,
}
