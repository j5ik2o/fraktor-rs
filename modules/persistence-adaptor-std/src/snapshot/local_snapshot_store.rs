//! Filesystem-backed local snapshot store.

#[cfg(test)]
#[path = "local_snapshot_store_test.rs"]
mod tests;

use core::{
  any::Any,
  future::{Ready, ready},
  ops::Deref,
};
use std::{
  ffi::OsStr,
  fs::{self, File},
  io::{Error, ErrorKind, Write},
  path::{Path, PathBuf},
};

use fraktor_actor_core_kernel_rs::serialization::{
  SerializationDelegator, SerializedMessage, serialization_registry::SerializationRegistry,
};
use fraktor_persistence_core_kernel_rs::{
  serialization::SnapshotPayload,
  snapshot::{Snapshot, SnapshotError, SnapshotMetadata, SnapshotSelectionCriteria, SnapshotStore},
};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::snapshot::LocalSnapshotStoreConfig;

const SNAPSHOT_FILE_PREFIX: &str = "snapshot-";
const SNAPSHOT_FILE_SEPARATOR: char = '-';
const SNAPSHOT_TEMP_EXTENSION: &str = "tmp";
const SNAPSHOT_METADATA_EXTENSION: &str = "meta";
const SNAPSHOT_PAYLOAD_TYPE_NAME: &str = "SnapshotPayload";
const PERCENT_ENCODING_MARKER: char = '%';
const SPACE_BYTE: u8 = b' ';
const PLUS_BYTE: u8 = b'+';
const HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";

#[derive(Clone, Debug)]
struct SnapshotCandidate {
  metadata: SnapshotMetadata,
  path:     PathBuf,
}

/// Filesystem-backed snapshot store compatible with the kernel [`SnapshotStore`] trait.
#[derive(Clone)]
pub struct LocalSnapshotStore {
  directory:         PathBuf,
  serialization:     ArcShared<SerializationRegistry>,
  max_load_attempts: usize,
}

impl LocalSnapshotStore {
  /// Opens a local snapshot store and creates its root directory when missing.
  ///
  /// # Errors
  ///
  /// Returns a [`SnapshotError`] when the configuration is invalid or the root directory cannot be
  /// created.
  pub fn open(config: LocalSnapshotStoreConfig) -> Result<Self, SnapshotError> {
    config.validate()?;
    let (directory, serialization, max_load_attempts) = config.into_parts();
    fs::create_dir_all(&directory).map_err(|error| {
      SnapshotError::LoadFailed(format!("create snapshot directory {}: {error}", directory.display()))
    })?;
    Ok(Self { directory, serialization, max_load_attempts })
  }

  fn save_snapshot_sync(
    &self,
    metadata: &SnapshotMetadata,
    snapshot: ArcShared<dyn Any + Send + Sync>,
  ) -> Result<(), SnapshotError> {
    let payload = SnapshotPayload::new(snapshot);
    let delegator = SerializationDelegator::new(self.serialization.deref());
    let serialized = delegator
      .serialize(&payload, SNAPSHOT_PAYLOAD_TYPE_NAME)
      .map_err(|error| SnapshotError::SaveFailed(format!("serialize snapshot: {error}")))?;
    let bytes = serialized.encode();
    let path = self.snapshot_path(metadata);
    let temp_path = Self::temp_snapshot_path(&path);
    let mut file = File::create(&temp_path)
      .map_err(|error| SnapshotError::SaveFailed(format!("create temp snapshot {}: {error}", temp_path.display())))?;
    file
      .write_all(&bytes)
      .map_err(|error| SnapshotError::SaveFailed(format!("write temp snapshot {}: {error}", temp_path.display())))?;
    file
      .sync_all()
      .map_err(|error| SnapshotError::SaveFailed(format!("sync temp snapshot {}: {error}", temp_path.display())))?;
    drop(file);
    Self::replace_temp_file(&temp_path, &path, "snapshot")?;
    if let Err(error) = self.save_snapshot_metadata(metadata, &path) {
      match fs::remove_file(&path) {
        | Ok(()) | Err(_) => (),
      }
      return Err(error);
    }
    #[cfg(not(windows))]
    File::open(&self.directory).and_then(|directory| directory.sync_all()).map_err(|error| {
      SnapshotError::SaveFailed(format!("sync snapshot directory {}: {error}", self.directory.display()))
    })?;
    Ok(())
  }

