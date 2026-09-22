//! Frozen actor source inputs. Admission covers Rust-owned discovery tables,
//! paths, source buffers and copies, plus one native directory object on the
//! audited Linux/glibc platform. Parser/template compiler allocations, generated
//! crowds and hydrated worlds are NOT covered. This owner cannot authorize
//! complete application adoption.
use super::*;
use cathedral_sim::checkpoint::{CheckpointBudget, Reservation};
use std::{fmt, io::Read, mem::size_of, sync::Arc};

pub const MAX_SOURCES: usize = 4096;
pub const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_TOTAL_SOURCE_BYTES: usize = 32 * 1024 * 1024;
const MAX_DIRECTORIES: usize = 256;
const MAX_DEPTH: usize = 8;
const MAX_ENTRIES: usize = 16_384;
const MAX_RELATIVE_PATH_BYTES: usize = 1024;
const MAX_ABSOLUTE_PATH_BYTES: usize = 4096;
const FIXED: [&str; 7] = [
    "world/seed.json",
    "core_lore/occupations.json",
    "world/areas.json",
    "sounds/catalog.toml",
    "prompts/turn.j2",
    "prompts/night.j2",
    "prompts/strings.toml",
];
const MAX_CHARACTERS: usize = MAX_SOURCES - FIXED.len();

// Audited x86_64 Linux/GNU, Ubuntu glibc 2.35-0ubuntu3.15, 4096-byte pages,
// ordinary ptmalloc mappings (glibc.malloc.hugetlb=0):
// opendir allocates clamp(st_blksize, 32768, 1048576) + sizeof(DIR=48).
// ptmalloc's ordinary chunk overhead is <=32; mmap overhead is <=2*sizeof(size_t)
// plus a page remainder. 16+4096 conservatively covers both. This is the DIR
// object's chunk/mapping envelope, not shared arenas, tcache or process RSS.
// Reserve the same allowance without restricting startup on other platforms;
// their native implementation, replacement allocators and huge-page allocator
// tuning remain unproved. The malloc-request ceiling needs no page assumption.
// Pinned source/binary audit: plan/evidence/m3_native_directory/README.md.
const NATIVE_DIRECTORY_BYTES: usize = 1024 * 1024 + 48 + 16 + 4096;

/// No diagnostic allocation can outlive a failed capture's reservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceCaptureError {
    Admission,
    Io,
    PathLimit,
    DirectoryLimit,
    DepthLimit,
    EntryLimit,
    SourceCountLimit,
    SourceSizeLimit,
    TotalSizeLimit,
    EmptyCast,
    Utf8,
}
impl fmt::Display for SourceCaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Admission => "actor source admission refused",
            Self::Io => "actor source IO failed",
            Self::PathLimit => "actor source path exceeds supported limit",
            Self::DirectoryLimit => "too many actor source directories",
            Self::DepthLimit => "actor source directory nesting exceeds supported limit",
            Self::EntryLimit => "too many actor source directory entries",
            Self::SourceCountLimit => "too many actor source files",
            Self::SourceSizeLimit => "actor source exceeds 4 MiB",
            Self::TotalSizeLimit => "actor sources exceed 32 MiB",
            Self::EmptyCast => "no character JSON files found",
            Self::Utf8 => "actor source is not UTF-8",
        })
    }
}
impl std::error::Error for SourceCaptureError {}
type Result<T> = std::result::Result<T, SourceCaptureError>;

struct SourceOwner {
    fixed: Vec<Box<str>>,
    characters: Vec<(String, Box<str>)>,
    // Source data and indexes are destroyed before the charge is reusable.
    lease: Reservation,
}

/// Clones share the actual buffers and their reservation. Borrowed views cannot
/// escape the owner. Parsing these inputs requires separate future admission.
#[derive(Clone)]
pub struct CapturedActorSources(Arc<SourceOwner>);

