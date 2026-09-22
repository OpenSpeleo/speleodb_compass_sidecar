//! Read-only import analysis and isolated staging for an initial Compass import.
//!
//! MAK records retain their original byte spans. We only remove excluded records
//! and rewrite paths that cannot safely be copied into the destination. DATs are
//! never serialized. Dependency analysis deliberately includes every possible
//! earlier station provider, including shots whose X flag may be overridden by
//! Compass settings. Unsupported analysis permits a complete, unchanged import.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    ops::Range,
    path::{Component, Path, PathBuf},
};

use common::{
    Error,
    compass_import::{ImportDependency, ImportPreview, ImportSection, resolve_selection},
};
use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{LocalProject, SPELEODB_COMPASS_PROJECT_FILE};

#[derive(Debug)]
struct Snapshot {
    path: PathBuf,
    canonical_path: PathBuf,
    digest: [u8; 32],
}

impl Snapshot {
    fn read(path: &Path) -> Result<(Self, Vec<u8>), Error> {
        let canonical_path = fs::canonicalize(path).map_err(|error| read_error(path, &error))?;
        if !canonical_path.is_file() {
            return Err(invalid(format!("{} is not a regular file", path.display())));
        }
        let bytes = fs::read(&canonical_path).map_err(|error| read_error(path, &error))?;
        Ok((
            Self {
                path: path.to_path_buf(),
                canonical_path,
                digest: Sha256::digest(&bytes).into(),
            },
            bytes,
        ))
    }

    fn open_unchanged(&self) -> Result<File, Error> {
        if fs::canonicalize(&self.path).ok().as_ref() != Some(&self.canonical_path) {
            return Err(self.changed());
        }
        File::open(&self.path).map_err(|_| self.changed())
    }

    fn changed(&self) -> Error {
        Error::ImportSourceChanged(format!(
            "{} changed since the import preview. Choose the MAK file again.",
            self.path.display()
        ))
    }

    fn verify(&self) -> Result<(), Error> {
        let mut source = self.open_unchanged()?;
        let digest = transfer(&mut source, &mut std::io::sink()).map_err(|_| self.changed())?;
        if digest != self.digest {
            return Err(self.changed());
        }
        Ok(())
    }

    fn copy_verified(&self, destination: &Path) -> Result<(), Error> {
        let mut source = self.open_unchanged()?;
        let mut target = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(destination)
            .map_err(|error| copy_error(&self.path, destination, &error))?;
        let digest = transfer(&mut source, &mut target)
            .map_err(|error| copy_error(&self.path, destination, &error))?;
        if digest != self.digest {
            return Err(self.changed());
        }
        Ok(())
    }
}

fn transfer(source: &mut impl Read, target: &mut impl Write) -> std::io::Result<[u8; 32]> {
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        target.write_all(&buffer[..count])?;
        digest.update(&buffer[..count]);
    }
    Ok(digest.finalize().into())
}

#[derive(Debug)]
struct SurveySource {
    snapshot: Snapshot,
    destination: String,
    encoding: &'static Encoding,
}

#[derive(Debug)]
struct Record {
    span: Range<usize>,
    path_span: Range<usize>,
    path: String,
    stations: Vec<Station>,
    source_index: usize,
}

#[derive(Debug)]
struct Station {
    name: String,
    fixed: bool,
}

#[derive(Debug)]
struct Mak {
    records: Vec<Record>,
    full_import_reason: Option<String>,
}

/// An immutable preview and its source fingerprints. No project files are changed
/// until `stage`, which writes only into the caller's private empty directory.
#[derive(Debug)]
pub struct AnalyzedImport {
    mak_snapshot: Snapshot,
    source_directory: PathBuf,
    mak_bytes: Vec<u8>,
    mak_encoding: &'static Encoding,
    mak_destination: String,
    records: Vec<Record>,
    sources: Vec<SurveySource>,
    sections: Vec<ImportSection>,
    full_import_reason: Option<String>,
}

/// Read the MAK inventory and each unique referenced DAT once. Call on a blocking
/// worker because fingerprinting and station indexing can read large projects.
pub fn analyze(path: &Path) -> Result<AnalyzedImport, Error> {
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mak"))
    {
        return Err(invalid("Choose a .MAK project file"));
    }
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| read_error(path, &error))?
            .join(path)
    };
    let path = absolute_path.as_path();
    let (mak_snapshot, mak_bytes) = Snapshot::read(path)?;
    // Relative DAT references are relative to the selected MAK location, even
    // when the MAK itself is a symlink to a file in a different directory.
    let source_directory = path
        .parent()
        .ok_or_else(|| invalid("The MAK file has no parent directory"))?
        .to_path_buf();
    let source_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("The MAK filename is not valid UTF-8"))?;
    let mak_destination = if portable_component(source_name) {
        source_name.to_string()
    } else {
        "project.mak".into()
    };
    let mak_encoding = text_encoding(&mak_bytes);
    let Mak {
        mut records,
        mut full_import_reason,
    } = scan_mak(&mak_bytes, mak_encoding)?;
    if records.is_empty() {
        return Err(invalid("The MAK file contains no survey files"));
    }

    let mut sources = Vec::<SurveySource>::new();
    let mut by_canonical_path = BTreeMap::<PathBuf, usize>::new();
    let mut inventories = Vec::<BTreeSet<String>>::new();
    let mut destinations = BTreeSet::from([
        mak_destination.to_lowercase(),
        SPELEODB_COMPASS_PROJECT_FILE.to_lowercase(),
    ]);
    for (id, record) in records.iter_mut().enumerate() {
        let source_path = resolve_source(&source_directory, &record.path)?;
        let canonical =
            fs::canonicalize(&source_path).map_err(|error| read_error(&source_path, &error))?;
        if let Some(index) = by_canonical_path.get(&canonical) {
            record.source_index = *index;
            continue;
        }
        let (snapshot, bytes) = Snapshot::read(&source_path)?;
        let relative = portable_relative(&record.path);
        let destination = match relative {
            Some(relative) if destination_available(&relative, &destinations) => relative,
            _ => remapped_destination(id, &record.path, &destinations),
        };
        destinations.insert(destination.to_lowercase());
        match index_dat(&bytes) {
            Ok(stations) => inventories.push(stations),
            Err(reason) => {
                full_import_reason.get_or_insert_with(|| format!(
                    "Connections in {} could not be verified: {reason}. Import all sections to preserve the original project.",
                    record.path
                ));
                inventories.push(BTreeSet::new());
            }
        }
        record.source_index = sources.len();
        by_canonical_path.insert(snapshot.canonical_path.clone(), sources.len());
        sources.push(SurveySource {
            snapshot,
            destination,
            encoding: text_encoding(&bytes),
        });
    }
    let (mut sections, unresolved_link) = build_sections(
        &records,
        &sources,
        &inventories,
        mak_encoding,
        full_import_reason.is_none(),
    );
    if let Some(reason) = unresolved_link {
        full_import_reason = Some(reason);
        for section in &mut sections {
            section.dependencies.clear();
        }
    }
    Ok(AnalyzedImport {
        mak_snapshot,
        source_directory,
        mak_bytes,
        mak_encoding,
        mak_destination,
        records,
        sources,
        sections,
        full_import_reason,
    })
}

impl AnalyzedImport {
    pub fn preview(&self, preview_id: Uuid) -> ImportPreview {
        ImportPreview {
            preview_id,
            source_name: self
                .mak_snapshot
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            source_directory: self.source_directory.display().to_string(),
            sections: self.sections.clone(),
            full_import_reason: self.full_import_reason.clone(),
        }
    }