  fn load_snapshot_sync(
    &self,
    persistence_id: &str,
    criteria: &SnapshotSelectionCriteria,
  ) -> Result<Option<Snapshot>, SnapshotError> {
    let candidates = self.snapshot_candidates(persistence_id, criteria).map_err(|error| {
      SnapshotError::LoadFailed(format!("list snapshot directory {}: {error}", self.directory.display()))
    })?;
    if candidates.is_empty() {
      return Ok(None);
    }

    let mut last_error = None;
    for candidate in candidates.iter().take(self.max_load_attempts) {
      match self.load_candidate(candidate) {
        | Ok(snapshot) => return Ok(Some(snapshot)),
        | Err(error) => last_error = Some(error),
      }
    }

    Err(
      last_error
        .unwrap_or_else(|| SnapshotError::LoadFailed(String::from("no local snapshot candidate was attempted"))),
    )
  }

  fn delete_snapshot_sync(&self, metadata: &SnapshotMetadata) -> Result<(), SnapshotError> {
    let candidates =
      self.snapshot_candidates(metadata.persistence_id(), &SnapshotSelectionCriteria::latest()).map_err(|error| {
        SnapshotError::DeleteFailed(format!("list snapshot directory {}: {error}", self.directory.display()))
      })?;
    for candidate in candidates {
      let candidate_metadata = self.snapshot_candidate_metadata(&candidate).map_err(|error| {
        SnapshotError::DeleteFailed(format!("read snapshot metadata {}: {error}", candidate.path.display()))
      })?;
      if &candidate_metadata == metadata {
        self.remove_snapshot_file(&candidate.path)?;
      }
    }
    Ok(())
  }

  fn delete_snapshots_sync(
    &self,
    persistence_id: &str,
    criteria: &SnapshotSelectionCriteria,
  ) -> Result<(), SnapshotError> {
    let candidates = self.snapshot_candidates(persistence_id, criteria).map_err(|error| {
      SnapshotError::DeleteFailed(format!("list snapshot directory {}: {error}", self.directory.display()))
    })?;
    for candidate in candidates {
      self.remove_snapshot_file(&candidate.path)?;
    }
    Ok(())
  }

  fn load_candidate(&self, candidate: &SnapshotCandidate) -> Result<Snapshot, SnapshotError> {
    let bytes = fs::read(&candidate.path)
      .map_err(|error| SnapshotError::LoadFailed(format!("read snapshot {}: {error}", candidate.path.display())))?;
    let serialized = SerializedMessage::decode(&bytes)
      .map_err(|error| SnapshotError::LoadFailed(format!("decode snapshot {}: {error}", candidate.path.display())))?;
    let delegator = SerializationDelegator::new(self.serialization.deref());
    let payload = delegator.deserialize(&serialized, None).map_err(|error| {
      SnapshotError::LoadFailed(format!("deserialize snapshot {}: {error}", candidate.path.display()))
    })?;
    let payload = payload.downcast::<SnapshotPayload>().map_err(|_| {
      SnapshotError::LoadFailed(format!("deserialize snapshot {}: payload type mismatch", candidate.path.display()))
    })?;
    let metadata = self.snapshot_candidate_metadata(candidate)?;
    Ok(Snapshot::new(metadata, payload.data().clone()))
  }

  fn snapshot_candidate_metadata(&self, candidate: &SnapshotCandidate) -> Result<SnapshotMetadata, SnapshotError> {
    match self.load_snapshot_metadata(&candidate.path)? {
      | Some(metadata) => Ok(candidate.metadata.clone().with_metadata(metadata)),
      | None => Ok(candidate.metadata.clone()),
    }
  }