/// Conservative supported-maximum Rust allocation inventory. It includes both
/// discovery and retained indexes, simultaneous raw/lossy paths, and two file
/// buffers beyond retained payload (reader plus possible copy/shrink overlap).
/// Reviewed platform: x86_64 Linux/GNU. Rust std's Unix ReadDir retains one
/// Arc<InnerReadDir> (DIR pointer plus PathBuf), clones the root, and owns a
/// CString for each DirEntry; file_name clones that CString. Linux dirent64's
/// u16 d_reclen bounds each name by 64 KiB even for filesystems whose names
/// exceed 255 bytes. Do not assume the declared d_name[256] is its bound.
/// See the handoff's pinned rust-src inventory. The native DIR object allowance
/// is scoped above; other platforms' filesystem ownership remains an open gate.
fn peak_bytes() -> usize {
    size_of::<SourceOwner>()
        + 64
        + FIXED.len() * size_of::<Box<str>>()
        + MAX_CHARACTERS * size_of::<PathBuf>()
        + MAX_CHARACTERS * size_of::<(String, Box<str>)>()
        + MAX_DIRECTORIES * size_of::<(PathBuf, usize)>()
        + MAX_CHARACTERS * (MAX_RELATIVE_PATH_BYTES + 32)
        + MAX_CHARACTERS * (3 * MAX_RELATIVE_PATH_BYTES + 32)
        + MAX_DIRECTORIES * (MAX_RELATIVE_PATH_BYTES + 32)
        + MAX_TOTAL_SOURCE_BYTES
        + MAX_SOURCES * 32
        + 2 * (MAX_SOURCE_BYTES + 1)
        + path_scratch_bytes()
        + NATIVE_DIRECTORY_BYTES
}

// Four overlapping path buffers: captured character root, joined directory,
// std ReadDir's root clone, and syscall CString including its terminating NUL.
// Source-file opening needs fewer simultaneous paths than directory traversal.
// One ignored entry path can coexist with full retained discovery tables.
// Lossy UTF-8 conversion may grow geometrically to twice its 3*R output bound;
// the exact retained copy is separately counted in peak_bytes.
fn path_scratch_bytes() -> usize {
    4 * (MAX_ABSOLUTE_PATH_BYTES + 1 + 32)
        + 2 * (u16::MAX as usize + 1 + 32) // DirEntry name and file_name copy
        + MAX_RELATIVE_PATH_BYTES + 32 // transient ignored-entry path
        + 6 * MAX_RELATIVE_PATH_BYTES + 8 + 32 // lossy conversion temporary
        + 64 + size_of::<usize>() + size_of::<PathBuf>() // ReadDir Arc owner
}

impl CapturedActorSources {
    /// Admission precedes path creation, directory discovery and source IO.
    /// Reads freeze one observed set of bytes; this is not a filesystem-wide
    /// atomic snapshot. Edits after capture never affect the returned owner.
    pub fn capture_admitted(
        budget: &CheckpointBudget,
        assets_root: &Path,
        lore_root: &Path,
    ) -> Result<Self> {
        let lease = budget
            .reserve_running_overhead(peak_bytes())
            .map_err(|_| SourceCaptureError::Admission)?;
        // Declared first so all temporary buffers drop before its lease on error.
        let mut owner = SourceOwner {
            fixed: Vec::with_capacity(FIXED.len()),
            characters: Vec::with_capacity(MAX_CHARACTERS),
            lease,
        };
        capture_into(&mut owner, assets_root, lore_root)?;
        // capture_into's reader/discovery/path scratch is already disposed.
        let retained = size_of::<SourceOwner>()
            + 64
            + owner.fixed.capacity() * size_of::<Box<str>>()
            + owner.characters.capacity() * size_of::<(String, Box<str>)>()
            + owner
                .fixed
                .iter()
                .map(|source| source.len() + 32)
                .sum::<usize>()
            + owner
                .characters
                .iter()
                .map(|(path, source)| path.capacity() + source.len() + 64)
                .sum::<usize>();
        if retained > owner.lease.bytes() {
            return Err(SourceCaptureError::Admission);
        }
        owner
            .lease
            .resize(retained)
            .map_err(|_| SourceCaptureError::Admission)?;
        Ok(Self(Arc::new(owner)))
    }
    pub fn seed(&self) -> &str {
        &self.0.fixed[0]
    }
    pub fn occupations(&self) -> &str {
        &self.0.fixed[1]
    }
    pub fn areas(&self) -> &str {
        &self.0.fixed[2]
    }
    pub fn sounds(&self) -> &str {
        &self.0.fixed[3]
    }
    pub fn turn_prompt(&self) -> &str {
        &self.0.fixed[4]
    }
    pub fn night_prompt(&self) -> &str {
        &self.0.fixed[5]
    }
    pub fn prompt_strings(&self) -> &str {
        &self.0.fixed[6]
    }
    pub fn characters(&self) -> impl ExactSizeIterator<Item = (&str, &str)> {
        self.0
            .characters
            .iter()
            .map(|(path, source)| (path.as_str(), source.as_ref()))
    }
    pub fn charged_bytes(&self) -> usize {
        self.0.lease.bytes()
    }
    pub fn world_seed(&self, knowledge: PlayerKnowledge) -> std::result::Result<WorldSeed, String> {
        super::world_seed_from_sources(
            self.seed(),
            self.occupations(),
            self.characters(),
            knowledge,
        )
    }
}