    /// Validate selection and source fingerprints, then write a complete staged
    /// project. The caller owns staging cleanup and atomic publication.
    pub fn stage(
        &self,
        project_id: Uuid,
        selected_section_ids: &[usize],
        staging_dir: &Path,
    ) -> Result<(), Error> {
        let explicit = selected_section_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if explicit.is_empty() {
            return Err(invalid("Select at least one section to import"));
        }
        let included = resolve_selection(&self.sections, &explicit)?.included;
        if self.full_import_reason.is_some() && included.len() != self.records.len() {
            return Err(invalid(
                "This project must be imported with all sections selected",
            ));
        }
        let metadata =
            fs::symlink_metadata(staging_dir).map_err(|error| read_error(staging_dir, &error))?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || fs::read_dir(staging_dir)
                .map_err(|error| read_error(staging_dir, &error))?
                .next()
                .is_some()
        {
            return Err(invalid(
                "The import staging directory must be an empty regular directory",
            ));
        }
        // Validate even excluded files: their contents informed the dependency graph.
        self.verify_sources()?;
        let mut selected_sources = BTreeSet::new();
        let mut dat_files = Vec::new();
        for id in &included {
            let index = self.records[*id].source_index;
            if selected_sources.insert(index) {
                let source = &self.sources[index];
                let destination = staging_dir.join(&source.destination);
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|error| copy_error(&source.snapshot.path, &destination, &error))?;
                }
                source.snapshot.copy_verified(&destination)?;
                dat_files.push(source.destination.clone());
            }
        }
        let generated = self.filtered_mak(&included)?;
        self.validate_generated(&generated, &included)?;
        let mak_path = staging_dir.join(&self.mak_destination);
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&mak_path)
            .and_then(|mut file| file.write_all(&generated))
            .map_err(|error| copy_error(&self.mak_snapshot.path, &mak_path, &error))?;
        LocalProject::write_import_metadata(
            project_id,
            self.mak_destination.clone(),
            dat_files,
            staging_dir,
        )?;
        // Catch changes during staging as well as changes since preview.
        self.verify_sources()
    }

    fn verify_sources(&self) -> Result<(), Error> {
        self.mak_snapshot.verify()?;
        for source in &self.sources {
            source.snapshot.verify()?;
        }
        // Re-resolve every original reference, not just the previously resolved
        // filename: a new exact-case file can supersede an earlier fallback.
        for record in &self.records {
            let canonical = resolve_source(&self.source_directory, &record.path)
                .ok()
                .and_then(|path| fs::canonicalize(path).ok());
            if canonical.as_ref()
                != Some(&self.sources[record.source_index].snapshot.canonical_path)
            {
                return Err(Error::ImportSourceChanged(format!(
                    "The reference {} changed since the import preview. Choose the MAK file again.",
                    record.path
                )));
            }
        }
        Ok(())
    }

    fn validate_generated(
        &self,
        generated: &[u8],
        included: &BTreeSet<usize>,
    ) -> Result<(), Error> {
        let parsed = scan_mak(generated, self.mak_encoding)?;
        let expected = included
            .iter()
            .map(|id| &self.sources[self.records[*id].source_index].destination)
            .collect::<Vec<_>>();
        let actual = parsed
            .records
            .iter()
            .map(|record| portable_relative(&record.path))
            .collect::<Vec<_>>();
        if actual.len() != expected.len()
            || actual
                .iter()
                .zip(expected)
                .any(|(actual, expected)| actual.as_ref() != Some(expected))
        {
            return Err(invalid(
                "Generated MAK does not match the selected file inventory",
            ));
        }
        Ok(())
    }

    fn filtered_mak(&self, included: &BTreeSet<usize>) -> Result<Vec<u8>, Error> {
        let mut result = Vec::with_capacity(self.mak_bytes.len());
        let mut copied = 0;
        for (id, record) in self.records.iter().enumerate() {
            result.extend_from_slice(&self.mak_bytes[copied..record.span.start]);
            if included.contains(&id) {
                let destination = &self.sources[record.source_index].destination;
                if portable_relative(&record.path).as_ref() == Some(destination) {
                    result.extend_from_slice(&self.mak_bytes[record.span.clone()]);
                } else {
                    result.extend_from_slice(
                        &self.mak_bytes[record.span.start..record.path_span.start],
                    );
                    // Compass uses '/' for comments; generated file references
                    // use Windows separators while metadata uses portable '/'.
                    let path = destination.replace('/', "\\");
                    let (encoded, _, errors) = self.mak_encoding.encode(&path);
                    if errors {
                        return Err(invalid(
                            "A destination filename cannot be represented in the MAK encoding",
                        ));
                    }
                    result.extend_from_slice(&encoded);
                    result
                        .extend_from_slice(&self.mak_bytes[record.path_span.end..record.span.end]);
                }
            }
            copied = record.span.end;
        }
        result.extend_from_slice(&self.mak_bytes[copied..]);
        Ok(result)
    }
}

fn invalid(message: impl Into<String>) -> Error {
    Error::CompassProject(message.into())
}

fn read_error(path: &Path, error: &std::io::Error) -> Error {
    Error::FileRead(format!("{}: {error}", path.display()))
}

fn copy_error(source: &Path, destination: &Path, error: &std::io::Error) -> Error {
    Error::ProjectImport {
        src_path: source.to_path_buf(),
        dst_path: destination.to_path_buf(),
        details: error.to_string(),
        is_permission_error: error.kind() == std::io::ErrorKind::PermissionDenied,
    }
}

fn resolve_source(directory: &Path, raw: &str) -> Result<PathBuf, Error> {
    let normalized = raw.replace('\\', "/");
    if !cfg!(windows)
        && (normalized.starts_with("//") || normalized.as_bytes().get(1) == Some(&b':'))
    {
        return Err(invalid(format!(
            "The Windows path {raw} cannot be resolved on this computer"
        )));
    }
    let requested = directory.join(normalized);
    if requested.exists() {
        return Ok(requested);
    }
    // Windows-authored MAK files frequently differ from the actual DAT casing.
    // Resolve a unique case-insensitive match on case-sensitive filesystems,
    // while refusing ambiguity rather than selecting an arbitrary source.
    let mut resolved = PathBuf::new();
    for component in requested.components() {
        let Component::Normal(name) = component else {
            resolved.push(component.as_os_str());
            continue;
        };
        let exact = resolved.join(name);
        if exact.exists() {
            resolved = exact;
            continue;
        }
        let requested_name = name.to_string_lossy().to_lowercase();
        let mut matches = fs::read_dir(&resolved)
            .map_err(|error| read_error(&requested, &error))?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().to_lowercase() == requested_name);
        let found = matches
            .next()
            .ok_or_else(|| Error::ProjectFileNotFound(requested.clone()))?;
        if matches.next().is_some() {
            return Err(invalid(format!(
                "The capitalization of {raw} matches more than one source file"
            )));
        }
        resolved = found.path();
    }
    Ok(resolved)
}

fn portable_component(component: &str) -> bool {
    if component.is_empty()
        || matches!(component, "." | "..")
        || component.ends_with(['.', ' '])
        || component
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return false;
    }
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !(stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        && !["COM¹", "COM²", "COM³", "LPT¹", "LPT²", "LPT³"].contains(&stem.as_str())
}

fn portable_relative(raw: &str) -> Option<String> {
    let normalized = raw.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        if part == "." {
            continue;
        }
        if !portable_component(part) {
            return None;
        }
        parts.push(part);
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn destination_available(path: &str, occupied: &BTreeSet<String>) -> bool {
    let path = path.to_lowercase();
    occupied.iter().all(|existing| {
        existing != &path
            && !existing.starts_with(&format!("{path}/"))
            && !path.starts_with(&format!("{existing}/"))
    })
}

fn remapped_destination(id: usize, raw: &str, occupied: &BTreeSet<String>) -> String {
    let normalized = raw.replace('\\', "/");
    let basename = normalized
        .rsplit('/')
        .next()
        .filter(|name| portable_component(name))
        .unwrap_or("survey.dat");
    let mut suffix = 0;
    loop {
        let folder = if suffix == 0 {
            "_imports".to_string()
        } else {
            format!("_imports_{suffix}")
        };
        let candidate = format!("{folder}/{id}/{basename}");
        if destination_available(&candidate, occupied) {
            return candidate;
        }
        suffix += 1;
    }
}

/// Skip comments only where '/' cannot be part of a path. Comment terminators
/// inside comments must never be interpreted as MAK commands or record ends.
fn skip_layout(bytes: &[u8], cursor: &mut usize) {
    loop {
        while bytes
            .get(*cursor)
            .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == 0x1a)
        {
            *cursor += 1;
        }
        if bytes.get(*cursor) != Some(&b'/') {
            return;
        }
        *cursor += 1;
        while bytes
            .get(*cursor)
            .is_some_and(|byte| !matches!(byte, b'/' | b'\r' | b'\n'))
        {
            *cursor += 1;
        }
        if bytes.get(*cursor) == Some(&b'/') {
            *cursor += 1;
        }
    }
}

fn record_end(bytes: &[u8], mut cursor: usize, comments: bool) -> Result<usize, Error> {
    let mut brackets = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b';' if brackets == 0 => return Ok(cursor + 1),
            b'[' if comments => {
                brackets += 1;
                cursor += 1;
            }
            b']' if comments => {
                if brackets == 0 {
                    return Err(invalid("Unbalanced fixed-station brackets"));
                }
                brackets -= 1;
                cursor += 1;
            }
            b'/' if comments => {
                skip_layout(bytes, &mut cursor);
            }
            _ => cursor += 1,
        }
    }
    Err(invalid("An incomplete MAK record is missing its semicolon"))
}

/// Prefer Unicode exports, falling back to the legacy Windows Western encoding.
/// Decide once per file: individual byte ranges can look like UTF-8 by chance.
fn text_encoding(bytes: &[u8]) -> &'static Encoding {
    if std::str::from_utf8(bytes).is_ok() {
        UTF_8
    } else {
        WINDOWS_1252
    }
}

