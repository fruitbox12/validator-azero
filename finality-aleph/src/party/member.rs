use crate::{
    crypto::KeyBox,
    data_io::{AlephData, OrderedDataInterpreter},
    network::{AlephNetworkData, DataNetwork, NetworkWrapper},
    party::{AuthoritySubtaskCommon, Task},
};
use aleph_bft::{Config, LocalIO, SpawnHandle};
use futures::channel::oneshot;
use log::{debug, error};
use sc_client_api::HeaderBackend;
use sp_runtime::traits::Block;
use std::{
    fmt, fs,
    fs::File,
    io,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

/// Runs the member within a single session.
pub fn task<
    B: Block,
    C: HeaderBackend<B> + Send + 'static,
    ADN: DataNetwork<AlephNetworkData<B>> + 'static,
>(
    subtask_common: AuthoritySubtaskCommon,
    multikeychain: KeyBox,
    config: Config,
    network: NetworkWrapper<AlephNetworkData<B>, ADN>,
    data_provider: impl aleph_bft::DataProvider<AlephData<B>> + Send + 'static,
    ordered_data_interpreter: OrderedDataInterpreter<B, C>,
    backup_saving_path: Option<PathBuf>,
) -> Task {
    let AuthoritySubtaskCommon {
        spawn_handle,
        session_id,
    } = subtask_common;
    let (stop, exit) = oneshot::channel();
    let task = {
        let spawn_handle = spawn_handle.clone();
        async move {
            debug!(target: "aleph-party", "Loading AlephBFT backup for {:?}", session_id);
            let (saver, loader): (Box<dyn Write + Send>, Box<dyn Read + Send>) =
                if let Some(stash_path) = backup_saving_path.as_deref() {
                    let (saver, loader) = match rotate_saved_backup_files(stash_path, session_id) {
                        Err(err) => {
                            error!(
                                target: "AlephBFT-member",
                                "Error setting up backup saving for session {}: {}",
                                session_id, err
                            );
                            return;
                        }
                        Ok((saver, loader)) => (saver, loader),
                    };
                    (Box::new(saver), Box::new(loader))
                } else {
                    (Box::new(io::sink()), Box::new(io::empty()))
                };
            let local_io = LocalIO::new(data_provider, ordered_data_interpreter, saver, loader);
            debug!(target: "aleph-party", "Running the member task for {:?}", session_id);
            aleph_bft::run_session(config, local_io, network, multikeychain, spawn_handle, exit)
                .await;
            debug!(target: "aleph-party", "Member task stopped for {:?}", session_id);
        }
    };

    let handle = spawn_handle.spawn_essential("aleph/consensus_session_member", task);
    Task::new(handle, stop)
}

#[derive(Debug)]
enum BackupLoadError {
    BackupIncomplete(Vec<usize>),
    IOError(io::Error),
}

impl fmt::Display for BackupLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackupLoadError::BackupIncomplete(backups) => {
                write!(
                    f,
                    "Backup is not complete. Got backup for sessions {:?}",
                    backups
                )
            }
            BackupLoadError::IOError(err) => {
                write!(f, "Backup could not be loaded because of IO error: {}", err)
            }
        }
    }
}

impl From<io::Error> for BackupLoadError {
    fn from(err: io::Error) -> Self {
        Self::IOError(err)
    }
}

impl std::error::Error for BackupLoadError {}

/// Loads the existing backups, and opens a new backup file to write to.
///
/// `backup_path` is the path to the backup directory (i.e. the argument to `--backup-saving-path`).
///
/// Returns the newly-created file (opened for writing), and the concatenation of the contents of
/// all existing files.
///
/// Current directory structure (this is an implementation detail, not part of the public API):
///   backup-stash/      - the main directory, backup_path/--backup-saving-path
///   `-- 18723/         - subdirectory for the current session
///       |-- 0.abfts    - files containing data
///       |-- 1.abfts    - each restart after a crash will cause another one to be created
///       |-- 2.abfts    - these numbers count up sequentially
///       `-- 3.abfts
fn rotate_saved_backup_files(
    backup_path: &Path,
    session_id: u32,
) -> Result<(File, Cursor<Vec<u8>>), BackupLoadError> {
    let extension = ".abfts";
    let session_path = backup_path.join(format!("{}", session_id));
    fs::create_dir_all(&session_path)?;
    let mut session_backups: Vec<_> = fs::read_dir(&session_path)?
        .filter_map(|r| r.ok())
        .filter_map(|x| x.file_name().into_string().ok())
        .filter_map(|s| usize::from_str(s.strip_suffix(extension)?).ok())
        .collect();
    session_backups.sort_unstable();
    if !session_backups.iter().cloned().eq(0..session_backups.len()) {
        error!(target: "aleph-party", "Session backup is not complete. Got backup for sessions {:?}", session_backups);
        return Err(BackupLoadError::BackupIncomplete(session_backups));
    }
    let mut buffer = Vec::new();
    for index in session_backups.iter() {
        let load_path = session_path.join(format!("{}{}", index, extension));
        let _ = File::open(load_path)?.read_to_end(&mut buffer)?;
    }
    let loader = Cursor::new(buffer);
    let saver = File::create(session_path.join(format!(
        "{}{}",
        session_backups.last().map_or(0, |i| i + 1),
        extension
    )))?;
    Ok((saver, loader))
}