fn joined(root: &Path, relative: &Path) -> Result<PathBuf> {
    let bytes = root
        .as_os_str()
        .as_encoded_bytes()
        .len()
        .checked_add(relative.as_os_str().as_encoded_bytes().len())
        .and_then(|n| n.checked_add(1))
        .ok_or(SourceCaptureError::PathLimit)?;
    if bytes > MAX_ABSOLUTE_PATH_BYTES {
        return Err(SourceCaptureError::PathLimit);
    }
    let mut joined = PathBuf::with_capacity(bytes);
    joined.push(root);
    joined.push(relative);
    Ok(joined)
}

fn capture_into(owner: &mut SourceOwner, assets: &Path, lore: &Path) -> Result<()> {
    let root = joined(lore, Path::new("characters"))?;
    let files = discover(&root)?;
    let mut buffer = vec![0u8; MAX_SOURCE_BYTES + 1];
    let mut total = 0;
    for (index, relative) in FIXED.iter().enumerate() {
        let path = joined(if index == 1 { lore } else { assets }, Path::new(relative))?;
        owner
            .fixed
            .push(read_source(&path, &mut buffer, &mut total)?);
    }
    for relative in files {
        let path = joined(&root, &relative)?;
        let source = read_source(&path, &mut buffer, &mut total)?;
        // Match the existing loader's lossy relative-path behavior. At most
        // three UTF-8 output bytes per raw byte are retained.
        let lossy = relative.to_string_lossy();
        owner.characters.push((lossy.as_ref().to_owned(), source));
    }
    Ok(())
}

fn discover(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::with_capacity(MAX_CHARACTERS);
    let mut pending = Vec::with_capacity(MAX_DIRECTORIES);
    pending.push((PathBuf::new(), 0usize));
    let mut directories = 1;
    let mut entries_seen = 0;
    while let Some((relative, depth)) = pending.pop() {
        let directory = joined(root, &relative)?;
        // Only one native stream is alive at once; DirEntry never escapes this
        // loop. Its native allowance is already reserved in peak_bytes and is
        // kept until capture_into returns, after ReadDir's final closedir.
        for entry in fs::read_dir(&directory).map_err(|_| SourceCaptureError::Io)? {
            entries_seen += 1;
            if entries_seen > MAX_ENTRIES {
                return Err(SourceCaptureError::EntryLimit);
            }
            let entry = entry.map_err(|_| SourceCaptureError::Io)?;
            let name = entry.file_name();
            let length = relative.as_os_str().as_encoded_bytes().len()
                + name.as_encoded_bytes().len()
                + usize::from(!relative.as_os_str().is_empty());
            if length > MAX_RELATIVE_PATH_BYTES {
                return Err(SourceCaptureError::PathLimit);
            }
            let kind = entry.file_type().map_err(|_| SourceCaptureError::Io)?;
            let mut path = PathBuf::with_capacity(length);
            path.push(&relative);
            path.push(name);
            if kind.is_dir() {
                directories += 1;
                if directories > MAX_DIRECTORIES {
                    return Err(SourceCaptureError::DirectoryLimit);
                }
                if depth >= MAX_DEPTH {
                    return Err(SourceCaptureError::DepthLimit);
                }
                pending.push((path, depth + 1));
            } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                if files.len() == MAX_CHARACTERS {
                    return Err(SourceCaptureError::SourceCountLimit);
                }
                files.push(path);
            }
            // Symlinks are ignored, matching collect_json_files.
        }
    }
    if files.is_empty() {
        return Err(SourceCaptureError::EmptyCast);
    }
    // Unique filesystem paths need no stable-sort scratch allocation.
    files.sort_unstable();
    Ok(files)
}

fn read_source(path: &Path, buffer: &mut [u8], total: &mut usize) -> Result<Box<str>> {
    let mut file = fs::File::open(path).map_err(|_| SourceCaptureError::Io)?;
    let mut used = 0;
    while used < buffer.len() {
        match file.read(&mut buffer[used..]) {
            Ok(0) => break,
            Ok(n) => used += n,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(SourceCaptureError::Io),
        }
    }
    if used > MAX_SOURCE_BYTES {
        return Err(SourceCaptureError::SourceSizeLimit);
    }
    let next = total
        .checked_add(used)
        .ok_or(SourceCaptureError::TotalSizeLimit)?;
    if next > MAX_TOTAL_SOURCE_BYTES {
        return Err(SourceCaptureError::TotalSizeLimit);
    }
    let text = std::str::from_utf8(&buffer[..used]).map_err(|_| SourceCaptureError::Utf8)?;
    *total = next;
    Ok(text.into())
}

#[cfg(test)]
mod tests;