fn scan_mak(bytes: &[u8], encoding: &'static Encoding) -> Result<Mak, Error> {
    if bytes.contains(&0) || bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        return Err(invalid(
            "The MAK must be UTF-8 or Windows-1252 text, not UTF-16 or binary data",
        ));
    }
    let mut cursor = if encoding == UTF_8 && bytes.starts_with(b"\xef\xbb\xbf") {
        3
    } else {
        0
    };
    let mut folders = 0_usize;
    let mut mak = Mak {
        records: Vec::new(),
        full_import_reason: None,
    };
    while cursor < bytes.len() {
        while bytes
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == 0x1a)
        {
            cursor += 1;
        }
        if cursor == bytes.len() {
            break;
        }
        // A UUID record is the sole exception to slash-delimited comments.
        if bytes[cursor] == b'/' {
            if let Some(end) = bytes[cursor + 1..].iter().position(|byte| *byte == b';') {
                let candidate = &bytes[cursor + 1..cursor + 1 + end];
                if std::str::from_utf8(candidate).is_ok_and(|value| Uuid::parse_str(value).is_ok())
                {
                    cursor += end + 2;
                    continue;
                }
            }
            cursor += 1;
            while bytes
                .get(cursor)
                .is_some_and(|byte| !matches!(byte, b'/' | b'\r' | b'\n'))
            {
                cursor += 1;
            }
            if bytes.get(cursor) == Some(&b'/') {
                cursor += 1;
            }
            continue;
        }
        let start = cursor;
        let command = bytes[cursor];
        cursor += 1;
        match command {
            b'#' => {
                let raw_start = cursor;
                while bytes
                    .get(cursor)
                    .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
                {
                    cursor += 1;
                }
                let first_path_byte = cursor;
                while let Some(byte) = bytes.get(cursor) {
                    if matches!(byte, b',' | b';' | b'\r' | b'\n')
                        || (*byte == b'/'
                            && cursor > first_path_byte
                            && bytes[cursor - 1].is_ascii_whitespace())
                    {
                        break;
                    }
                    cursor += 1;
                }
                // Scan and trim the original bytes so spans stay valid even when
                // a single Windows-1252 byte expands to several UTF-8 bytes.
                let raw = &bytes[raw_start..cursor];
                let trimmed = raw.trim_ascii();
                let (path, _) = encoding.decode_without_bom_handling(trimmed);
                if path.is_empty() || path.contains(['\r', '\n', '\0', '#']) {
                    return Err(invalid("A MAK survey record contains an invalid filename"));
                }
                let leading = raw.len() - raw.trim_ascii_start().len();
                let path_span = raw_start + leading..raw_start + leading + trimmed.len();
                skip_layout(bytes, &mut cursor);
                if !bytes
                    .get(cursor)
                    .is_some_and(|byte| matches!(byte, b',' | b';'))
                {
                    return Err(invalid(
                        "A MAK filename is not followed by a station list or semicolon",
                    ));
                }
                let end = record_end(bytes, cursor, true)?;
                let (links, _) = encoding.decode_without_bom_handling(&bytes[cursor..end - 1]);
                let stations = match parse_stations(&links) {
                    Ok(stations) => stations,
                    Err(reason) => {
                        mak.full_import_reason.get_or_insert(format!(
                            "The MAK uses unsupported linking information ({reason}). Import all sections to preserve it."
                        ));
                        Vec::new()
                    }
                };
                mak.records.push(Record {
                    span: start..end,
                    path_span,
                    path: path.into_owned(),
                    stations,
                    source_index: 0,
                });
                cursor = end;
            }
            b'[' => {
                let end = record_end(bytes, cursor, false)?;
                let (name, _) = encoding.decode_without_bom_handling(&bytes[cursor..end - 1]);
                let name = name.trim();
                if name.is_empty() || name.contains(['[', ']', '#', '\r', '\n']) {
                    return Err(invalid(
                        "Unsupported MAK folder syntax; the file inventory cannot be verified",
                    ));
                }
                folders += 1;
                mak.full_import_reason.get_or_insert_with(||
                    "This MAK contains Compass folders. Import all sections to preserve their grouping and connections.".into());
                cursor = end;
            }
            b']' => {
                skip_layout(bytes, &mut cursor);
                if folders == 0 || bytes.get(cursor) != Some(&b';') {
                    return Err(invalid("Unbalanced MAK folder delimiters"));
                }
                folders -= 1;
                cursor += 1;
            }
            b'@' | b'&' | b'$' | b'%' | b'*' | b'!' => {
                let end = record_end(bytes, cursor, true)?;
                let (value, _) = encoding.decode_without_bom_handling(&bytes[cursor..end - 1]);
                if !known_directive(command, &value) {
                    mak.full_import_reason.get_or_insert_with(||
                        "Some MAK settings are not supported for connection analysis. Import all sections to preserve them.".into());
                }
                cursor = end;
            }
            _ => {
                return Err(invalid(format!(
                    "Unsupported MAK command at byte {start}; the complete file inventory cannot be verified"
                )));
            }
        }
    }
    if folders != 0 {
        return Err(invalid("Unbalanced MAK folder delimiters"));
    }
    Ok(mak)
}

fn known_directive(command: u8, value: &str) -> bool {
    let uncommented = strip_comments(value);
    let value = uncommented.trim();
    match command {
        b'@' => {
            let fields: Vec<_> = value.split(',').collect();
            fields.len() == 5 && fields.iter().all(|field| finite_number(field.trim()))
        }
        b'&' => !value.is_empty() && !value.contains(['\r', '\n', '#']),
        b'$' => value.parse::<i32>().is_ok(),
        b'%' | b'*' => finite_number(value),
        b'!' => value
            .bytes()
            .all(|byte| b"GgIEAVvOoTtSsXxPpLlCc".contains(&byte)),
        _ => false,
    }
}

fn parse_stations(text: &str) -> Result<Vec<Station>, String> {
    let bytes = text.as_bytes();
    let mut cursor = 0;
    let mut stations = Vec::<Station>::new();
    loop {
        skip_layout(bytes, &mut cursor);
        if cursor == bytes.len() {
            return Ok(stations);
        }
        if bytes[cursor] != b',' {
            return Err("unrecognized station separator".into());
        }
        cursor += 1;
        skip_layout(bytes, &mut cursor);
        let start = cursor;
        while bytes
            .get(cursor)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && !b",[]/".contains(byte))
        {
            cursor += 1;
        }
        let name = &text[start..cursor];
        if !station_name(name) {
            return Err("unrecognized station name".into());
        }
        skip_layout(bytes, &mut cursor);
        let fixed = bytes.get(cursor) == Some(&b'[');
        if fixed {
            let mut end = cursor + 1;
            while let Some(byte) = bytes.get(end) {
                if *byte == b']' {
                    break;
                }
                if *byte == b'/' {
                    skip_layout(bytes, &mut end);
                } else {
                    end += 1;
                }
            }
            if end == bytes.len() {
                return Err("unclosed fixed station".into());
            }
            let coordinates = strip_comments(&text[cursor + 1..end]);
            let fields: Vec<_> = coordinates
                .split(|c: char| c == ',' || c.is_ascii_whitespace())
                .filter(|field| !field.is_empty())
                .collect();
            if fields.len() != 4
                || !matches!(fields[0], "F" | "f" | "M" | "m")
                || !fields[1..].iter().all(|field| finite_number(field))
            {
                return Err("unrecognized fixed coordinates".into());
            }
            cursor = end + 1;
        }
        if stations
            .iter()
            .any(|station| station.name == name && (station.fixed || fixed))
        {
            return Err("ambiguous repeated fixed station".into());
        }
        stations.push(Station {
            name: name.into(),
            fixed,
        });
    }
}

fn strip_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] == b'/' {
            skip_layout(bytes, &mut cursor);
            result.push(b' ');
        } else {
            result.push(bytes[cursor]);
            cursor += 1;
        }
    }
    // Removing ASCII comment ranges from an already UTF-8 string preserves UTF-8.
    String::from_utf8(result).expect("comment removal preserves UTF-8")
}

fn station_name(name: &str) -> bool {
    // This is a structural index, not an editor field validator. Preserve full
    // case-sensitive names, including long and accented identifiers in exports.
    !name.is_empty()
        && name.chars().all(|c| !c.is_whitespace() && !c.is_control())
        && !name.starts_with(')')
}

fn finite_number(text: &str) -> bool {
    text.parse::<f64>().is_ok_and(f64::is_finite)
}

/// A structural index, not a survey serializer or geometry compiler. Every
/// nonempty survey/shot must be recognized before filtering is offered.
fn index_dat(bytes: &[u8]) -> Result<BTreeSet<String>, String> {
    let (text, _) = text_encoding(bytes).decode_without_bom_handling(bytes);
    let text = text
        .trim_start_matches('\u{feff}')
        .trim_end_matches(|c: char| c.is_ascii_whitespace() || c == '\x1a');
    let mut stations = BTreeSet::new();
    let mut count = 0;
    for block in text.split('\x0c') {
        count += index_dat_block(block, &mut stations)?;
    }
    if count == 0 {
        return Err("no complete surveys were found".into());
    }
    Ok(stations)
}

fn looks_like_shot(line: &str) -> bool {
    let fields = line.split_ascii_whitespace().take(9).collect::<Vec<_>>();
    fields.len() == 9 && fields[2..].iter().all(|field| finite_number(field))
}

