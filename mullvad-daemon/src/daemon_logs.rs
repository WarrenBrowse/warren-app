//! The daemon's own log files, served to the problem report collector.
//!
//! The log directory is readable by root only, since the logs narrate the owner's use of the
//! VPN. The collector runs as the user who files the report, so it reads them through
//! `GetDaemonLogs`, which the access table keeps for the wallet owner and administrators.

use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

/// The most of each file served, its end: the collector's own cap.
pub const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;

/// The end of one log file.
#[derive(Debug, PartialEq, Eq)]
pub struct LogTail {
    pub name: String,
    pub content: Vec<u8>,
    pub size: u64,
}

/// The `.log` files of `dir`, each cut to its last `max_bytes`, sorted by name.
pub fn read(dir: &Path, max_bytes: u64) -> io::Result<Vec<LogTail>> {
    let mut tails = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_none_or(|extension| extension != "log") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let mut file = File::open(&path)?;
        let size = file.metadata()?.len();
        if size > max_bytes {
            file.seek(SeekFrom::Start(size - max_bytes))?;
        }
        let mut content = Vec::new();
        file.take(max_bytes).read_to_end(&mut content)?;
        tails.push(LogTail {
            name: name.to_owned(),
            content,
            size,
        });
    }
    tails.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(tails)
}

/// The largest piece of a file one message carries, well under a client's 4 MiB decoding limit.
pub const CHUNK_BYTES: usize = 1024 * 1024;

/// One message of `GetDaemonLogs`: `name`, the whole file's `size`, and the next piece.
#[derive(Debug, PartialEq, Eq)]
pub struct LogChunk {
    pub name: String,
    pub size: u64,
    pub content: Vec<u8>,
}

/// `tails` as the messages that carry them, each file in order and as one empty chunk when it
/// has no content.
pub fn chunks(tails: Vec<LogTail>, chunk_bytes: usize) -> Vec<LogChunk> {
    let mut out = Vec::new();
    for tail in tails {
        if tail.content.is_empty() {
            out.push(LogChunk {
                name: tail.name,
                size: tail.size,
                content: Vec::new(),
            });
            continue;
        }
        for piece in tail.content.chunks(chunk_bytes) {
            out.push(LogChunk {
                name: tail.name.clone(),
                size: tail.size,
                content: piece.to_vec(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_cut_into_pieces_no_message_exceeds() {
        let tails = vec![
            LogTail {
                name: "daemon.log".to_owned(),
                content: b"abcde".to_vec(),
                size: 9,
            },
            LogTail {
                name: "empty.log".to_owned(),
                content: vec![],
                size: 0,
            },
        ];

        let chunks = chunks(tails, 2);

        let pieces: Vec<(&str, &[u8])> = chunks
            .iter()
            .map(|chunk| (chunk.name.as_str(), chunk.content.as_slice()))
            .collect();
        assert_eq!(
            pieces,
            vec![
                ("daemon.log", &b"ab"[..]),
                ("daemon.log", &b"cd"[..]),
                ("daemon.log", &b"e"[..]),
                ("empty.log", &b""[..]),
            ]
        );
        assert!(chunks[..3].iter().all(|chunk| chunk.size == 9));
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("warren-daemon-logs-{tag}-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn every_log_file_is_served_and_nothing_else() {
        let dir = scratch("files");
        std::fs::write(dir.join("daemon.log"), b"started").unwrap();
        std::fs::write(dir.join("early-boot-fw.log"), b"blocked").unwrap();
        std::fs::write(dir.join("settings.json"), b"{}").unwrap();

        let tails = read(&dir, MAX_LOG_BYTES).unwrap();

        assert_eq!(
            tails,
            vec![
                LogTail {
                    name: "daemon.log".to_owned(),
                    content: b"started".to_vec(),
                    size: 7
                },
                LogTail {
                    name: "early-boot-fw.log".to_owned(),
                    content: b"blocked".to_vec(),
                    size: 7
                },
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_long_log_is_served_by_its_end() {
        let dir = scratch("tail");
        std::fs::write(dir.join("daemon.log"), b"0123456789").unwrap();

        let tails = read(&dir, 4).unwrap();

        assert_eq!(tails[0].content, b"6789");
        assert_eq!(tails[0].size, 10);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
