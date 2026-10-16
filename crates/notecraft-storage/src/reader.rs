//! A worker thread that reads page files, so the UI thread never waits on the disk
//! (docs/architecture.md section 7, "IO worker").
//!
//! Requests carry an opaque tag the caller uses to match the answer. Answers come back in
//! request order through [`PageReader::poll`], which never blocks. Dropping the reader stops
//! the thread once it finishes the read it is on.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::page_file::{read_page_file, PageReadError, PageText};

/// One finished read.
#[derive(Debug)]
pub struct ReadDone<T> {
    pub tag: T,
    pub path: PathBuf,
    pub result: Result<PageText, PageReadError>,
}

/// The reader thread's handle.
pub struct PageReader<T: Send + 'static> {
    requests: Sender<(T, PathBuf)>,
    done: Receiver<ReadDone<T>>,
}

impl<T: Send + 'static> PageReader<T> {
    /// Start the thread. `wake` is called after each read, so a UI can repaint.
    pub fn start(wake: impl Fn() + Send + 'static) -> std::io::Result<Self> {
        let (requests, request_rx) = mpsc::channel::<(T, PathBuf)>();
        let (done_tx, done) = mpsc::channel::<ReadDone<T>>();
        std::thread::Builder::new()
            .name("notecraft-page-reader".to_owned())
            .spawn(move || {
                while let Ok((tag, path)) = request_rx.recv() {
                    let result = read_page_file(&path);
                    if done_tx.send(ReadDone { tag, path, result }).is_err() {
                        return;
                    }
                    wake();
                }
            })?;
        Ok(Self { requests, done })
    }

    /// Queue a read of `path`. A reader whose thread has stopped drops the request.
    pub fn request(&self, tag: T, path: PathBuf) {
        let _ = self.requests.send((tag, path));
    }

    /// Reads finished since the last call.
    pub fn poll(&self) -> Vec<ReadDone<T>> {
        let mut out = Vec::new();
        while let Ok(done) = self.done.try_recv() {
            out.push(done);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn reads_come_back_in_order_with_their_tags() {
        let dir = tempfile::TempDir::new().unwrap();
        let a = dir.path().join("a.md");
        std::fs::write(&a, "alpha").unwrap();
        let reader = PageReader::<u32>::start(|| {}).unwrap();
        reader.request(1, a.clone());
        reader.request(2, dir.path().join("missing.md"));
        let start = Instant::now();
        let mut got = Vec::new();
        while got.len() < 2 && start.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(10));
            got.extend(reader.poll());
        }
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].tag, 1);
        assert_eq!(got[0].result.as_ref().unwrap().text, "alpha");
        assert_eq!(got[1].tag, 2);
        assert!(matches!(got[1].result, Err(PageReadError::NotFound)));
    }
}