fn index_dat_block(text: &str, stations: &mut BTreeSet<String>) -> Result<usize, String> {
    // Some existing Compass exports omit form feeds. Complete repeated headers
    // also give unambiguous boundaries; every intervening shot still gets read.
    let lines: Vec<_> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let mut count = 0;
    let mut cursor = 0;
    while cursor < lines.len() {
        count += 1;
        let header = lines.get(cursor..cursor + 3);
        let Some(header) = header else {
            return Err("an incomplete survey header was found".into());
        };
        if looks_like_shot(header[0]) {
            return Err("a survey heading is ambiguous with a shot".into());
        }
        let name = header[1]
            .strip_prefix("SURVEY NAME:")
            .map(str::trim)
            .ok_or("an unrecognized survey header was found")?;
        // Survey labels do not define station identity. Real exports contain
        // spaces and accented text here; dependencies use the FROM/TO columns.
        if name.is_empty() || name.chars().any(|c| c.is_control() && c != '\t') {
            return Err("a survey name is empty or contains control characters".into());
        }
        let date = header[2]
            .strip_prefix("SURVEY DATE:")
            .ok_or("a survey date header is missing")?;
        let date = date.split_once("COMMENT:").map_or(date, |(date, _)| date);
        let date: Vec<_> = date.split_ascii_whitespace().collect();
        if date.len() != 3 || !date.iter().all(|part| part.parse::<u32>().is_ok()) {
            return Err("a survey header contains unsupported fields".into());
        }
        let mut team_index = cursor + 3;
        if lines
            .get(team_index)
            .is_some_and(|line| line.starts_with("COMMENT:"))
        {
            team_index += 1;
        }
        if lines.get(team_index) != Some(&"SURVEY TEAM:") {
            return Err("a survey header contains unsupported fields".into());
        }
        // Blank team values disappear with layout-only lines. The declination
        // therefore follows either immediately or after one team-text line.
        let parameters_index = if lines
            .get(team_index + 1)
            .is_some_and(|line| line.starts_with("DECLINATION:"))
        {
            team_index + 1
        } else {
            team_index + 2
        };
        let parameters = lines
            .get(parameters_index)
            .ok_or("a survey header is incomplete")?;
        let backsights = parse_dat_parameters(parameters)?;
        if lines
            .get(parameters_index + 1)
            .ok_or("a survey shot table is missing")?
            .split_ascii_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            != ["FROM", "TO"]
        {
            return Err("a survey shot table is missing".into());
        }
        cursor = parameters_index + 2;
        while cursor < lines.len() {
            if lines
                .get(cursor + 1)
                .is_some_and(|line| line.starts_with("SURVEY NAME:"))
                && !looks_like_shot(lines[cursor])
            {
                break;
            }
            let line = lines[cursor];
            let fields: Vec<_> = line.split_ascii_whitespace().collect();
            let numeric_count = if backsights { 9 } else { 7 };
            if fields.len() < numeric_count + 2
                || !station_name(fields[0])
                || !station_name(fields[1])
                || !fields[2..numeric_count + 2]
                    .iter()
                    .all(|field| finite_number(field))
            {
                return Err("a shot uses unsupported or incomplete syntax".into());
            }
            if fields
                .get(numeric_count + 2)
                .is_some_and(|field| field.starts_with("#|"))
            {
                // Flags are delimited, not whitespace-separated: exports also
                // use an empty `#| #` marker and comments adjacent to the '#'.
                let suffix = fields[numeric_count + 2..].join(" ");
                let (flags, _) = suffix[2..]
                    .split_once('#')
                    .ok_or("a shot flag is incomplete")?;
                if !flags
                    .bytes()
                    .all(|flag| flag.is_ascii_whitespace() || b"LPCX".contains(&flag))
                {
                    return Err("a shot uses unsupported flags".into());
                }
            }
            stations.insert(fields[0].to_string());
            stations.insert(fields[1].to_string());
            cursor += 1;
        }
    }
    Ok(count)
}

fn parse_dat_parameters(line: &str) -> Result<bool, String> {
    let parameters = line
        .strip_prefix("DECLINATION:")
        .ok_or("a declination header is missing")?;
    let fields: Vec<_> = parameters.split_ascii_whitespace().collect();
    if fields.first().is_none_or(|field| !finite_number(field)) {
        return Err("the declination header is incomplete".into());
    }
    let mut cursor = 1;
    let mut backsights = false;
    let mut seen = BTreeSet::new();
    while cursor < fields.len() {
        let key = fields[cursor];
        if !seen.insert(key) {
            return Err("duplicate survey parameters were found".into());
        }
        cursor += 1;
        match key {
            "FORMAT:" => {
                let format = fields.get(cursor).ok_or("a survey format is missing")?;
                let bytes = format.as_bytes();
                if !matches!(bytes.len(), 11 | 12 | 13 | 15)
                    || !bytes.iter().all(u8::is_ascii_alphabetic)
                {
                    return Err("a survey format is unsupported".into());
                }
                let backsight_index = match bytes.len() {
                    12 | 13 => Some(11),
                    15 => Some(13),
                    _ => None,
                };
                if let Some(index) = backsight_index {
                    if !matches!(bytes[index], b'B' | b'N') {
                        return Err("a backsight format is unsupported".into());
                    }
                    backsights = bytes[index] == b'B';
                }
                cursor += 1;
            }
            "CORRECTIONS:" | "CORRECTIONS2:" => {
                let count = if key == "CORRECTIONS:" { 3 } else { 2 };
                let values = fields
                    .get(cursor..cursor + count)
                    .ok_or("a correction parameter is incomplete")?;
                if !values.iter().all(|field| finite_number(field)) {
                    return Err("a correction parameter is unsupported".into());
                }
                cursor += count;
            }
            _ => return Err(format!("the survey parameter {key} is unsupported")),
        }
    }
    Ok(backsights)
}

/// Keep both identities: Unicode exports may share names across encodings,
/// while an otherwise-ASCII legacy file can accidentally also be valid UTF-8.
/// Matching the original bytes prevents unrelated comments from hiding a link.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum StationKey {
    Decoded(String),
    OriginalBytes(Vec<u8>),
}

fn station_keys(name: &str, encoding: &'static Encoding) -> [StationKey; 2] {
    let (bytes, _, _) = encoding.encode(name);
    [
        StationKey::Decoded(name.to_string()),
        StationKey::OriginalBytes(bytes.into_owned()),
    ]
}

