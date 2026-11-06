use crate::{profile, state::BrowserState};
use std::sync::mpsc::{self, SyncSender, TrySendError};

enum Write {
    Save(Vec<u8>),
    Flush(Vec<u8>, mpsc::Sender<Result<(), String>>),
}

pub struct SessionWriter(SyncSender<Write>);

impl SessionWriter {
    pub fn new(report: impl Fn(Result<(), String>) + Send + 'static) -> Self {
        Self::at(profile::directory().join("session.json"), report)
    }

    fn at(path: std::path::PathBuf, report: impl Fn(Result<(), String>) + Send + 'static) -> Self {
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut previous = std::fs::read(&path).ok().filter(|bytes| {
                std::str::from_utf8(bytes)
                    .ok()
                    .and_then(BrowserState::from_session)
                    .is_some()
            });
            while let Ok(message) = receiver.recv() {
                let (bytes, reply) = match message {
                    Write::Save(bytes) => (bytes, None),
                    Write::Flush(bytes, reply) => (bytes, Some(reply)),
                };
                let result = (|| {
                    if let Some(previous) = &previous {
                        let backup = path.with_extension("json.bak");
                        // Keep the generation before the backup so one bad write cannot lose both.
                        if let Ok(older) = std::fs::read(&backup) {
                            let _ = profile::write_atomic(&path.with_extension("json.bak2"), &older);
                        }
                        profile::write_atomic(&backup, previous)?;
                    }
                    profile::write_atomic(&path, &bytes)?;
                    previous = Some(bytes);
                    Ok::<_, std::io::Error>(())
                })()
                .map_err(|error| error.to_string());
                if let Some(reply) = reply {
                    let _ = reply.send(result);
                } else {
                    report(result);
                }
            }
        });
        Self(sender)
    }

    pub fn try_save(&self, state: &BrowserState) -> Result<bool, String> {
        let bytes = serde_json::to_vec(&*state.for_session()).map_err(|error| error.to_string())?;
        match self.0.try_send(Write::Save(bytes)) {
            Ok(()) => Ok(true),
            Err(TrySendError::Full(_)) => Ok(false),
            Err(TrySendError::Disconnected(_)) => Err("Session writer stopped".into()),
        }
    }

    pub fn flush(&self, state: &BrowserState) -> Result<(), String> {
        let bytes = serde_json::to_vec(&*state.for_session()).map_err(|error| error.to_string())?;
        let (sender, receiver) = mpsc::channel();
        self.0
            .send(Write::Flush(bytes, sender))
            .map_err(|error| error.to_string())?;
        receiver.recv().map_err(|error| error.to_string())?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flush_orders_writes_and_preserves_a_valid_backup() {
        let directory = std::env::temp_dir().join(format!("veluno-writer-{}", std::process::id()));
        let path = directory.join("session.json");
        let writer = SessionWriter::at(path.clone(), |_| {});
        let mut state = BrowserState::new();
        assert!(writer.try_save(&state).unwrap());
        let latest = state.add_tab(Some("https://example.com/last".into()), true);
        writer.flush(&state).unwrap();
        let restored =
            BrowserState::from_session(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(restored.active_tab_id, latest);
        let backup = std::fs::read_to_string(path.with_extension("json.bak")).unwrap();
        assert_eq!(BrowserState::from_session(&backup).unwrap().tabs.len(), 1);
        // A second flush rotates the first backup into the older generation.
        let newest = state.add_tab(Some("https://example.com/newest".into()), false);
        writer.flush(&state).unwrap();
        let older = std::fs::read_to_string(path.with_extension("json.bak2")).unwrap();
        assert_eq!(
            BrowserState::from_session(&older).unwrap().tabs.len(),
            1,
            "the oldest generation is still readable"
        );
        assert_eq!(
            BrowserState::from_session(
                &std::fs::read_to_string(path.with_extension("json.bak")).unwrap()
            )
            .unwrap()
            .active_tab_id,
            latest,
            "the newest backup restores the newest saved tab"
        );
        assert_eq!(
            BrowserState::from_session(&std::fs::read_to_string(&path).unwrap())
                .unwrap()
                .active_tab_id,
            newest
        );
        drop(writer);
        std::fs::remove_file(path.with_extension("json.bak2")).unwrap();
        std::fs::remove_file(path.with_extension("json.bak")).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn flush_reports_write_failures() {
        let directory =
            std::env::temp_dir().join(format!("veluno-writer-error-{}", std::process::id()));
        let path = directory.join("session.json");
        std::fs::create_dir_all(&path).unwrap();
        let writer = SessionWriter::at(path.clone(), |_| {});
        assert!(writer.flush(&BrowserState::new()).is_err());
        drop(writer);
        assert!(!profile::temporary_path(&path).exists());
        std::fs::remove_dir(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