  fn snapshot_candidates(
    &self,
    persistence_id: &str,
    criteria: &SnapshotSelectionCriteria,
  ) -> Result<Vec<SnapshotCandidate>, Error> {
    let mut candidates = Vec::new();
    for entry in fs::read_dir(&self.directory)? {
      let entry = entry?;
      let path = entry.path();
      if matches!(
        path.extension(),
        Some(extension)
          if extension == OsStr::new(SNAPSHOT_TEMP_EXTENSION) || extension == OsStr::new(SNAPSHOT_METADATA_EXTENSION)
      ) {
        continue;
      }
      let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
        continue;
      };
      let Some(metadata) = Self::parse_snapshot_file_name(file_name) else {
        continue;
      };
      if metadata.persistence_id() == persistence_id && criteria.matches(&metadata) {
        candidates.push(SnapshotCandidate { metadata, path });
      }
    }
    candidates.sort_by(|left, right| {
      right
        .metadata
        .sequence_nr()
        .cmp(&left.metadata.sequence_nr())
        .then_with(|| right.metadata.timestamp().cmp(&left.metadata.timestamp()))
    });
    Ok(candidates)
  }

  fn remove_snapshot_file(&self, path: &Path) -> Result<(), SnapshotError> {
    match fs::remove_file(path) {
      | Ok(()) => Ok(()),
      | Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
      | Err(error) => Err(SnapshotError::DeleteFailed(format!("delete snapshot {}: {error}", path.display()))),
    }?;
    let metadata_path = Self::snapshot_metadata_path(path);
    match fs::remove_file(&metadata_path) {
      | Ok(()) => Ok(()),
      | Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
      | Err(error) => {
        Err(SnapshotError::DeleteFailed(format!("delete snapshot metadata {}: {error}", metadata_path.display())))
      },
    }
  }

  fn save_snapshot_metadata(&self, metadata: &SnapshotMetadata, path: &Path) -> Result<(), SnapshotError> {
    let metadata_path = Self::snapshot_metadata_path(path);
    let Some(metadata) = metadata.metadata() else {
      return Self::remove_stale_snapshot_metadata(&metadata_path);
    };
    let temp_metadata_path = Self::temp_snapshot_path(&metadata_path);
    let mut file = File::create(&temp_metadata_path).map_err(|error| {
      SnapshotError::SaveFailed(format!("create temp snapshot metadata {}: {error}", temp_metadata_path.display()))
    })?;
    file.write_all(metadata.as_bytes()).map_err(|error| {
      SnapshotError::SaveFailed(format!("write temp snapshot metadata {}: {error}", temp_metadata_path.display()))
    })?;
    file.sync_all().map_err(|error| {
      SnapshotError::SaveFailed(format!("sync temp snapshot metadata {}: {error}", temp_metadata_path.display()))
    })?;
    drop(file);
    Self::replace_temp_file(&temp_metadata_path, &metadata_path, "snapshot metadata")?;
    Ok(())
  }

  fn replace_temp_file(temp_path: &Path, path: &Path, label: &str) -> Result<(), SnapshotError> {
    #[cfg(windows)]
    match fs::remove_file(path) {
      | Ok(()) => (),
      | Err(error) if error.kind() == ErrorKind::NotFound => (),
      | Err(error) => {
        return Err(SnapshotError::SaveFailed(format!("remove existing {label} {}: {error}", path.display())));
      },
    }
    fs::rename(temp_path, path).map_err(|error| {
      SnapshotError::SaveFailed(format!("rename temp {label} {} to {}: {error}", temp_path.display(), path.display()))
    })
  }

  fn remove_stale_snapshot_metadata(metadata_path: &Path) -> Result<(), SnapshotError> {
    match fs::remove_file(metadata_path) {
      | Ok(()) => Ok(()),
      | Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
      | Err(error) => {
        Err(SnapshotError::SaveFailed(format!("delete snapshot metadata {}: {error}", metadata_path.display())))
      },
    }
  }

  fn load_snapshot_metadata(&self, path: &Path) -> Result<Option<String>, SnapshotError> {
    let metadata_path = Self::snapshot_metadata_path(path);
    match fs::read_to_string(&metadata_path) {
      | Ok(metadata) => Ok(Some(metadata)),
      | Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
      | Err(error) => {
        Err(SnapshotError::LoadFailed(format!("read snapshot metadata {}: {error}", metadata_path.display())))
      },
    }
  }

  fn snapshot_metadata_path(path: &Path) -> PathBuf {
    match path.file_name().and_then(OsStr::to_str) {
      | Some(file_name) => path.with_file_name(format!("{file_name}.{SNAPSHOT_METADATA_EXTENSION}")),
      | None => path.with_extension(SNAPSHOT_METADATA_EXTENSION),
    }
  }

  fn snapshot_path(&self, metadata: &SnapshotMetadata) -> PathBuf {
    self.directory.join(Self::snapshot_file_name(metadata))
  }

  fn temp_snapshot_path(path: &Path) -> PathBuf {
    match path.file_name().and_then(OsStr::to_str) {
      | Some(file_name) => path.with_file_name(format!("{file_name}.{SNAPSHOT_TEMP_EXTENSION}")),
      | None => path.with_extension(SNAPSHOT_TEMP_EXTENSION),
    }
  }

  fn snapshot_file_name(metadata: &SnapshotMetadata) -> String {
    format!(
      "{SNAPSHOT_FILE_PREFIX}{}{SNAPSHOT_FILE_SEPARATOR}{}{SNAPSHOT_FILE_SEPARATOR}{}",
      Self::encode_persistence_id(metadata.persistence_id()),
      metadata.sequence_nr(),
      metadata.timestamp()
    )
  }

  fn parse_snapshot_file_name(file_name: &str) -> Option<SnapshotMetadata> {
    let rest = file_name.strip_prefix(SNAPSHOT_FILE_PREFIX)?;
    let mut segments = rest.rsplitn(3, SNAPSHOT_FILE_SEPARATOR);
    let timestamp = segments.next()?.parse::<u64>().ok()?;
    let sequence_nr = segments.next()?.parse::<u64>().ok()?;
    let encoded_persistence_id = segments.next()?;
    let persistence_id = Self::decode_persistence_id(encoded_persistence_id)?;
    Some(SnapshotMetadata::new(persistence_id, sequence_nr, timestamp))
  }

  fn encode_persistence_id(persistence_id: &str) -> String {
    let mut encoded = String::new();
    for byte in persistence_id.bytes() {
      if byte == SPACE_BYTE {
        encoded.push(char::from(PLUS_BYTE));
      } else if Self::is_form_urlencoded_safe(byte) {
        encoded.push(char::from(byte));
      } else {
        encoded.push(PERCENT_ENCODING_MARKER);
        encoded.push(char::from(HEX_DIGITS[(byte >> 4) as usize]));
        encoded.push(char::from(HEX_DIGITS[(byte & 0x0F) as usize]));
      }
    }
    encoded
  }

  fn decode_persistence_id(encoded: &str) -> Option<String> {
    let mut bytes = Vec::new();
    let mut iter = encoded.as_bytes().iter().copied();
    while let Some(byte) = iter.next() {
      if byte == PERCENT_ENCODING_MARKER as u8 {
        let high = Self::decode_hex(iter.next()?)?;
        let low = Self::decode_hex(iter.next()?)?;
        bytes.push((high << 4) | low);
      } else if byte == PLUS_BYTE {
        bytes.push(SPACE_BYTE);
      } else {
        bytes.push(byte);
      }
    }
    String::from_utf8(bytes).ok()
  }

  const fn is_form_urlencoded_safe(byte: u8) -> bool {
    matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.')
  }

  const fn decode_hex(byte: u8) -> Option<u8> {
    match byte {
      | b'0'..=b'9' => Some(byte - b'0'),
      | b'A'..=b'F' => Some(byte - b'A' + 10),
      | b'a'..=b'f' => Some(byte - b'a' + 10),
      | _ => None,
    }
  }
}