fn build_sections(
    records: &[Record],
    sources: &[SurveySource],
    inventories: &[BTreeSet<String>],
    mak_encoding: &'static Encoding,
    analyze_connections: bool,
) -> (Vec<ImportSection>, Option<String>) {
    let mut active = BTreeMap::<StationKey, BTreeSet<usize>>::new();
    let mut last_reset = None;
    let mut unresolved_link = None;
    let sections = records.iter().enumerate().map(|(id, record)| {
        let mut reasons = BTreeMap::<usize, BTreeSet<String>>::new();
        if analyze_connections {
            if record.stations.is_empty() {
                if let Some(reset) = last_reset {
                    reasons.entry(reset).or_default().insert("Preserves station context".into());
                }
            } else {
                let mut retained = BTreeMap::new();
                for station in &record.stations {
                    let keys = station_keys(&station.name, mak_encoding);
                    let providers = if station.fixed { BTreeSet::from([id]) }
                        else { keys.iter().filter_map(|key| active.get(key)).flatten().copied().collect() };
                    if providers.is_empty() {
                        unresolved_link.get_or_insert_with(|| format!(
                            "The link {} in {} has no verified earlier provider. Import all sections to preserve the original project.",
                            station.name, record.path
                        ));
                    }
                    for provider in &providers {
                        if *provider != id { reasons.entry(*provider).or_default().insert(station.name.clone()); }
                    }
                    for key in keys {
                        retained.entry(key).or_insert_with(BTreeSet::new).extend(&providers);
                    }
                }
                active = retained;
                last_reset = Some(id);
            }
            for station in &inventories[record.source_index] {
                for key in station_keys(station, sources[record.source_index].encoding) {
                    let providers = active.entry(key).or_default();
                    for provider in providers.iter().filter(|provider| **provider != id) {
                        reasons.entry(*provider).or_default().insert(station.clone());
                    }
                    providers.insert(id);
                }
            }
        }
        let destination = &sources[record.source_index].destination;
        ImportSection {
            id,
            name: record.path.replace('\\', "/").rsplit('/').next().unwrap_or(&record.path).into(),
            relative_path: destination.clone(),
            dependencies: reasons.into_iter().map(|(section_id, reasons)| {
                let reason = if reasons.contains("Preserves station context") {
                    "Preserves station context".into()
                } else {
                    let names = reasons.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
                    if reasons.len() > 3 { format!("Connected through {names} and {} more stations", reasons.len() - 3) }
                    else { format!("Connected through {names}") }
                };
                ImportDependency { section_id, reason }
            }).collect(),
        }
    }).collect();
    (sections, unresolved_link)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("compass-import-{}", Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            Self(root)
        }

        fn write(&self, path: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, bytes).unwrap();
            path
        }

        fn staging(&self) -> PathBuf {
            let path = self.0.join(format!("stage-{}", Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            path
        }

        fn dat(&self, path: &str, from: &str, to: &str) {
            self.write(path, survey(&format!("{from} {to} 1 2 3 4 5 6 7")));
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn survey(shots: &str) -> String {
        format!(
            "Cave\nSURVEY NAME: A\nSURVEY DATE: 1 2 2020 COMMENT: Header\nSURVEY TEAM:\nA surveyor\nDECLINATION: 0 FORMAT: DDDDLUDRADLNF\n\nFROM TO LENGTH BEARING INC LEFT UP DOWN RIGHT\n\n{shots}\n\x0c\n"
        )
    }

    fn dependencies(analysis: &AnalyzedImport, id: usize) -> BTreeSet<usize> {
        analysis.sections[id]
            .dependencies
            .iter()
            .map(|edge| edge.section_id)
            .collect()
    }

    #[test]
    fn identical_legacy_station_bytes_cannot_lose_a_dependency_when_comments_change_encoding() {
        let fixture = Fixture::new();
        // Both DATs originate in Windows-1252. The first happens to be valid
        // UTF-8; a legacy comment in the second changes the inferred encoding.
        let provider = survey("A Ã© 1 2 3 4 5 6 7");
        let dependent = survey("Ã© B 1 2 3 4 5 6 7").replace("Header", "Café");
        let provider_bytes = WINDOWS_1252.encode(&provider).0.into_owned();
        let dependent_bytes = WINDOWS_1252.encode(&dependent).0.into_owned();
        assert_eq!(text_encoding(&provider_bytes), UTF_8);
        assert_eq!(text_encoding(&dependent_bytes), WINDOWS_1252);
        fixture.write("a.dat", &provider_bytes);
        fixture.write("b.dat", &dependent_bytes);
        for mak in ["#a.dat;#b.dat;", "/ Café\n#a.dat;#b.dat,Ã©;"] {
            let mak_bytes = WINDOWS_1252.encode(mak).0.into_owned();
            let analysis = analyze(&fixture.write("Cave.mak", &mak_bytes)).unwrap();
            assert!(analysis.full_import_reason.is_none());
            assert_eq!(dependencies(&analysis, 1), BTreeSet::from([0]));
            let stage = fixture.staging();
            analysis.stage(Uuid::new_v4(), &[1], &stage).unwrap();
            assert_eq!(fs::read(stage.join("a.dat")).unwrap(), provider_bytes);
            assert_eq!(fs::read(stage.join("b.dat")).unwrap(), dependent_bytes);
            assert_eq!(fs::read(stage.join("Cave.mak")).unwrap(), mak_bytes);
        }
    }

    #[test]
    fn legacy_windows_text_preserves_names_offsets_and_source_bytes() {
        let fixture = Fixture::new();
        let mak = b"/ Caf\xe9 \x96 original\r\n#Backshall\xb4s Backdoor.DAT;\r\n#Peque\xf1a.DAT;\r\n#last.dat;\r\n";
        let dat = survey("Ma´ax1 B 1 2 3 4 5 6 7").replace("A surveyor", "André — surveyor");
        let (dat_bytes, _, errors) = WINDOWS_1252.encode(&dat);
        assert!(!errors);
        let dat_path = fixture.write("Backshall´s Backdoor.DAT", &dat_bytes);
        fixture.dat("Pequeña.DAT", "C", "D");
        fixture.dat("last.dat", "E", "F");
        let path = fixture.write("Cave.mak", mak);
        let analysis = analyze(&path).unwrap();
        assert!(analysis.full_import_reason.is_none());
        assert_eq!(analysis.sections[0].name, "Backshall´s Backdoor.DAT");
        assert_eq!(analysis.sections[1].name, "Pequeña.DAT");
        let complete = fixture.staging();
        analysis
            .stage(Uuid::new_v4(), &[0, 1, 2], &complete)
            .unwrap();
        assert_eq!(fs::read(complete.join("Cave.mak")).unwrap(), mak);
        let subset = fixture.staging();
        analysis.stage(Uuid::new_v4(), &[0, 2], &subset).unwrap();
        assert_eq!(
            fs::read(subset.join("Cave.mak")).unwrap(),
            b"/ Caf\xe9 \x96 original\r\n#Backshall\xb4s Backdoor.DAT;\r\n\r\n#last.dat;\r\n"
        );
        assert!(!subset.join("Pequeña.DAT").exists());
        assert_eq!(
            fs::read(subset.join("Backshall´s Backdoor.DAT")).unwrap(),
            dat_bytes.as_ref()
        );
        assert_eq!(fs::read(path).unwrap(), mak);
        assert_eq!(fs::read(dat_path).unwrap(), dat_bytes.as_ref());
        assert!(
            analyze(&subset.join("Cave.mak"))
                .unwrap()
                .full_import_reason
                .is_none()
        );
    }

    #[test]
    fn mixed_file_encodings_preserve_dependencies_and_staged_bytes() {
        for mak_encoding in [UTF_8, WINDOWS_1252] {
            for (provider_encoding, dependent_encoding) in
                [(UTF_8, WINDOWS_1252), (WINDOWS_1252, UTF_8)]
            {
                let fixture = Fixture::new();
                let provider = survey("A Ma´ax1 1 2 3 4 5 6 7");
                let dependent = survey("Ma´ax1 B 1 2 3 4 5 6 7");
                let (provider_bytes, _, errors) = provider_encoding.encode(&provider);
                assert!(!errors);
                let (dependent_bytes, _, errors) = dependent_encoding.encode(&dependent);
                assert!(!errors);
                let provider_path = fixture.write("Café.dat", &provider_bytes);
                let dependent_path = fixture.write("next.dat", &dependent_bytes);
                fixture.dat("unrelated.dat", "C", "D");
                let mak = "#Café.dat;\r\n#unrelated.dat;\r\n#next.dat,Ma´ax1;\r\n";
                let (mak_bytes, _, errors) = mak_encoding.encode(mak);
                assert!(!errors);
                let path = fixture.write("Cave.mak", &mak_bytes);
                let analysis = analyze(&path).unwrap();
                assert!(
                    analysis.full_import_reason.is_none(),
                    "{:?}",
                    analysis.full_import_reason
                );
                assert_eq!(dependencies(&analysis, 2), BTreeSet::from([0]));

                let stage = fixture.staging();
                analysis.stage(Uuid::new_v4(), &[2], &stage).unwrap();
                let expected = mak.replace("#unrelated.dat;", "");
                assert_eq!(
                    fs::read(stage.join("Cave.mak")).unwrap(),
                    mak_encoding.encode(&expected).0.as_ref()
                );
                assert_eq!(
                    fs::read(stage.join("Café.dat")).unwrap(),
                    provider_bytes.as_ref()
                );
                assert_eq!(
                    fs::read(stage.join("next.dat")).unwrap(),
                    dependent_bytes.as_ref()
                );
                assert!(!stage.join("unrelated.dat").exists());
                let metadata: toml::Value =
                    toml::from_str(&fs::read_to_string(stage.join("compass.toml")).unwrap())
                        .unwrap();
                assert_eq!(
                    metadata["project"]["dat_files"].as_array().unwrap(),
                    &vec![
                        toml::Value::String("Café.dat".into()),
                        toml::Value::String("next.dat".into())
                    ]
                );
                assert_eq!(fs::read(path).unwrap(), mak_bytes.as_ref());
                assert_eq!(fs::read(provider_path).unwrap(), provider_bytes.as_ref());
                assert_eq!(fs::read(dependent_path).unwrap(), dependent_bytes.as_ref());
            }
        }
    }

    #[test]
    fn remapped_paths_keep_the_mak_encoding() {
        for encoding in [UTF_8, WINDOWS_1252] {
            let fixture = Fixture::new();
            fixture.dat("Backshall´s.DAT", "A", "B");
            let (bytes, _, errors) = encoding.encode("/ Café\n#..\\Backshall´s.DAT;\n");
            assert!(!errors);
            let path = fixture.write("nested/Cave.mak", &bytes);
            let analysis = analyze(&path).unwrap();
            let stage = fixture.staging();
            analysis.stage(Uuid::new_v4(), &[0], &stage).unwrap();
            let expected = encoding
                .encode("/ Café\n#_imports\\0\\Backshall´s.DAT;\n")
                .0;
            assert_eq!(fs::read(stage.join("Cave.mak")).unwrap(), expected.as_ref());
            assert!(stage.join("_imports/0/Backshall´s.DAT").is_file());
            assert_eq!(fs::read(path).unwrap(), bytes.as_ref());
            assert_eq!(analyze(&stage.join("Cave.mak")).unwrap().sections.len(), 1);
        }
    }

    #[test]
    fn utf8_bom_and_unicode_names_still_round_trip() {
        let fixture = Fixture::new();
        fixture.dat("洞窟.dat", "A", "B");
        fixture.dat("Pequeña.dat", "C", "D");
        let mak = "\u{feff}/ Unicode café\r\n#洞窟.dat;\r\n#Pequeña.dat;\r\n";
        let analysis = analyze(&fixture.write("Cave.mak", mak)).unwrap();
        let stage = fixture.staging();
        analysis.stage(Uuid::new_v4(), &[1], &stage).unwrap();
        assert_eq!(
            fs::read_to_string(stage.join("Cave.mak")).unwrap(),
            mak.replace("#洞窟.dat;", "")
        );
        assert_eq!(analysis.sections[0].name, "洞窟.dat");
    }

    #[test]
    fn legacy_encoding_is_selected_per_file_not_per_filename() {
        let fixture = Fixture::new();
        // C3 A9 is valid UTF-8 in isolation, but this file is Windows-1252.
        fixture.dat("Ã©.dat", "A", "B");
        let mak = b"/ \xb4\n#\xc3\xa9.dat;\n";
        let analysis = analyze(&fixture.write("Cave.mak", mak)).unwrap();
        assert_eq!(analysis.sections[0].name, "Ã©.dat");
        let stage = fixture.staging();
        analysis.stage(Uuid::new_v4(), &[0], &stage).unwrap();
        assert_eq!(fs::read(stage.join("Cave.mak")).unwrap(), mak);
    }

    #[test]
    fn survey_labels_and_empty_teams_do_not_change_station_dependencies() {
        for team in ["Kim Davidsson", "", "   "] {
            for label in ["Silt in the reg line", "Café — survey", "Silt\tline"] {
                let fixture = Fixture::new();
                let dat = survey("ANJL3 ANSR1 44 330 6.52 0 0 0 0")
                    .replace("SURVEY NAME: A", &format!("SURVEY NAME: {label}"))
                    .replace("A surveyor", team);
                fixture.write("a.dat", &dat);
                fixture.dat("b.dat", "ANSR1", "ANSR2");
                fixture.dat("unrelated.dat", "C", "D");
                let analysis =
                    analyze(&fixture.write("Cave.mak", "#a.dat;#b.dat;#unrelated.dat;")).unwrap();
                assert!(
                    analysis.full_import_reason.is_none(),
                    "{:?}",
                    analysis.full_import_reason
                );
                assert_eq!(dependencies(&analysis, 1), BTreeSet::from([0]));
                let stage = fixture.staging();
                analysis.stage(Uuid::new_v4(), &[1], &stage).unwrap();
                assert_eq!(fs::read_to_string(stage.join("a.dat")).unwrap(), dat);
                assert!(!stage.join("unrelated.dat").exists());
            }
        }
    }

    #[test]
    fn multi_survey_dat_layouts_preserve_later_station_dependencies() {
        for form_feeds in [true, false] {
            let fixture = Fixture::new();
            let first = survey("ANBB0 ANBB1 13 220 -32.58 0 0 0 0")
                .replace("SURVEY NAME: A", "SURVEY NAME: Blackbees");
            let second = survey("ANJL3 ANSR1 44 330 6.52 0 0 0 0 #| #sm: passage")
                .replace("SURVEY NAME: A", "SURVEY NAME: Silt in the reg line")
                .replace("A surveyor", "");
            let mut dat = format!("{first}{second}").replace('\n', "\r\n");
            if !form_feeds {
                dat = dat.replace('\x0c', "");
            }
            let source = fixture.write("sections.dat", &dat);
            fixture.dat("connected.dat", "ANSR1", "END");
            fixture.dat("unrelated.dat", "C", "D");
            assert_eq!(
                index_dat(dat.as_bytes()).unwrap(),
                BTreeSet::from([
                    "ANBB0".into(),
                    "ANBB1".into(),
                    "ANJL3".into(),
                    "ANSR1".into()
                ])
            );
            let analysis =
                analyze(&fixture.write("Cave.mak", "#sections.dat;#connected.dat;#unrelated.dat;"))
                    .unwrap();
            assert!(
                analysis.full_import_reason.is_none(),
                "{:?}",
                analysis.full_import_reason
            );
            assert_eq!(dependencies(&analysis, 1), BTreeSet::from([0]));
            let stage = fixture.staging();
            analysis.stage(Uuid::new_v4(), &[1], &stage).unwrap();
            assert_eq!(
                fs::read(stage.join("sections.dat")).unwrap(),
                dat.as_bytes()
            );
            assert!(stage.join("connected.dat").exists());
            assert!(!stage.join("unrelated.dat").exists());
            assert_eq!(fs::read(source).unwrap(), dat.as_bytes());
        }
    }

    #[test]
    fn permissive_metadata_parsing_still_requires_complete_headers() {
        let valid = survey("A B 1 2 3 4 5 6 7")
            .replace("SURVEY NAME: A", "SURVEY NAME: Silt in the reg line")
            .replace("A surveyor", "");
        for malformed in [
            valid.replace("Silt in the reg line", ""),
            valid.replace("Silt in the reg line", "Silt\0line"),
            valid.replace("DECLINATION: 0 FORMAT: DDDDLUDRADLNF\n", ""),
            valid.replace("FROM TO LENGTH BEARING INC LEFT UP DOWN RIGHT\n", ""),
        ] {
            let fixture = Fixture::new();
            let source = fixture.write("unsupported.dat", &malformed);
            fixture.dat("valid.dat", "C", "D");
            let analysis =
                analyze(&fixture.write("Cave.mak", "#unsupported.dat;#valid.dat;")).unwrap();
            assert!(analysis.full_import_reason.is_some(), "{malformed:?}");
            let stage = fixture.staging();
            assert!(analysis.stage(Uuid::new_v4(), &[1], &stage).is_err());
            assert_eq!(fs::read_dir(&stage).unwrap().count(), 0);
            analysis.stage(Uuid::new_v4(), &[0, 1], &stage).unwrap();
            assert_eq!(
                fs::read(stage.join("unsupported.dat")).unwrap(),
                malformed.as_bytes()
            );
            assert_eq!(fs::read(source).unwrap(), malformed.as_bytes());
        }
    }

    #[test]
    fn delimited_flags_allow_empty_values_and_adjacent_comments() {
        for flags in [
            "#| #",
            "#|\t #",
            "#|L P#",
            "#|XXX#",
            "#|X#comment",
            "#| # sm Abbey #|comment",
        ] {
            let dat = survey(&format!("A B 1 2 3 4 5 6 7 {flags}\nB C 1 2 3 4 5 6 7"));
            assert_eq!(
                index_dat(dat.as_bytes()).unwrap(),
                BTreeSet::from(["A".into(), "B".into(), "C".into()])
            );
        }
        // A flag-like sequence in a station identifier or comment is not a flag.
        assert!(index_dat(survey("#|A B 1 2 3 4 5 6 7 #| # comment").as_bytes()).is_ok());
        for flags in ["#|", "#| X", "#|Q#", "#| X Q #"] {
            assert!(index_dat(survey(&format!("A B 1 2 3 4 5 6 7 {flags}")).as_bytes()).is_err());
        }
    }

    #[test]
    fn long_and_accented_station_names_retain_their_complete_identity() {
        let fixture = Fixture::new();
        fixture.write("a.dat", survey("KLDWNTTMres36 Ma´ax1 1 2 3 4 5 6 7"));
        fixture.write("b.dat", survey("Ma´ax1 B 1 2 3 4 5 6 7"));
        fixture.dat("c.dat", "KLDWNTTMres37", "C");
        fixture.dat("d.dat", "KLDWNTTMres36", "D");
        let analysis =
            analyze(&fixture.write("Cave.mak", "#a.dat;#b.dat;#c.dat;#d.dat,KLDWNTTMres36;"))
                .unwrap();
        assert!(
            analysis.full_import_reason.is_none(),
            "{:?}",
            analysis.full_import_reason
        );
        assert_eq!(dependencies(&analysis, 1), BTreeSet::from([0]));
        assert!(dependencies(&analysis, 2).is_empty());
        assert_eq!(dependencies(&analysis, 3), BTreeSet::from([0]));
        let stage = fixture.staging();
        analysis.stage(Uuid::new_v4(), &[3], &stage).unwrap();
        assert!(stage.join("a.dat").exists());
        assert!(!stage.join("b.dat").exists());
        assert!(!stage.join("c.dat").exists());
        assert!(stage.join("d.dat").exists());
    }

    #[test]
    fn utf16_and_binary_mak_files_are_rejected() {
        for bytes in [
            b"#bad\0.dat;".as_slice(),
            b"\xff\xfe#x.dat;",
            b"\xfe\xff#x.dat;",
        ] {
            assert!(scan_mak(bytes, text_encoding(bytes)).is_err());
        }
    }

    #[test]
    fn four_of_twenty_sections_stage_only_four_dat_files_without_modifying_sources() {
        let fixture = Fixture::new();
        let mut mak = String::from("/ the original project\n@1,2,3,13,0;\n&North American 1983;\n");
        for id in 0..20 {
            mak.push_str(&format!("#section{id}.dat;\n"));
            fixture.dat(
                &format!("section{id}.dat"),
                &format!("A{id}"),
                &format!("B{id}"),
            );
        }
        let path = fixture.write("Cave.mak", &mak);
        let analysis = analyze(&path).unwrap();
        assert!(analysis.full_import_reason.is_none());
        let destination = fixture.staging();
        let id = Uuid::new_v4();
        analysis.stage(id, &[0, 3, 7, 19], &destination).unwrap();
        let generated = fs::read_to_string(destination.join("Cave.mak")).unwrap();
        let records = scan_mak(generated.as_bytes(), UTF_8).unwrap().records;
        assert_eq!(
            records
                .iter()
                .map(|record| record.path.as_str())
                .collect::<Vec<_>>(),
            [
                "section0.dat",
                "section3.dat",
                "section7.dat",
                "section19.dat"
            ]
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), mak);
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 6);
        let metadata: toml::Value =
            toml::from_str(&fs::read_to_string(destination.join("compass.toml")).unwrap()).unwrap();
        assert_eq!(
            metadata["speleodb"]["id"].as_str(),
            Some(id.to_string().as_str())
        );
        assert_eq!(
            metadata["project"]["dat_files"].as_array().unwrap().len(),
            4
        );
        for selected in [0, 3, 7, 19] {
            assert_eq!(
                fs::read(destination.join(format!("section{selected}.dat"))).unwrap(),
                fs::read(fixture.0.join(format!("section{selected}.dat"))).unwrap()
            );
        }
    }

    #[test]
    fn all_selected_preserves_every_source_byte_and_scanner_handles_comments() {
        let fixture = Fixture::new();
        fixture.dat("a.dat", "A1", "A2");
        fixture.dat("b.dat", "B1", "B2");
        let mak = format!(
            "\u{feff}/ ignored #no.dat; [ /\r\n/{};#a.dat;\r\n/other ; comment\r\n#b.dat;\r\n\x1a",
            Uuid::new_v4()
        );
        let analysis = analyze(&fixture.write("Cave.mak", &mak)).unwrap();
        assert_eq!(analysis.sections.len(), 2);
        let stage = fixture.staging();
        analysis.stage(Uuid::new_v4(), &[0, 1], &stage).unwrap();
        assert_eq!(fs::read(stage.join("Cave.mak")).unwrap(), mak.as_bytes());
    }

    #[test]
    fn removing_records_retains_rolling_settings_and_multiline_station_records() {
        let fixture = Fixture::new();
        fixture.dat("a.dat", "A1", "A2");
        fixture.dat("b.dat", "B1", "B2");
        let mak = "@1,2,3,13,0;\r\n&North American 1983;\r\n#a.dat;\r\n$12;\r\n%-1.2;\r\n!GAVOTSCXPL;\r\n#b.dat,\r\n / ignored ; [ #x.dat; / B1[M,1,2,3];\r\n";
        let analysis = analyze(&fixture.write("Cave.mak", mak)).unwrap();
        assert!(analysis.full_import_reason.is_none());
        let stage = fixture.staging();
        analysis.stage(Uuid::new_v4(), &[1], &stage).unwrap();
        assert_eq!(
            fs::read_to_string(stage.join("Cave.mak")).unwrap(),
            mak.replace("#a.dat;", "")
        );
    }

    #[test]
    fn classic_comments_between_filename_links_and_fixed_coordinates_are_supported() {
        let fixture = Fixture::new();
        fixture.dat("FULFORD.DAT", "A", "B");
        let mak = "# FULFORD.DAT /comment 1\n ,/comment 2\n A / comment3\n [ f, 1.1, / comment4\n/comment5\n2.2, 3.3\n/comment6\n]\n/comment7\n, B\n,C[m,4.4,5.5,6.6]\n;\n";
        // B has no earlier provider, so this fixture exercises the scanner
        // directly, separately from the deliberate unresolved-link fallback.
        let scanned = scan_mak(mak.as_bytes(), UTF_8).unwrap();
        assert!(scanned.full_import_reason.is_none());
        assert_eq!(scanned.records[0].path, "FULFORD.DAT");
        assert_eq!(scanned.records[0].stations.len(), 3);
        assert!(scanned.records[0].stations[0].fixed);
        assert!(!scanned.records[0].stations[1].fixed);
        let analysis = analyze(&fixture.write("Cave.mak", mak)).unwrap();
        let stage = fixture.staging();
        analysis.stage(Uuid::new_v4(), &[0], &stage).unwrap();
        assert_eq!(fs::read_to_string(stage.join("Cave.mak")).unwrap(), mak);
    }

    #[test]
    fn explicit_links_shared_stations_carry_links_and_transitive_selection() {
        let fixture = Fixture::new();
        fixture.dat("a.dat", "A1", "A2");
        fixture.dat("b.dat", "A1", "B1");
        fixture.dat("c.dat", "A2", "C1");
        fixture.dat("d.dat", "C1", "D1");
        let analysis =
            analyze(&fixture.write("Cave.mak", "#a.dat;\n#b.dat,A1,A2;\n#c.dat,A2;\n#d.dat;\n"))
                .unwrap();
        assert!(analysis.full_import_reason.is_none());
        assert_eq!(dependencies(&analysis, 1), BTreeSet::from([0]));
        assert_eq!(dependencies(&analysis, 2), BTreeSet::from([0]));
        assert_eq!(dependencies(&analysis, 3), BTreeSet::from([2]));
        let stage = fixture.staging();
        analysis.stage(Uuid::new_v4(), &[3], &stage).unwrap();
        assert!(stage.join("a.dat").exists());
        assert!(!stage.join("b.dat").exists());
        assert!(stage.join("c.dat").exists());
    }

    #[test]
    fn fixed_station_resets_namespace_and_context_dependency_is_conservative() {
        let fixture = Fixture::new();
        fixture.dat("a.dat", "X", "A1");
        fixture.dat("b.dat", "X", "B1");
        fixture.dat("c.dat", "Z", "C1");
        let analysis =
            analyze(&fixture.write("Cave.mak", "#a.dat;#b.dat,X[F,1,2,3];#c.dat;")).unwrap();
        assert!(dependencies(&analysis, 1).is_empty());
        assert_eq!(dependencies(&analysis, 2), BTreeSet::from([1]));
        assert_eq!(
            analysis.sections[2].dependencies[0].reason,
            "Preserves station context"
        );
    }

    #[test]
    fn provenance_sets_include_later_real_providers_after_x_flagged_shots() {
        let fixture = Fixture::new();
        fixture.write("a.dat", survey("X A1 1 2 3 4 5 6 7 #|X#"));
        fixture.dat("b.dat", "X", "B1");
        fixture.dat("c.dat", "X", "C1");
        let analysis = analyze(&fixture.write("Cave.mak", "#a.dat;#b.dat;#c.dat;")).unwrap();
        assert!(analysis.full_import_reason.is_none());
        assert_eq!(dependencies(&analysis, 2), BTreeSet::from([0, 1]));
    }

    #[test]
    fn station_names_are_case_sensitive_and_reversed_shots_are_indexed() {
        let fixture = Fixture::new();
        fixture.dat("a.dat", "A1", "X");
        fixture.dat("b.dat", "a1", "B1");
        fixture.dat("c.dat", "C1", "A1");
        let analysis = analyze(&fixture.write("Cave.mak", "#a.dat;#b.dat;#c.dat;")).unwrap();
        assert!(dependencies(&analysis, 1).is_empty());
        assert_eq!(dependencies(&analysis, 2), BTreeSet::from([0]));
    }

    #[test]
    fn known_fulford_fixture_discovers_connection_without_explicit_mak_links() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/test_data/Fulfords.mak");
        let analysis = analyze(&path).unwrap();
        assert_eq!(analysis.full_import_reason, None);
        assert_eq!(analysis.sections.len(), 2);
        assert_eq!(dependencies(&analysis, 1), BTreeSet::from([0]));
    }

    #[test]
    fn logical_folders_use_raw_paths_and_only_allow_full_import() {
        let fixture = Fixture::new();
        fixture.dat("a.dat", "A1", "A2");
        fixture.dat("nested/b.dat", "B1", "B2");
        let mak = "[Logical folder;#a.dat;[Second folder;#nested\\b.dat;];];";
        let analysis = analyze(&fixture.write("Cave.mak", mak)).unwrap();
        assert!(analysis.full_import_reason.is_some());
        let stage = fixture.staging();
        assert!(analysis.stage(Uuid::new_v4(), &[0], &stage).is_err());
        assert_eq!(fs::read_dir(&stage).unwrap().count(), 0);
        analysis.stage(Uuid::new_v4(), &[0, 1], &stage).unwrap();
        assert_eq!(fs::read_to_string(stage.join("Cave.mak")).unwrap(), mak);
        assert!(stage.join("nested/b.dat").exists());
    }

    #[test]
    fn unsupported_dat_partial_content_flags_prefixes_and_splays_offer_full_import() {
        for contents in [
            "unsupported DAT".into(),
            format!("{}trailing garbage", survey("A1 A2 1 2 3 4 5 6 7")),
            survey("A1 A2 1 2 3 4 5 6 7 #|Q#"),
            survey("A1 )A2 1 2 3 4 5 6 7"),
            survey("A1 A2 1 2 3 4 5 6 7").replace("FORMAT:", "PREFIX: X FORMAT:"),
            survey("A1 A2 incomplete"),
        ] {
            let fixture = Fixture::new();
            fixture.write("a.dat", &contents);
            fixture.dat("b.dat", "B1", "B2");
            let analysis = analyze(&fixture.write("Cave.mak", "#a.dat;#b.dat;")).unwrap();
            assert!(analysis.full_import_reason.is_some(), "{contents}");
            let stage = fixture.staging();
            assert!(analysis.stage(Uuid::new_v4(), &[1], &stage).is_err());
            analysis.stage(Uuid::new_v4(), &[0, 1], &stage).unwrap();
            assert_eq!(fs::read_to_string(stage.join("a.dat")).unwrap(), contents);
        }
    }

    #[test]
    fn missing_and_ambiguous_links_require_full_import() {
        for mak in ["#a.dat,MISSING;#b.dat;", "#a.dat,A1,A1[F,1,2,3];#b.dat;"] {
            let fixture = Fixture::new();
            fixture.dat("a.dat", "A1", "A2");
            fixture.dat("b.dat", "B1", "B2");
            let analysis = analyze(&fixture.write("Cave.mak", mak)).unwrap();
            assert!(analysis.full_import_reason.is_some());
            assert!(
                analysis
                    .sections
                    .iter()
                    .all(|section| section.dependencies.is_empty())
            );
        }
    }

    #[test]
    fn invalid_mak_inventory_and_missing_dat_fail_before_staging() {
        for mak in [
            "#missing.dat;",
            "#a.dat",
            "?unknown;#a.dat;",
            "[folder;#a.dat;",
            "];#a.dat;",
            "#a.dat,A1[F,1,2,3;",
        ] {
            let fixture = Fixture::new();
            fixture.dat("a.dat", "A1", "A2");
            assert!(analyze(&fixture.write("Cave.mak", mak)).is_err(), "{mak}");
        }
    }

    #[test]
    fn changed_mak_or_even_excluded_dat_invalidates_preview() {
        for changed in ["Cave.mak", "b.dat"] {
            let fixture = Fixture::new();
            fixture.dat("a.dat", "A1", "A2");
            fixture.dat("b.dat", "B1", "B2");
            let analysis = analyze(&fixture.write("Cave.mak", "#a.dat;#b.dat;")).unwrap();
            fixture.write(changed, "changed bytes");
            let stage = fixture.staging();
            assert!(matches!(
                analysis.stage(Uuid::new_v4(), &[0], &stage),
                Err(Error::ImportSourceChanged(_))
            ));
            assert_eq!(fs::read_dir(&stage).unwrap().count(), 0);
        }
    }

    #[test]
    fn duplicate_record_reuses_one_payload_but_keeps_each_record() {
        let fixture = Fixture::new();
        fixture.dat("a.dat", "A1", "A2");
        let mak = "#a.dat;#a.dat,A1[F,1,2,3];";
        let analysis = analyze(&fixture.write("Cave.mak", mak)).unwrap();
        assert_eq!(analysis.sources.len(), 1);
        let stage = fixture.staging();
        analysis.stage(Uuid::new_v4(), &[0, 1], &stage).unwrap();
        assert_eq!(fs::read_dir(&stage).unwrap().count(), 3);
        assert_eq!(fs::read_to_string(stage.join("Cave.mak")).unwrap(), mak);
    }

    #[test]
    fn parent_and_absolute_references_remap_and_canonical_aliases_reuse_payload() {
        let fixture = Fixture::new();
        fixture.dat("external.dat", "A1", "A2");
        let absolute = fixture.0.join("external.dat").display().to_string();
        let mak = format!("#../external.dat;\n#{absolute};\n");
        let source = fixture.write("project/Cave.mak", &mak);
        let analysis = analyze(&source).unwrap();
        assert_eq!(analysis.sources.len(), 1);
        assert_eq!(analysis.sources[0].destination, "_imports/0/external.dat");
        let stage = fixture.staging();
        analysis.stage(Uuid::new_v4(), &[0, 1], &stage).unwrap();
        assert_eq!(
            fs::read_to_string(stage.join("Cave.mak")).unwrap(),
            "#_imports\\0\\external.dat;\n#_imports\\0\\external.dat;\n"
        );
        assert_eq!(fs::read_to_string(source).unwrap(), mak);
    }

    #[test]
    fn reserved_names_and_output_file_directory_collisions_are_portably_remapped() {
        assert!(!portable_component("CON.dat"));
        assert!(!portable_component("LPT¹.dat"));
        assert!(portable_relative("../escape.dat").is_none());
        assert!(portable_relative("C:\\cave\\a.dat").is_none());
        let occupied = BTreeSet::from([
            "foo.dat".into(),
            "nested/file.dat".into(),
            "_imports".into(),
        ]);
        assert!(!destination_available("FOO.DAT", &occupied));
        assert!(!destination_available("foo.dat/other.dat", &occupied));
        assert!(!destination_available("nested", &occupied));
        assert_eq!(
            remapped_destination(3, "../a.dat", &occupied),
            "_imports_1/3/a.dat"
        );
    }

    #[test]
    fn staging_rejects_empty_unknown_selection_or_existing_contents() {
        let fixture = Fixture::new();
        fixture.dat("a.dat", "A1", "A2");
        let analysis = analyze(&fixture.write("Cave.mak", "#a.dat;")).unwrap();
        let stage = fixture.staging();
        assert!(analysis.stage(Uuid::new_v4(), &[], &stage).is_err());
        assert!(analysis.stage(Uuid::new_v4(), &[9], &stage).is_err());
        fs::write(stage.join("precious.txt"), "keep").unwrap();
        assert!(analysis.stage(Uuid::new_v4(), &[0], &stage).is_err());
        assert_eq!(
            fs::read_to_string(stage.join("precious.txt")).unwrap(),
            "keep"
        );
    }

    #[cfg(unix)]
    #[test]
    fn source_symlinks_copy_bytes_and_retargeted_aliases_invalidate_preview() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        fixture.dat("real.dat", "A1", "A2");
        fixture.dat("other.dat", "B1", "B2");
        symlink(fixture.0.join("real.dat"), fixture.0.join("alias.dat")).unwrap();
        let analysis = analyze(&fixture.write("Cave.mak", "#real.dat;#alias.dat;")).unwrap();
        let stage = fixture.staging();
        analysis.stage(Uuid::new_v4(), &[0, 1], &stage).unwrap();
        assert!(
            !fs::symlink_metadata(stage.join("real.dat"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::remove_file(fixture.0.join("alias.dat")).unwrap();
        symlink(fixture.0.join("other.dat"), fixture.0.join("alias.dat")).unwrap();
        assert!(matches!(
            analysis.stage(Uuid::new_v4(), &[0, 1], &fixture.staging()),
            Err(Error::ImportSourceChanged(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_mak_resolves_dats_beside_the_selected_mak() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        fixture.write("original/Cave.mak", "#survey.dat;");
        fixture.dat("original/survey.dat", "ORIGINAL1", "ORIGINAL2");
        fixture.dat("alias/survey.dat", "ALIAS1", "ALIAS2");
        let selected = fixture.0.join("alias/Cave.mak");
        symlink(fixture.0.join("original/Cave.mak"), &selected).unwrap();
        let analysis = analyze(&selected).unwrap();
        assert_eq!(analysis.source_directory, fixture.0.join("alias"));
        let stage = fixture.staging();
        analysis.stage(Uuid::new_v4(), &[0], &stage).unwrap();
        assert_eq!(
            fs::read(stage.join("survey.dat")).unwrap(),
            fs::read(fixture.0.join("alias/survey.dat")).unwrap()
        );
        fixture.dat("alias/survey.dat", "CHANGED1", "CHANGED2");
        assert!(matches!(
            analysis.stage(Uuid::new_v4(), &[0], &fixture.staging()),
            Err(Error::ImportSourceChanged(_))
        ));
    }

    #[test]
    fn an_exact_case_file_superseding_a_fallback_invalidates_preview() {
        let fixture = Fixture::new();
        fixture.dat("cave.dat", "A1", "A2");
        // Only case-sensitive filesystems can represent this source change.
        if fixture.0.join("CAVE.DAT").exists() {
            return;
        }
        let analysis = analyze(&fixture.write("Cave.mak", "#CAVE.DAT;")).unwrap();
        fixture.dat("CAVE.DAT", "B1", "B2");
        let stage = fixture.staging();
        assert!(matches!(
            analysis.stage(Uuid::new_v4(), &[0], &stage),
            Err(Error::ImportSourceChanged(_))
        ));
        assert_eq!(fs::read_dir(stage).unwrap().count(), 0);
    }

    #[test]
    fn invalid_date_is_not_a_panic_and_full_shot_coverage_is_required() {
        // Dates are metadata for this index; no date library or upstream parser is used.
        let dat = survey("A1 A2 1 2 3 4 5 6 7").replace("1 2 2020", "13 42 2020");
        assert_eq!(
            index_dat(dat.as_bytes()).unwrap(),
            BTreeSet::from(["A1".into(), "A2".into()])
        );
        let malformed = format!(
            "{}{}",
            survey("A1 A2 1 2 3 4 5 6 7"),
            survey("B1 B2 incomplete")
        );
        assert!(index_dat(malformed.as_bytes()).is_err());
    }

    #[test]
    fn malformed_next_header_cannot_consume_an_actual_shot_as_a_cave_name() {
        let first = survey("X A1 1 2 3 4 5 6 7");
        let second = survey("B1 B2 1 2 3 4 5 6 7");
        let second = second.strip_prefix("Cave\n").unwrap();
        let malformed = format!("{first}{second}");
        assert!(index_dat(malformed.as_bytes()).is_err());
        assert!(index_dat(malformed.replace('\x0c', "").as_bytes()).is_err());
    }

    #[test]
    fn non_mak_sources_cannot_collide_with_generated_metadata() {
        let fixture = Fixture::new();
        fixture.dat("a.dat", "A1", "A2");
        let error = analyze(&fixture.write("compass.toml", "#a.dat;")).unwrap_err();
        assert!(matches!(error, Error::CompassProject(message) if message.contains(".MAK")));
    }
}