impl SnapshotStore for LocalSnapshotStore {
  type DeleteManyFuture<'a>
    = Ready<Result<(), SnapshotError>>
  where
    Self: 'a;
  type DeleteOneFuture<'a>
    = Ready<Result<(), SnapshotError>>
  where
    Self: 'a;
  type LoadFuture<'a>
    = Ready<Result<Option<Snapshot>, SnapshotError>>
  where
    Self: 'a;
  type SaveFuture<'a>
    = Ready<Result<(), SnapshotError>>
  where
    Self: 'a;

  fn save_snapshot<'a>(
    &'a mut self,
    metadata: SnapshotMetadata,
    snapshot: ArcShared<dyn Any + Send + Sync>,
  ) -> Self::SaveFuture<'a> {
    ready(self.save_snapshot_sync(&metadata, snapshot))
  }

  fn load_snapshot<'a>(&'a self, persistence_id: &'a str, criteria: SnapshotSelectionCriteria) -> Self::LoadFuture<'a> {
    ready(self.load_snapshot_sync(persistence_id, &criteria))
  }

  fn delete_snapshot<'a>(&'a mut self, metadata: &'a SnapshotMetadata) -> Self::DeleteOneFuture<'a> {
    ready(self.delete_snapshot_sync(metadata))
  }

  fn delete_snapshots<'a>(
    &'a mut self,
    persistence_id: &'a str,
    criteria: SnapshotSelectionCriteria,
  ) -> Self::DeleteManyFuture<'a> {
    ready(self.delete_snapshots_sync(persistence_id, &criteria))
  }
}
