//! Lightweight ZMODEM transport controller for `rz`/`sz` shell workflows.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use zmodem2::{Action, Event, FileInfo, Position, Receiver, Sender};

const ZRQINIT_PREFIX: &[u8] = b"**\x18B00";
const ZRINIT_PREFIX: &[u8] = b"**\x18B01";
const ZMODEM_ZHEX_HEADER_CORE_LEN: usize = 18;
const TERMINAL_ERASE_CELL: &[u8] = b"\x08 \x08";
pub(super) const ZMODEM_ABORT_WIRE: &[u8] = b"**\x18B070000000067d4\r\n\x11";
const ZMODEM_MAX_FILE_SIZE: u64 = u32::MAX as u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZmodemTransferDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZmodemTransferPhase {
    AwaitingUploadSelection,
    AwaitingDownloadDirectory,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZmodemDownloadConflictPolicy {
    Overwrite,
    AutoRename,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZmodemTransferState {
    pub direction: ZmodemTransferDirection,
    pub phase: ZmodemTransferPhase,
    pub title: String,
    pub headline: String,
    pub status_text: String,
    pub detail_text: String,
    pub error_text: String,
    pub current_file_name: String,
    pub files_completed: usize,
    pub files_total: Option<usize>,
    pub bytes_transferred: u64,
    pub bytes_total: Option<u64>,
    pub local_file_path: Option<PathBuf>,
    pub local_reveal_path: Option<PathBuf>,
}

impl ZmodemTransferState {
    fn new(
        direction: ZmodemTransferDirection,
        phase: ZmodemTransferPhase,
        title: impl Into<String>,
        headline: impl Into<String>,
        status_text: impl Into<String>,
        detail_text: impl Into<String>,
    ) -> Self {
        Self {
            direction,
            phase,
            title: title.into(),
            headline: headline.into(),
            status_text: status_text.into(),
            detail_text: detail_text.into(),
            error_text: String::new(),
            current_file_name: String::new(),
            files_completed: 0,
            files_total: None,
            bytes_transferred: 0,
            bytes_total: None,
            local_file_path: None,
            local_reveal_path: None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ZmodemAdvanceOutcome {
    WriteWire(Vec<u8>),
    Continue,
    Idle,
}

enum SenderPollAction {
    WriteWire(Vec<u8>),
    ReadFile { offset: u64, max_len: usize },
    Event(SenderPollEvent),
    Idle,
}

enum SenderPollEvent {
    FileCompleted,
    SessionCompleted,
    Aborted,
    FileStarted,
    Other,
}

enum ReceiverPollAction {
    WriteWire(Vec<u8>),
    WriteFile(Vec<u8>),
    Event(ReceiverPollEvent),
    Idle,
}

enum ReceiverPollEvent {
    FileStarted { name: Vec<u8>, size: Option<u64> },
    FileCompleted,
    SessionCompleted,
    Aborted,
    Other,
}

#[derive(Default)]
struct TentativeTerminalBytes {
    bytes: Vec<u8>,
    visible_prefix_len: usize,
}

enum PostSessionTailStage {
    Trailer,
    OverAndOut,
}

struct PostSessionTail {
    direction: ZmodemTransferDirection,
    stage: PostSessionTailStage,
    candidate: Vec<u8>,
}

enum PostSessionTailOutcome {
    Pending,
    Released(Vec<u8>),
}

impl PostSessionTail {
    fn new(direction: ZmodemTransferDirection) -> Self {
        Self {
            direction,
            stage: PostSessionTailStage::Trailer,
            candidate: Vec::new(),
        }
    }

    fn consume(&mut self, bytes: &[u8]) -> PostSessionTailOutcome {
        let mut offset = 0;
        while offset < bytes.len() {
            self.candidate.push(bytes[offset]);
            offset += 1;

            let matches_expected = match self.stage {
                PostSessionTailStage::Trailer => {
                    matches!(self.candidate.as_slice(), [b'\r'] | [b'\r', b'\n' | 0x8a])
                }
                PostSessionTailStage::OverAndOut => {
                    matches!(self.candidate.as_slice(), [b'O'] | [b'O', b'O'])
                }
            };
            if !matches_expected {
                self.candidate.extend_from_slice(&bytes[offset..]);
                return PostSessionTailOutcome::Released(std::mem::take(&mut self.candidate));
            }

            let stage_complete = self.candidate.len() == 2;
            if !stage_complete {
                continue;
            }
            self.candidate.clear();
            match self.stage {
                PostSessionTailStage::Trailer
                    if self.direction == ZmodemTransferDirection::Download =>
                {
                    self.stage = PostSessionTailStage::OverAndOut;
                }
                PostSessionTailStage::Trailer | PostSessionTailStage::OverAndOut => {
                    return PostSessionTailOutcome::Released(bytes[offset..].to_vec());
                }
            }
        }
        PostSessionTailOutcome::Pending
    }
}

#[derive(Default)]
pub(super) struct ZmodemController {
    tentative: TentativeTerminalBytes,
    session: Option<ZmodemSession>,
    pending_control_wire: Option<Vec<u8>>,
    post_session_tail: Option<PostSessionTail>,
    released_terminal_bytes: Vec<u8>,
    automatic_rz_echo_expected: bool,
    modal_state: Option<ZmodemTransferState>,
    modal_dirty: bool,
}

impl ZmodemController {
    pub(super) fn intercept_remote_bytes(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut released_terminal_bytes = std::mem::take(&mut self.released_terminal_bytes);
        if bytes.is_empty() {
            return released_terminal_bytes;
        }

        if self.session.is_none()
            && let Some(mut tail) = self.post_session_tail.take()
        {
            match tail.consume(bytes) {
                PostSessionTailOutcome::Pending => {
                    self.post_session_tail = Some(tail);
                    return released_terminal_bytes;
                }
                PostSessionTailOutcome::Released(remaining) => {
                    if remaining.is_empty() {
                        return released_terminal_bytes;
                    }
                    released_terminal_bytes
                        .extend(self.intercept_remote_bytes(remaining.as_slice()));
                    return released_terminal_bytes;
                }
            }
        }

        if let Some(session) = self.session.as_mut() {
            if let Err(err) = session.submit_wire(bytes) {
                self.fail_session(format!("ZMODEM protocol error: {err}"));
            }
            released_terminal_bytes.extend(self.take_released_terminal_bytes());
            return released_terminal_bytes;
        }

        self.tentative.bytes.extend_from_slice(bytes);
        let mut terminal_bytes = released_terminal_bytes;
        loop {
            let Some((prefix_index, direction)) =
                find_zmodem_prefix(self.tentative.bytes.as_slice())
            else {
                let marker_suffix_len =
                    partial_marker_suffix_len(self.tentative.bytes.as_slice(), ZRQINIT_PREFIX).max(
                        partial_marker_suffix_len(self.tentative.bytes.as_slice(), ZRINIT_PREFIX),
                    );
                let automatic_echo_suffix_len = if self.automatic_rz_echo_expected {
                    automatic_rz_echo_candidate_suffix_len(self.tentative.bytes.as_slice())
                } else {
                    0
                };
                let keep_suffix_len = marker_suffix_len.max(automatic_echo_suffix_len);
                let emit_len = self.tentative.bytes.len().saturating_sub(keep_suffix_len);
                self.release_tentative_prefix(emit_len, &mut terminal_bytes);
                self.present_tentative_stars(&mut terminal_bytes);
                return terminal_bytes;
            };

            if self.tentative.bytes.len() < prefix_index.saturating_add(ZMODEM_ZHEX_HEADER_CORE_LEN)
            {
                let candidate_start = if direction == ZmodemTransferDirection::Upload
                    && self.automatic_rz_echo_expected
                {
                    automatic_rz_echo_start(&self.tentative.bytes[..prefix_index])
                        .unwrap_or(prefix_index)
                } else {
                    prefix_index
                };
                self.release_tentative_prefix(candidate_start, &mut terminal_bytes);
                self.present_tentative_stars(&mut terminal_bytes);
                return terminal_bytes;
            }

            let header_end = prefix_index + ZMODEM_ZHEX_HEADER_CORE_LEN;
            if !validate_initial_header_with_zmodem2(
                &self.tentative.bytes[prefix_index..header_end],
                direction,
            ) {
                if direction == ZmodemTransferDirection::Upload
                    && self.automatic_rz_echo_expected
                    && automatic_rz_echo_start(&self.tentative.bytes[..prefix_index]).is_some()
                {
                    self.automatic_rz_echo_expected = false;
                }
                self.release_tentative_prefix(prefix_index + 1, &mut terminal_bytes);
                continue;
            }

            let visible_header_bytes = self
                .tentative
                .visible_prefix_len
                .saturating_sub(prefix_index)
                .min(ZMODEM_ZHEX_HEADER_CORE_LEN);
            let raw_prefix = &self.tentative.bytes[..prefix_index];
            let ordinary_prefix = match direction {
                ZmodemTransferDirection::Download => {
                    strip_lrzsz_download_autostart_invocation(raw_prefix)
                }
                ZmodemTransferDirection::Upload if self.automatic_rz_echo_expected => {
                    strip_automatic_rz_echo(raw_prefix)
                }
                ZmodemTransferDirection::Upload => raw_prefix,
            };
            self.automatic_rz_echo_expected = false;
            let ordinary_prefix_len = ordinary_prefix.len();
            if ordinary_prefix_len > self.tentative.visible_prefix_len {
                terminal_bytes.extend_from_slice(
                    &self.tentative.bytes[self.tentative.visible_prefix_len..ordinary_prefix_len],
                );
            }
            for _ in 0..visible_header_bytes {
                terminal_bytes.extend_from_slice(TERMINAL_ERASE_CELL);
            }

            let protocol_bytes = self.tentative.bytes[prefix_index..].to_vec();
            self.tentative = TentativeTerminalBytes::default();
            self.session = Some(match direction {
                ZmodemTransferDirection::Upload => ZmodemSession::new_sender(),
                ZmodemTransferDirection::Download => ZmodemSession::new_receiver(),
            });
            self.set_modal_state(Some(match direction {
                ZmodemTransferDirection::Upload => ZmodemTransferState::new(
                    ZmodemTransferDirection::Upload,
                    ZmodemTransferPhase::AwaitingUploadSelection,
                    "ZMODEM Upload",
                    "Remote `rz` is waiting for files",
                    "Choose one or more local files to upload into the current shell session.",
                    "Transfer starts after you confirm the picker.",
                ),
                ZmodemTransferDirection::Download => ZmodemTransferState::new(
                    ZmodemTransferDirection::Download,
                    ZmodemTransferPhase::AwaitingDownloadDirectory,
                    "ZMODEM Download",
                    "Remote `sz` is ready to send files",
                    "Choose a local folder to receive the download.",
                    "Files will be written into the folder you pick for this transfer.",
                ),
            }));
            if let Some(session) = self.session.as_mut()
                && let Err(err) = session.submit_wire(protocol_bytes.as_slice())
            {
                self.fail_session(format!("ZMODEM protocol error: {err}"));
            }
            return terminal_bytes;
        }
    }

    pub(super) fn note_local_input(&mut self) {
        self.automatic_rz_echo_expected = false;
    }

    pub(super) fn expect_automatic_rz_echo(&mut self) {
        self.automatic_rz_echo_expected = true;
    }

    pub(super) fn cancel_automatic_rz_echo_expectation(&mut self) {
        self.automatic_rz_echo_expected = false;
    }

    pub(super) fn flush_terminal_bytes(&mut self) -> Vec<u8> {
        if self.session.is_some() {
            return self.take_released_terminal_bytes();
        }
        let mut terminal_bytes = self.take_released_terminal_bytes();
        terminal_bytes
            .extend_from_slice(&self.tentative.bytes[self.tentative.visible_prefix_len..]);
        self.tentative = TentativeTerminalBytes::default();
        terminal_bytes
    }

    pub(super) fn take_released_terminal_bytes(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.released_terminal_bytes)
    }

    pub(super) fn start_upload(&mut self, local_paths: Vec<PathBuf>) -> Result<()> {
        let Some(ZmodemSession::Sender(sender)) = self.session.as_mut() else {
            return Err(anyhow!("zmodem upload is not pending"));
        };
        sender.start(local_paths).map_err(|err| anyhow!(err))
    }

    pub(super) fn start_download(
        &mut self,
        local_dir: PathBuf,
        conflict_policy: ZmodemDownloadConflictPolicy,
    ) -> Result<()> {
        let Some(ZmodemSession::Receiver(receiver)) = self.session.as_mut() else {
            return Err(anyhow!("zmodem download is not pending"));
        };
        receiver
            .start(local_dir, conflict_policy)
            .map_err(|err| anyhow!(err))
    }

    pub(super) fn cancel(&mut self) -> Result<()> {
        let Some(mut session) = self.session.take() else {
            return Err(anyhow!("no active zmodem transfer"));
        };
        let direction = session.direction();
        session.cancel();
        self.pending_control_wire = Some(ZMODEM_ABORT_WIRE.to_vec());
        self.post_session_tail = None;
        self.automatic_rz_echo_expected = false;
        self.set_modal_state(session.state().cloned());
        tracing::info!(
            target: "app.zmodem",
            direction = ?direction,
            "zmodem transfer cancelled locally and abort frame queued"
        );
        Ok(())
    }

    pub(super) fn dismiss(&mut self) -> bool {
        if self.session.is_some() {
            return false;
        }
        if self.modal_state.is_none() {
            return false;
        }
        self.set_modal_state(None);
        true
    }

    pub(super) fn dismiss_if_matches(&mut self, expected: &ZmodemTransferState) -> bool {
        if self.current_state() != Some(expected) {
            return false;
        }
        self.dismiss()
    }

    pub(super) fn current_state(&self) -> Option<&ZmodemTransferState> {
        self.modal_state.as_ref()
    }

    pub(super) fn take_modal_state_change(&mut self) -> Option<Option<ZmodemTransferState>> {
        if !self.modal_dirty {
            return None;
        }
        self.modal_dirty = false;
        Some(self.modal_state.clone())
    }

    pub(super) fn surface_error(&mut self, error_text: impl Into<String>) {
        let error_text = error_text.into();
        if let Some(state) = self.modal_state.as_mut() {
            state.error_text = error_text;
            self.modal_dirty = true;
        }
    }

    pub(super) fn advance(&mut self) -> ZmodemAdvanceOutcome {
        if let Some(bytes) = self.pending_control_wire.take() {
            return ZmodemAdvanceOutcome::WriteWire(bytes);
        }

        let Some(mut session) = self.session.take() else {
            return ZmodemAdvanceOutcome::Idle;
        };

        let outcome = match session.advance() {
            Ok(outcome) => outcome,
            Err(err) => {
                self.fail_session(format!("ZMODEM transfer failed: {err}"));
                return ZmodemAdvanceOutcome::Continue;
            }
        };

        self.set_modal_state(session.state().cloned());
        if session.is_finished() {
            let direction = session.direction();
            let completed = session
                .state()
                .is_some_and(|state| state.phase == ZmodemTransferPhase::Completed);
            let pending_wire = session.take_pending_wire();
            if completed {
                self.begin_post_session_tail(direction, pending_wire);
            } else {
                self.route_unconsumed_terminal_bytes(pending_wire);
            }
        } else {
            self.session = Some(session);
        }

        outcome
    }

    pub(super) fn note_wire_written(&mut self, written: usize) {
        if let Some(session) = self.session.as_mut() {
            session.wire_written(written);
        }
    }

    pub(super) fn mark_transport_closed(&mut self) {
        self.automatic_rz_echo_expected = false;
        let transfer_was_active = self.session.is_some()
            || self.modal_state.as_ref().is_some_and(|state| {
                matches!(
                    state.phase,
                    ZmodemTransferPhase::AwaitingUploadSelection
                        | ZmodemTransferPhase::AwaitingDownloadDirectory
                        | ZmodemTransferPhase::Running
                )
            });
        if transfer_was_active {
            self.fail_session("The SSH channel closed before the ZMODEM transfer finished.");
        }
        self.post_session_tail = None;
    }

    fn fail_session(&mut self, error_text: impl Into<String>) {
        let error_text = error_text.into();
        let state = self.modal_state.clone().or_else(|| {
            self.session
                .as_ref()
                .and_then(|session| session.state().cloned())
        });
        if let Some(mut state) = state {
            state.phase = ZmodemTransferPhase::Failed;
            state.headline = match state.direction {
                ZmodemTransferDirection::Upload => "Upload failed".into(),
                ZmodemTransferDirection::Download => "Download failed".into(),
            };
            state.error_text = error_text;
            self.session = None;
            self.post_session_tail = None;
            self.set_modal_state(Some(state));
        } else {
            self.session = None;
            self.post_session_tail = None;
        }
    }

    fn set_modal_state(&mut self, state: Option<ZmodemTransferState>) {
        if self.modal_state != state {
            self.modal_state = state;
            self.modal_dirty = true;
        }
    }

    fn release_tentative_prefix(&mut self, len: usize, terminal_bytes: &mut Vec<u8>) {
        let len = len.min(self.tentative.bytes.len());
        let already_visible = self.tentative.visible_prefix_len.min(len);
        terminal_bytes.extend_from_slice(&self.tentative.bytes[already_visible..len]);
        self.tentative.bytes.drain(..len);
        self.tentative.visible_prefix_len = self.tentative.visible_prefix_len.saturating_sub(len);
    }

    fn present_tentative_stars(&mut self, terminal_bytes: &mut Vec<u8>) {
        let star_count = self
            .tentative
            .bytes
            .iter()
            .take_while(|byte| **byte == b'*')
            .count();
        if star_count <= self.tentative.visible_prefix_len {
            return;
        }
        terminal_bytes.extend_from_slice(
            &self.tentative.bytes[self.tentative.visible_prefix_len..star_count],
        );
        self.tentative.visible_prefix_len = star_count;
    }

    fn begin_post_session_tail(
        &mut self,
        direction: ZmodemTransferDirection,
        pending_wire: Vec<u8>,
    ) {
        let mut tail = PostSessionTail::new(direction);
        match tail.consume(pending_wire.as_slice()) {
            PostSessionTailOutcome::Pending => {
                self.post_session_tail = Some(tail);
            }
            PostSessionTailOutcome::Released(remaining) => {
                self.post_session_tail = None;
                self.route_unconsumed_terminal_bytes(remaining);
            }
        }
    }

    fn route_unconsumed_terminal_bytes(&mut self, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }
        let mut released = std::mem::take(&mut self.released_terminal_bytes);
        released.extend(self.intercept_remote_bytes(bytes.as_slice()));
        self.released_terminal_bytes = released;
    }
}

enum ZmodemSession {
    Sender(SenderTransfer),
    Receiver(ReceiverTransfer),
}

impl ZmodemSession {
    fn new_sender() -> Self {
        Self::Sender(SenderTransfer::new())
    }

    fn new_receiver() -> Self {
        Self::Receiver(ReceiverTransfer::new())
    }

    fn submit_wire(&mut self, bytes: &[u8]) -> Result<()> {
        match self {
            Self::Sender(sender) => sender.submit_wire(bytes),
            Self::Receiver(receiver) => receiver.submit_wire(bytes),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Sender(sender) => sender.cancel(),
            Self::Receiver(receiver) => receiver.cancel(),
        }
    }

    fn direction(&self) -> ZmodemTransferDirection {
        match self {
            Self::Sender(_) => ZmodemTransferDirection::Upload,
            Self::Receiver(_) => ZmodemTransferDirection::Download,
        }
    }

    fn advance(&mut self) -> Result<ZmodemAdvanceOutcome> {
        match self {
            Self::Sender(sender) => sender.advance(),
            Self::Receiver(receiver) => receiver.advance(),
        }
    }

    fn wire_written(&mut self, written: usize) {
        match self {
            Self::Sender(sender) => sender.protocol.wire_written(written),
            Self::Receiver(receiver) => receiver.protocol.wire_written(written),
        }
    }

    fn state(&self) -> Option<&ZmodemTransferState> {
        match self {
            Self::Sender(sender) => sender.state.as_ref(),
            Self::Receiver(receiver) => receiver.state.as_ref(),
        }
    }

    fn is_finished(&self) -> bool {
        match self {
            Self::Sender(sender) => sender.finished,
            Self::Receiver(receiver) => receiver.finished,
        }
    }

    fn take_pending_wire(&mut self) -> Vec<u8> {
        match self {
            Self::Sender(sender) => sender.take_pending_wire(),
            Self::Receiver(receiver) => receiver.take_pending_wire(),
        }
    }
}

#[derive(Debug)]
struct SenderFileEntry {
    path: PathBuf,
    file_name: Vec<u8>,
    display_name: String,
    size_bytes: u64,
}

struct SenderTransfer {
    protocol: Sender,
    pending_wire: Vec<u8>,
    files: Vec<SenderFileEntry>,
    current_index: usize,
    current_file: Option<File>,
    bytes_transferred: u64,
    total_bytes: u64,
    session_complete_pending: bool,
    finished: bool,
    state: Option<ZmodemTransferState>,
}

impl SenderTransfer {
    fn new() -> Self {
        Self {
            protocol: Sender::new().expect("create zmodem sender"),
            pending_wire: Vec::new(),
            files: Vec::new(),
            current_index: 0,
            current_file: None,
            bytes_transferred: 0,
            total_bytes: 0,
            session_complete_pending: false,
            finished: false,
            state: Some(ZmodemTransferState::new(
                ZmodemTransferDirection::Upload,
                ZmodemTransferPhase::AwaitingUploadSelection,
                "ZMODEM Upload",
                "Remote `rz` is waiting for files",
                "Choose one or more local files to upload into the current shell session.",
                "Transfer starts after you confirm the picker.",
            )),
        }
    }

    fn start(&mut self, local_paths: Vec<PathBuf>) -> Result<()> {
        if self.finished {
            return Err(anyhow!("zmodem upload is already finished"));
        }

        let files = local_paths
            .into_iter()
            .map(|path| build_sender_file_entry(path.as_path()))
            .collect::<Result<Vec<_>>>()?;
        if files.is_empty() {
            return Err(anyhow!("no files were selected"));
        }

        self.total_bytes = files.iter().map(|file| file.size_bytes).sum();
        self.files = files;
        self.current_index = 0;
        self.bytes_transferred = 0;
        self.start_current_file()?;
        self.sync_running_state("Preparing upload");
        Ok(())
    }

    fn submit_wire(&mut self, bytes: &[u8]) -> Result<()> {
        let protocol = &mut self.protocol;
        buffer_protocol_wire(&mut self.pending_wire, bytes, |chunk| {
            protocol.submit_wire(chunk).map_err(Into::into)
        })
    }

    fn take_pending_wire(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pending_wire)
    }

    fn cancel(&mut self) {
        self.protocol.abort();
        self.pending_wire.clear();
        self.current_file = None;
        self.session_complete_pending = false;
        self.finished = true;
        let mut state = self.state.clone().unwrap_or_else(|| {
            ZmodemTransferState::new(
                ZmodemTransferDirection::Upload,
                ZmodemTransferPhase::Cancelled,
                "ZMODEM Upload",
                "Upload cancelled",
                "The transfer was cancelled.",
                "",
            )
        });
        state.phase = ZmodemTransferPhase::Cancelled;
        state.headline = "Upload cancelled".into();
        state.status_text = "The transfer was cancelled before all files were sent.".into();
        state.detail_text = "The remote shell was told to abort the ZMODEM session.".into();
        state.error_text.clear();
        state.current_file_name.clear();
        state.files_completed = self.current_index.min(self.files.len());
        state.files_total = (!self.files.is_empty()).then_some(self.files.len());
        state.bytes_transferred = self.bytes_transferred.min(self.total_bytes);
        state.bytes_total = (self.total_bytes > 0).then_some(self.total_bytes);
        state.local_file_path = None;
        state.local_reveal_path = None;
        self.state = Some(state);
    }

    fn advance(&mut self) -> Result<ZmodemAdvanceOutcome> {
        if self.finished {
            return Ok(ZmodemAdvanceOutcome::Idle);
        }

        let protocol = &mut self.protocol;
        flush_pending_wire(&mut self.pending_wire, |chunk| {
            protocol.submit_wire(chunk).map_err(Into::into)
        })?;

        let action = match self.protocol.poll() {
            Action::WriteWire(bytes) => SenderPollAction::WriteWire(bytes.to_vec()),
            Action::ReadFile { offset, max_len } => SenderPollAction::ReadFile {
                offset: u64::from(offset.get()),
                max_len,
            },
            Action::Event(event) => SenderPollAction::Event(match event {
                Event::FileCompleted => SenderPollEvent::FileCompleted,
                Event::SessionCompleted => SenderPollEvent::SessionCompleted,
                Event::Aborted => SenderPollEvent::Aborted,
                Event::FileStarted(_) => SenderPollEvent::FileStarted,
                _ => SenderPollEvent::Other,
            }),
            Action::Idle | Action::WriteFile(_) => SenderPollAction::Idle,
            _ => SenderPollAction::Idle,
        };

        match action {
            SenderPollAction::WriteWire(bytes) => Ok(ZmodemAdvanceOutcome::WriteWire(bytes)),
            SenderPollAction::ReadFile { offset, max_len } => {
                let current_file = self
                    .current_file
                    .as_mut()
                    .ok_or_else(|| anyhow!("zmodem sender requested file data without a file"))?;
                let mut buffer = vec![0u8; max_len.max(1)];
                current_file
                    .seek(SeekFrom::Start(offset))
                    .context("seek local upload file")?;
                let read = current_file
                    .read(buffer.as_mut_slice())
                    .context("read local upload file")?;
                if read == 0 {
                    return Err(anyhow!(
                        "unexpected end of local file `{}`",
                        self.current_file_name()
                    ));
                }
                buffer.truncate(read);
                self.protocol.submit_file(buffer.as_slice())?;
                let next_bytes = offset + read as u64;
                let completed_before = self.completed_bytes_before_current_file();
                self.bytes_transferred =
                    completed_before + next_bytes.min(self.current_file_size_bytes());
                self.sync_running_state("Uploading");
                Ok(ZmodemAdvanceOutcome::Continue)
            }
            SenderPollAction::Event(event) => {
                self.handle_event(event)?;
                Ok(ZmodemAdvanceOutcome::Continue)
            }
            SenderPollAction::Idle => {
                if self.session_complete_pending {
                    self.session_complete_pending = false;
                    self.complete_session();
                    self.finished = true;
                }
                Ok(ZmodemAdvanceOutcome::Idle)
            }
        }
    }

    fn handle_event(&mut self, event: SenderPollEvent) -> Result<()> {
        match event {
            SenderPollEvent::FileCompleted => {
                self.current_file = None;
                self.current_index = self.current_index.saturating_add(1);
                self.bytes_transferred = self.completed_bytes_before_current_file();
                if self.current_index < self.files.len() {
                    self.start_current_file()?;
                    self.sync_running_state("Uploading");
                } else {
                    self.protocol.finish()?;
                    self.sync_running_state("Finalizing upload");
                }
            }
            SenderPollEvent::SessionCompleted => {
                self.session_complete_pending = true;
            }
            SenderPollEvent::Aborted => {
                self.finished = true;
                if let Some(state) = self.state.as_mut() {
                    state.phase = ZmodemTransferPhase::Cancelled;
                    state.headline = "Upload cancelled".into();
                    state.status_text =
                        "The remote shell aborted the transfer or the transfer was cancelled."
                            .into();
                    state.detail_text = "No more files will be sent.".into();
                    state.error_text.clear();
                }
            }
            SenderPollEvent::FileStarted | SenderPollEvent::Other => {}
        }
        Ok(())
    }

    fn complete_session(&mut self) {
        let mut state = self.state.clone().unwrap_or_else(|| {
            ZmodemTransferState::new(
                ZmodemTransferDirection::Upload,
                ZmodemTransferPhase::Completed,
                "ZMODEM Upload",
                "Upload complete",
                "All selected files were transferred.",
                "",
            )
        });
        state.phase = ZmodemTransferPhase::Completed;
        state.headline = "Upload complete".into();
        state.status_text = format!("Transferred {} file(s) with ZMODEM.", self.files.len());
        state.detail_text = "The remote shell can continue using the received files.".into();
        state.error_text.clear();
        state.current_file_name.clear();
        state.files_completed = self.files.len();
        state.files_total = Some(self.files.len());
        state.bytes_transferred = self.total_bytes;
        state.bytes_total = Some(self.total_bytes);
        self.state = Some(state);
    }

    fn start_current_file(&mut self) -> Result<()> {
        let entry = self
            .files
            .get(self.current_index)
            .ok_or_else(|| anyhow!("no current zmodem upload file"))?;
        let size = u32::try_from(entry.size_bytes).context("file exceeds ZMODEM size limit")?;
        self.protocol.start_file(FileInfo::new(
            entry.file_name.as_slice(),
            Some(Position::new(size)),
        ))?;
        self.current_file = Some(
            File::open(entry.path.as_path())
                .with_context(|| format!("open local upload file `{}`", entry.path.display()))?,
        );
        Ok(())
    }

    fn completed_bytes_before_current_file(&self) -> u64 {
        self.files
            .iter()
            .take(self.current_index.min(self.files.len()))
            .map(|file| file.size_bytes)
            .sum()
    }

    fn current_file_size_bytes(&self) -> u64 {
        self.files
            .get(self.current_index)
            .map(|file| file.size_bytes)
            .unwrap_or(0)
    }

    fn current_file_name(&self) -> String {
        self.files
            .get(self.current_index)
            .map(|file| file.display_name.clone())
            .unwrap_or_default()
    }

    fn sync_running_state(&mut self, status_text: &str) {
        let current_file_name = self.current_file_name();
        let detail_text = if current_file_name.is_empty() {
            String::new()
        } else {
            format!(
                "File {} of {}",
                self.current_index.saturating_add(1),
                self.files.len()
            )
        };
        let mut state = self.state.clone().unwrap_or_else(|| {
            ZmodemTransferState::new(
                ZmodemTransferDirection::Upload,
                ZmodemTransferPhase::Running,
                "ZMODEM Upload",
                "Uploading files",
                status_text,
                detail_text.clone(),
            )
        });
        state.phase = ZmodemTransferPhase::Running;
        state.title = "ZMODEM Upload".into();
        state.headline = "Uploading files".into();
        state.status_text = status_text.into();
        state.detail_text = detail_text;
        state.error_text.clear();
        state.current_file_name = current_file_name;
        state.files_completed = self.current_index.min(self.files.len());
        state.files_total = Some(self.files.len());
        state.bytes_transferred = self.bytes_transferred.min(self.total_bytes);
        state.bytes_total = Some(self.total_bytes);
        self.state = Some(state);
    }
}

struct ReceiverTransfer {
    protocol: Receiver,
    target_dir: Option<PathBuf>,
    conflict_policy: ZmodemDownloadConflictPolicy,
    cancel_requested: bool,
    pending_wire: Vec<u8>,
    deferred_event: Option<ReceiverPollEvent>,
    current_file: Option<File>,
    current_target_path: Option<PathBuf>,
    current_file_wire_name: Option<Vec<u8>>,
    last_completed_file_wire_name: Option<Vec<u8>>,
    last_completed_file_size: Option<u64>,
    current_file_name: String,
    current_file_size: Option<u64>,
    completed_local_paths: Vec<PathBuf>,
    files_started: usize,
    files_completed: usize,
    bytes_transferred: u64,
    bytes_total: u64,
    session_complete_pending: bool,
    finished: bool,
    state: Option<ZmodemTransferState>,
}

impl ReceiverTransfer {
    fn new() -> Self {
        Self {
            protocol: Receiver::new().expect("create zmodem receiver"),
            target_dir: None,
            conflict_policy: ZmodemDownloadConflictPolicy::AutoRename,
            cancel_requested: false,
            pending_wire: Vec::new(),
            deferred_event: None,
            current_file: None,
            current_target_path: None,
            current_file_wire_name: None,
            last_completed_file_wire_name: None,
            last_completed_file_size: None,
            current_file_name: String::new(),
            current_file_size: None,
            completed_local_paths: Vec::new(),
            files_started: 0,
            files_completed: 0,
            bytes_transferred: 0,
            bytes_total: 0,
            session_complete_pending: false,
            finished: false,
            state: Some(ZmodemTransferState::new(
                ZmodemTransferDirection::Download,
                ZmodemTransferPhase::AwaitingDownloadDirectory,
                "ZMODEM Download",
                "Remote `sz` is ready to send files",
                "Choose a local folder to receive the download.",
                "Files will be written into the folder you pick for this transfer.",
            )),
        }
    }

    fn start(
        &mut self,
        local_dir: PathBuf,
        conflict_policy: ZmodemDownloadConflictPolicy,
    ) -> Result<()> {
        fs::create_dir_all(local_dir.as_path())
            .with_context(|| format!("create download directory `{}`", local_dir.display()))?;
        self.target_dir = Some(local_dir);
        self.conflict_policy = conflict_policy;
        self.cancel_requested = false;
        self.sync_running_state("Waiting for remote files");
        Ok(())
    }

    fn submit_wire(&mut self, bytes: &[u8]) -> Result<()> {
        let protocol = &mut self.protocol;
        buffer_protocol_wire(&mut self.pending_wire, bytes, |chunk| {
            protocol.submit_wire(chunk).map_err(Into::into)
        })
    }

    fn take_pending_wire(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pending_wire)
    }

    fn cancel(&mut self) {
        self.cancel_requested = true;
        self.deferred_event = None;
        self.pending_wire.clear();
        let _ = self.protocol.abort();
        self.current_file = None;
        self.current_target_path = None;
        self.current_file_wire_name = None;
        self.last_completed_file_wire_name = None;
        self.last_completed_file_size = None;
        self.current_file_name.clear();
        self.current_file_size = None;
        self.session_complete_pending = false;
        self.finished = true;
        let mut state = self.state.clone().unwrap_or_else(|| {
            ZmodemTransferState::new(
                ZmodemTransferDirection::Download,
                ZmodemTransferPhase::Cancelled,
                "ZMODEM Download",
                "Download cancelled",
                "The transfer was cancelled.",
                "",
            )
        });
        state.phase = ZmodemTransferPhase::Cancelled;
        state.headline = "Download cancelled".into();
        state.status_text = "The transfer was cancelled before all files were received.".into();
        state.detail_text = "The remote shell was told to abort the ZMODEM session.".into();
        state.error_text.clear();
        state.current_file_name.clear();
        state.files_completed = self.files_completed;
        state.files_total = (self.files_started > 0).then_some(self.files_started);
        state.bytes_transferred = self.bytes_transferred;
        state.bytes_total = (self.bytes_total > 0).then_some(self.bytes_total);
        state.local_file_path = None;
        state.local_reveal_path = None;
        self.state = Some(state);
    }

    fn advance(&mut self) -> Result<ZmodemAdvanceOutcome> {
        if self.finished {
            return Ok(ZmodemAdvanceOutcome::Idle);
        }

        if self.target_dir.is_none()
            && !self.cancel_requested
            && matches!(
                self.deferred_event,
                Some(ReceiverPollEvent::FileStarted { .. })
            )
        {
            return Ok(ZmodemAdvanceOutcome::Idle);
        }

        if let Some(event) = self.deferred_event.take() {
            self.handle_event(event)?;
            return Ok(ZmodemAdvanceOutcome::Continue);
        }

        let protocol = &mut self.protocol;
        flush_pending_wire(&mut self.pending_wire, |chunk| {
            protocol.submit_wire(chunk).map_err(Into::into)
        })?;

        let action = match self.protocol.poll() {
            Action::WriteWire(bytes) => ReceiverPollAction::WriteWire(bytes.to_vec()),
            Action::WriteFile(bytes) => ReceiverPollAction::WriteFile(bytes.to_vec()),
            Action::Event(event) => ReceiverPollAction::Event(match event {
                Event::FileStarted(info) => ReceiverPollEvent::FileStarted {
                    name: info.name.to_vec(),
                    size: info.size.map(|size| u64::from(size.get())),
                },
                Event::FileCompleted => ReceiverPollEvent::FileCompleted,
                Event::SessionCompleted => ReceiverPollEvent::SessionCompleted,
                Event::Aborted => ReceiverPollEvent::Aborted,
                _ => ReceiverPollEvent::Other,
            }),
            Action::Idle | Action::ReadFile { .. } => ReceiverPollAction::Idle,
            _ => ReceiverPollAction::Idle,
        };

        match action {
            ReceiverPollAction::WriteWire(bytes) => Ok(ZmodemAdvanceOutcome::WriteWire(bytes)),
            ReceiverPollAction::WriteFile(bytes) => {
                let file = self.current_file.as_mut().ok_or_else(|| {
                    anyhow!("zmodem receiver requested file writes without a file")
                })?;
                file.write_all(bytes.as_slice())
                    .context("write local download file")?;
                self.protocol.file_written(bytes.len())?;
                self.bytes_transferred = self.bytes_transferred.saturating_add(bytes.len() as u64);
                self.sync_running_state("Receiving files");
                Ok(ZmodemAdvanceOutcome::Continue)
            }
            ReceiverPollAction::Event(event) => {
                if self.target_dir.is_none() {
                    if self.cancel_requested
                        && matches!(event, ReceiverPollEvent::FileStarted { .. })
                    {
                        return Ok(ZmodemAdvanceOutcome::Continue);
                    }
                    if matches!(event, ReceiverPollEvent::FileStarted { .. }) {
                        self.deferred_event = Some(event);
                        return Ok(ZmodemAdvanceOutcome::Idle);
                    }
                }
                self.handle_event(event)?;
                Ok(ZmodemAdvanceOutcome::Continue)
            }
            ReceiverPollAction::Idle => {
                if self.session_complete_pending {
                    self.session_complete_pending = false;
                    self.complete_session();
                    self.finished = true;
                }
                Ok(ZmodemAdvanceOutcome::Idle)
            }
        }
    }

    fn handle_event(&mut self, event: ReceiverPollEvent) -> Result<()> {
        match event {
            ReceiverPollEvent::FileStarted { name, size } => {
                if self.current_file.is_some() {
                    if self
                        .current_file_wire_name
                        .as_ref()
                        .is_some_and(|current| current.as_slice() == name.as_slice())
                        && self.current_file_size == size
                    {
                        tracing::debug!(
                            target: "app.zmodem",
                            file_name = %String::from_utf8_lossy(name.as_slice()),
                            "ignored duplicate zmodem file-start event for active download"
                        );
                        return Ok(());
                    }
                    return Err(anyhow!(
                        "zmodem receiver reported a new file before the previous file completed"
                    ));
                }

                if self
                    .last_completed_file_wire_name
                    .as_ref()
                    .is_some_and(|current| current.as_slice() == name.as_slice())
                    && self.last_completed_file_size == size
                    && self.files_completed == self.files_started
                {
                    tracing::info!(
                        target: "app.zmodem",
                        file_name = %String::from_utf8_lossy(name.as_slice()),
                        "ignored duplicate zmodem file-start event for completed download"
                    );
                    return Ok(());
                }

                let target_dir = self
                    .target_dir
                    .clone()
                    .ok_or_else(|| anyhow!("download target directory is not selected"))?;
                let target_path = resolve_receiver_target_path(
                    target_dir.as_path(),
                    name.as_slice(),
                    size,
                    self.conflict_policy,
                )?;
                tracing::info!(
                    target: "app.zmodem",
                    local_path = %target_path.display(),
                    conflict_policy = ?self.conflict_policy,
                    "zmodem download target selected"
                );
                let display_name = target_path
                    .file_name()
                    .map(|value| value.to_string_lossy().to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "download".into());
                let file = File::create(target_path.as_path()).with_context(|| {
                    format!("create local download file `{}`", target_path.display())
                })?;
                self.current_file = Some(file);
                self.current_target_path = Some(target_path);
                self.current_file_wire_name = Some(name);
                self.current_file_name = display_name;
                self.current_file_size = size;
                self.files_started = self.files_started.saturating_add(1);
                if let Some(current_file_size) = self.current_file_size {
                    self.bytes_total = self.bytes_total.saturating_add(current_file_size);
                }
                self.sync_running_state("Receiving files");
            }
            ReceiverPollEvent::FileCompleted => {
                self.current_file = None;
                if let Some(path) = self.current_target_path.take() {
                    self.completed_local_paths.push(path);
                }
                self.last_completed_file_wire_name = self.current_file_wire_name.take();
                self.last_completed_file_size = self.current_file_size;
                self.current_file_name.clear();
                self.current_file_size = None;
                self.files_completed = self.files_completed.saturating_add(1);
                self.sync_running_state("Waiting for next file");
            }
            ReceiverPollEvent::SessionCompleted => {
                self.cancel_requested = false;
                self.session_complete_pending = true;
            }
            ReceiverPollEvent::Aborted => {
                self.cancel_requested = false;
                self.finished = true;
                if let Some(state) = self.state.as_mut() {
                    state.phase = ZmodemTransferPhase::Cancelled;
                    state.headline = "Download cancelled".into();
                    state.status_text =
                        "The remote shell aborted the transfer or the transfer was cancelled."
                            .into();
                    state.detail_text = "No more files will be received.".into();
                    state.error_text.clear();
                }
            }
            ReceiverPollEvent::Other => {}
        }
        Ok(())
    }

    fn complete_session(&mut self) {
        let mut state = self.state.clone().unwrap_or_else(|| {
            ZmodemTransferState::new(
                ZmodemTransferDirection::Download,
                ZmodemTransferPhase::Completed,
                "ZMODEM Download",
                "Download complete",
                "All remote files were received.",
                "",
            )
        });
        state.phase = ZmodemTransferPhase::Completed;
        state.headline = "Download complete".into();
        state.status_text = format!("Received {} file(s) with ZMODEM.", self.files_completed);
        state.detail_text = "The downloaded files are available in the folder you selected.".into();
        state.error_text.clear();
        state.current_file_name.clear();
        state.files_completed = self.files_completed;
        state.files_total = Some(self.files_completed);
        state.bytes_transferred = self.bytes_transferred;
        state.bytes_total = (self.bytes_total > 0).then_some(self.bytes_total);
        state.local_file_path = if self.completed_local_paths.len() == 1 {
            self.completed_local_paths.first().cloned()
        } else {
            None
        };
        state.local_reveal_path = self
            .completed_local_paths
            .first()
            .cloned()
            .or_else(|| self.target_dir.clone());
        self.state = Some(state);
        tracing::info!(
            target: "app.zmodem",
            files_completed = self.files_completed,
            bytes_transferred = self.bytes_transferred,
            "zmodem download completed after final wire drain"
        );
    }

    fn sync_running_state(&mut self, status_text: &str) {
        let detail_text = if self.files_started == 0 {
            "Waiting for the remote sender to begin streaming data.".into()
        } else if self.current_file_name.is_empty() {
            format!("{} file(s) received so far.", self.files_completed)
        } else {
            format!(
                "Receiving file {} of at least {}",
                self.files_completed.saturating_add(1),
                self.files_started
            )
        };
        let mut state = self.state.clone().unwrap_or_else(|| {
            ZmodemTransferState::new(
                ZmodemTransferDirection::Download,
                ZmodemTransferPhase::Running,
                "ZMODEM Download",
                "Receiving files",
                status_text,
                detail_text.clone(),
            )
        });
        state.phase = ZmodemTransferPhase::Running;
        state.title = "ZMODEM Download".into();
        state.headline = "Receiving files".into();
        state.status_text = status_text.into();
        state.detail_text = detail_text;
        state.error_text.clear();
        state.current_file_name = self.current_file_name.clone();
        state.files_completed = self.files_completed;
        state.files_total = (self.files_started > 0).then_some(self.files_started);
        state.bytes_transferred = self.bytes_transferred;
        state.bytes_total = (self.bytes_total > 0).then_some(self.bytes_total);
        self.state = Some(state);
    }
}

fn build_sender_file_entry(path: &Path) -> Result<SenderFileEntry> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("inspect local upload path `{}`", path.display()))?;
    if !metadata.is_file() {
        return Err(anyhow!(
            "ZMODEM upload currently supports files only: `{}`",
            path.display()
        ));
    }

    let display_name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("selected file is missing a visible name"))?;
    if metadata.len() > ZMODEM_MAX_FILE_SIZE {
        return Err(anyhow!(
            "ZMODEM upload does not support files larger than 4 GiB right now: `{}`",
            path.display()
        ));
    }

    Ok(SenderFileEntry {
        path: path.to_path_buf(),
        file_name: display_name.as_bytes().to_vec(),
        display_name,
        size_bytes: metadata.len(),
    })
}

fn buffer_protocol_wire<F>(pending_wire: &mut Vec<u8>, bytes: &[u8], submit: F) -> Result<()>
where
    F: FnMut(&[u8]) -> Result<usize>,
{
    pending_wire.extend_from_slice(bytes);
    flush_pending_wire(pending_wire, submit)
}

fn flush_pending_wire<F>(pending_wire: &mut Vec<u8>, mut submit: F) -> Result<()>
where
    F: FnMut(&[u8]) -> Result<usize>,
{
    let mut consumed_total = 0;
    while consumed_total < pending_wire.len() {
        let consumed = submit(&pending_wire[consumed_total..])?;
        if consumed == 0 {
            break;
        }
        consumed_total += consumed;
    }
    if consumed_total > 0 {
        pending_wire.drain(..consumed_total);
    }
    Ok(())
}

fn resolve_receiver_target_path(
    target_dir: &Path,
    file_name: &[u8],
    _size: Option<u64>,
    conflict_policy: ZmodemDownloadConflictPolicy,
) -> Result<PathBuf> {
    let file_name = sanitize_zmodem_file_name(file_name);
    let mut candidate = target_dir.join(file_name);
    if !candidate.exists() {
        return Ok(candidate);
    }

    match conflict_policy {
        ZmodemDownloadConflictPolicy::Overwrite => Ok(candidate),
        ZmodemDownloadConflictPolicy::AutoRename => {
            candidate = crate::app::sftp::local_ops::next_auto_rename_path(candidate.as_path());
            Ok(candidate)
        }
    }
}

fn strip_lrzsz_download_autostart_invocation(bytes: &[u8]) -> &[u8] {
    for marker in [b"rz\r\n".as_slice(), b"rz\r".as_slice(), b"rz\n".as_slice()] {
        if let Some(stripped) = bytes.strip_suffix(marker) {
            return stripped;
        }
    }
    bytes
}

fn strip_automatic_rz_echo(bytes: &[u8]) -> &[u8] {
    automatic_rz_echo_start(bytes)
        .map(|start| &bytes[..start])
        .unwrap_or(bytes)
}

fn automatic_rz_echo_start(bytes: &[u8]) -> Option<usize> {
    automatic_rz_echo_markers()
        .iter()
        .find_map(|marker| bytes.strip_suffix(*marker).map(|prefix| prefix.len()))
}

fn automatic_rz_echo_candidate_suffix_len(bytes: &[u8]) -> usize {
    (1..=bytes.len())
        .rev()
        .find(|suffix_len| {
            let suffix = &bytes[bytes.len() - suffix_len..];
            automatic_rz_echo_markers().iter().any(|marker| {
                marker.starts_with(suffix)
                    || suffix.strip_prefix(*marker).is_some_and(|after_echo| {
                        after_echo.len() < ZRINIT_PREFIX.len()
                            && ZRINIT_PREFIX.starts_with(after_echo)
                    })
            })
        })
        .unwrap_or(0)
}

fn automatic_rz_echo_markers() -> [&'static [u8]; 6] {
    [
        b" rz -q\r\n",
        b" rz -q\r",
        b" rz -q\n",
        b" rz\r\n",
        b" rz\r",
        b" rz\n",
    ]
}

fn sanitize_zmodem_file_name(raw_name: &[u8]) -> String {
    let lossy = String::from_utf8_lossy(raw_name);
    let normalized = lossy.replace('\\', "/");
    let base_name = normalized
        .rsplit('/')
        .next()
        .map(str::trim)
        .filter(|segment| !segment.is_empty() && *segment != "." && *segment != "..")
        .unwrap_or("download");
    base_name.to_string()
}

fn find_zmodem_prefix(bytes: &[u8]) -> Option<(usize, ZmodemTransferDirection)> {
    let upload = find_subsequence(bytes, ZRINIT_PREFIX)
        .map(|index| (index, ZmodemTransferDirection::Upload));
    let download = find_subsequence(bytes, ZRQINIT_PREFIX)
        .map(|index| (index, ZmodemTransferDirection::Download));
    match (upload, download) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn validate_initial_header_with_zmodem2(header: &[u8], direction: ZmodemTransferDirection) -> bool {
    if header.len() != ZMODEM_ZHEX_HEADER_CORE_LEN {
        return false;
    }

    match direction {
        ZmodemTransferDirection::Upload => {
            let Ok(mut sender) = Sender::new() else {
                return false;
            };
            while let Action::WriteWire(bytes) = sender.poll() {
                let written = bytes.len();
                sender.wire_written(written);
            }
            sender
                .submit_wire(header)
                .is_ok_and(|consumed| consumed == header.len())
        }
        ZmodemTransferDirection::Download => {
            let Ok(mut receiver) = Receiver::new() else {
                return false;
            };
            while let Action::WriteWire(bytes) = receiver.poll() {
                let written = bytes.len();
                receiver.wire_written(written);
            }
            receiver
                .submit_wire(header)
                .is_ok_and(|consumed| consumed == header.len())
        }
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn partial_marker_suffix_len(bytes: &[u8], marker: &[u8]) -> usize {
    let max_suffix = bytes.len().min(marker.len().saturating_sub(1));
    (1..=max_suffix)
        .rev()
        .find(|&suffix_len| bytes.ends_with(&marker[..suffix_len]))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain_sender_wire(sender: &mut Sender) {
        while let Action::WriteWire(bytes) = sender.poll() {
            let written = bytes.len();
            sender.wire_written(written);
        }
    }

    fn seed_pending_receiver_transfer() -> ReceiverTransfer {
        let mut transfer = ReceiverTransfer::new();
        let zrinit = match transfer.advance().expect("emit initial zrinit") {
            ZmodemAdvanceOutcome::WriteWire(bytes) => {
                let written = bytes.len();
                transfer.protocol.wire_written(written);
                bytes
            }
            other => panic!("unexpected initial receiver advance: {other:?}"),
        };

        let mut sender = Sender::new().expect("create sender");
        drain_sender_wire(&mut sender);
        sender
            .start_file(FileInfo::new(
                b"queued.bin",
                Some(Position::new(transfer_size_bytes())),
            ))
            .expect("queue sender file");
        assert!(
            sender
                .submit_wire(zrinit.as_slice())
                .expect("submit zrinit")
                > 0
        );

        let mut file_offer = Vec::new();
        loop {
            match sender.poll() {
                Action::WriteWire(bytes) => {
                    file_offer.extend_from_slice(bytes);
                    let written = bytes.len();
                    sender.wire_written(written);
                }
                Action::Idle => break,
                other => panic!("unexpected sender action while building file offer: {other:?}"),
            }
        }
        assert!(!file_offer.is_empty(), "sender did not emit a zfile offer");
        transfer
            .submit_wire(file_offer.as_slice())
            .expect("submit remote file offer");
        transfer
    }

    fn transfer_size_bytes() -> u32 {
        123
    }

    fn initial_header(direction: ZmodemTransferDirection) -> Vec<u8> {
        let wire = match direction {
            ZmodemTransferDirection::Upload => {
                let mut receiver = Receiver::new().expect("create receiver");
                match receiver.poll() {
                    Action::WriteWire(bytes) => bytes.to_vec(),
                    other => panic!("unexpected receiver initialization action: {other:?}"),
                }
            }
            ZmodemTransferDirection::Download => {
                let mut sender = Sender::new().expect("create sender");
                match sender.poll() {
                    Action::WriteWire(bytes) => bytes.to_vec(),
                    other => panic!("unexpected sender initialization action: {other:?}"),
                }
            }
        };
        assert!(wire.len() >= 18, "initial ZHEX header was too short");
        wire[..18].to_vec()
    }

    #[test]
    fn finds_both_known_zmodem_prefixes() {
        assert_eq!(
            find_zmodem_prefix(b"hello**\x18B01rest"),
            Some((5, ZmodemTransferDirection::Upload))
        );
        assert_eq!(
            find_zmodem_prefix(b"hello**\x18B00rest"),
            Some((5, ZmodemTransferDirection::Download))
        );
    }

    #[test]
    fn preserves_partial_prefix_suffixes_between_batches() {
        let mut controller = ZmodemController::default();

        let first = controller.intercept_remote_bytes(b"plain**\x18");
        assert_eq!(first, b"plain**".to_vec());
        assert_eq!(controller.flush_terminal_bytes(), b"\x18".to_vec());
    }

    #[test]
    fn ordinary_star_candidates_are_visible_without_waiting_for_enter() {
        for input in [
            b"*".as_slice(),
            b"**",
            b"***",
            b"*.log",
            b"a*b",
            b"'*'",
            b"\\*",
            b"paste:**",
        ] {
            let mut controller = ZmodemController::default();

            assert_eq!(
                controller.intercept_remote_bytes(input),
                input,
                "ordinary star bytes were not visible immediately for {input:?}"
            );
            assert!(controller.flush_terminal_bytes().is_empty());
        }
    }

    #[test]
    fn local_input_never_discards_tentative_remote_bytes() {
        let input = b"**\x18B0";
        let mut controller = ZmodemController::default();
        let mut terminal_bytes = controller.intercept_remote_bytes(input);

        controller.note_local_input();
        terminal_bytes.extend(controller.flush_terminal_bytes());

        assert_eq!(terminal_bytes, input);
    }

    #[test]
    fn partial_and_false_initial_markers_replay_exactly_once() {
        for marker in [ZRQINIT_PREFIX, ZRINIT_PREFIX] {
            for prefix_len in 1..=marker.len() {
                let mut input = marker[..prefix_len].to_vec();
                input.push(b'x');
                let mut controller = ZmodemController::default();
                let mut terminal_bytes = controller.intercept_remote_bytes(&input[..prefix_len]);

                controller.note_local_input();
                terminal_bytes.extend(controller.intercept_remote_bytes(&input[prefix_len..]));
                terminal_bytes.extend(controller.flush_terminal_bytes());

                assert_eq!(
                    terminal_bytes, input,
                    "false marker changed at prefix length {prefix_len} for {marker:?}"
                );
                assert!(controller.session.is_none());
            }
        }
    }

    #[test]
    fn transport_close_flushes_unseen_tentative_remote_bytes() {
        let input = b"**\x18B0";
        let mut controller = ZmodemController::default();
        let mut terminal_bytes = controller.intercept_remote_bytes(input);

        controller.mark_transport_closed();
        terminal_bytes.extend(controller.flush_terminal_bytes());

        assert_eq!(terminal_bytes, input);
    }

    #[test]
    fn split_valid_initial_headers_require_complete_crc() {
        for direction in [
            ZmodemTransferDirection::Upload,
            ZmodemTransferDirection::Download,
        ] {
            let header = initial_header(direction);
            for split in 0..header.len() {
                let mut controller = ZmodemController::default();

                let mut terminal_bytes = controller.intercept_remote_bytes(&header[..split]);
                assert!(
                    controller.session.is_none(),
                    "{direction:?} session started before the complete header at split {split}"
                );

                terminal_bytes.extend(controller.intercept_remote_bytes(&header[split..]));
                assert!(
                    controller.session.is_some(),
                    "{direction:?} session was not detected at split {split}"
                );
                let provisional_stars = split.min(2);
                let mut expected_terminal_bytes = vec![b'*'; provisional_stars];
                for _ in 0..provisional_stars {
                    expected_terminal_bytes.extend_from_slice(TERMINAL_ERASE_CELL);
                }
                assert_eq!(terminal_bytes, expected_terminal_bytes);
            }
        }
    }

    #[test]
    fn invalid_initial_header_replays_every_byte() {
        for direction in [
            ZmodemTransferDirection::Upload,
            ZmodemTransferDirection::Download,
        ] {
            let mut header = initial_header(direction);
            let crc_nibble = header.last_mut().expect("header crc nibble");
            *crc_nibble = if *crc_nibble == b'0' { b'1' } else { b'0' };

            let mut controller = ZmodemController::default();
            let split = header.len() / 2;
            let mut terminal_bytes = controller.intercept_remote_bytes(&header[..split]);
            controller.note_local_input();
            terminal_bytes.extend(controller.intercept_remote_bytes(&header[split..]));
            terminal_bytes.extend(controller.flush_terminal_bytes());

            assert_eq!(terminal_bytes, header);
            assert!(controller.session.is_none());
        }
    }

    #[test]
    fn ordinary_star_before_later_valid_header_is_not_erased() {
        let header = initial_header(ZmodemTransferDirection::Upload);
        let mut controller = ZmodemController::default();

        let first = controller.intercept_remote_bytes(b"***");
        let second = controller.intercept_remote_bytes(&header[2..]);

        let mut expected_second = Vec::new();
        expected_second.extend_from_slice(TERMINAL_ERASE_CELL);
        expected_second.extend_from_slice(TERMINAL_ERASE_CELL);
        assert_eq!(first, b"***");
        assert_eq!(second, expected_second);
        assert!(controller.session.is_some());
    }

    #[test]
    fn sanitizes_remote_names_to_safe_basename() {
        assert_eq!(sanitize_zmodem_file_name(b"../etc/passwd"), "passwd");
        assert_eq!(sanitize_zmodem_file_name(b"nested/file.txt"), "file.txt");
        assert_eq!(sanitize_zmodem_file_name(b""), "download");
    }

    #[test]
    fn buffered_protocol_wire_retains_unconsumed_tail() {
        let mut pending_wire = Vec::new();
        let mut first_call = true;
        buffer_protocol_wire(&mut pending_wire, b"abcdef", |chunk| {
            if first_call {
                first_call = false;
                assert_eq!(chunk, b"abcdef");
                Ok(3)
            } else {
                Ok(0)
            }
        })
        .expect("buffer protocol wire");
        assert_eq!(pending_wire, b"def");

        flush_pending_wire(&mut pending_wire, |chunk| Ok(chunk.len())).expect("flush pending wire");
        assert!(pending_wire.is_empty());
    }

    #[test]
    fn strips_lrzsz_autostart_invocation_before_download_prefix() {
        let mut controller = ZmodemController::default();
        let mut remote_bytes = b"prompt\nrz\r".to_vec();
        remote_bytes.extend(initial_header(ZmodemTransferDirection::Download));

        let terminal_bytes = controller.intercept_remote_bytes(remote_bytes.as_slice());

        assert_eq!(terminal_bytes, b"prompt\n".to_vec());
        let state = controller
            .take_modal_state_change()
            .expect("zmodem state changed")
            .expect("download state");
        assert_eq!(state.direction, ZmodemTransferDirection::Download);
        assert_eq!(state.phase, ZmodemTransferPhase::AwaitingDownloadDirectory);
    }

    #[test]
    fn automatic_quiet_rz_echo_requires_explicit_ownership() {
        let mut remote_bytes = b"prompt\n rz -q\r".to_vec();
        remote_bytes.extend(initial_header(ZmodemTransferDirection::Upload));
        for split in 0..remote_bytes.len() {
            let mut controller = ZmodemController::default();
            controller.expect_automatic_rz_echo();
            let mut terminal_bytes = controller.intercept_remote_bytes(&remote_bytes[..split]);
            terminal_bytes.extend(controller.intercept_remote_bytes(&remote_bytes[split..]));

            assert_eq!(terminal_bytes, b"prompt\n".to_vec(), "split {split}");
            let state = controller
                .take_modal_state_change()
                .expect("zmodem state changed")
                .expect("upload state");
            assert_eq!(state.direction, ZmodemTransferDirection::Upload);
            assert_eq!(state.phase, ZmodemTransferPhase::AwaitingUploadSelection);
        }

        let mut controller = ZmodemController::default();
        let mut manual_remote_bytes = b"prompt\n rz\r".to_vec();
        manual_remote_bytes.extend(initial_header(ZmodemTransferDirection::Upload));

        assert_eq!(
            controller.intercept_remote_bytes(manual_remote_bytes.as_slice()),
            b"prompt\n rz\r".to_vec()
        );
    }

    #[test]
    fn post_session_tail_consumes_only_direction_expected_bytes() {
        let mut upload = PostSessionTail::new(ZmodemTransferDirection::Upload);
        match upload.consume(b"\r\nroot@host:~# ") {
            PostSessionTailOutcome::Released(bytes) => assert_eq!(bytes, b"root@host:~# "),
            PostSessionTailOutcome::Pending => panic!("upload tail remained pending"),
        }

        let mut upload = PostSessionTail::new(ZmodemTransferDirection::Upload);
        match upload.consume(b"OO-service# ") {
            PostSessionTailOutcome::Released(bytes) => assert_eq!(bytes, b"OO-service# "),
            PostSessionTailOutcome::Pending => panic!("mismatched upload tail remained pending"),
        }

        let mut download = PostSessionTail::new(ZmodemTransferDirection::Download);
        assert!(matches!(
            download.consume(b"\r\nO"),
            PostSessionTailOutcome::Pending
        ));
        match download.consume(b"Oroot@host:~# ") {
            PostSessionTailOutcome::Released(bytes) => assert_eq!(bytes, b"root@host:~# "),
            PostSessionTailOutcome::Pending => panic!("download tail remained pending"),
        }

        let mut download = PostSessionTail::new(ZmodemTransferDirection::Download);
        match download.consume(b"\r\nOx-service# ") {
            PostSessionTailOutcome::Released(bytes) => assert_eq!(bytes, b"Ox-service# "),
            PostSessionTailOutcome::Pending => panic!("mismatched download tail remained pending"),
        }
    }

    #[test]
    fn controller_post_session_tail_releases_plain_prompt() {
        let mut controller = ZmodemController {
            post_session_tail: Some(PostSessionTail::new(ZmodemTransferDirection::Download)),
            ..ZmodemController::default()
        };

        assert!(controller.intercept_remote_bytes(b"\r\nO").is_empty());
        assert_eq!(
            controller.intercept_remote_bytes(b"Oroot@host:~# "),
            b"root@host:~# ".to_vec()
        );
    }

    #[test]
    fn post_session_upload_prompt_starting_with_oo_is_preserved() {
        let mut controller = ZmodemController {
            post_session_tail: Some(PostSessionTail::new(ZmodemTransferDirection::Upload)),
            ..ZmodemController::default()
        };

        assert_eq!(
            controller.intercept_remote_bytes(b"OO-service# "),
            b"OO-service# ".to_vec()
        );
    }

    #[test]
    fn completed_sender_releases_same_chunk_prompt() {
        for prompt in [
            b"root@host:~# ".as_slice(),
            b"OO-service# ",
            b"rz-admin# ",
            b"* prompt",
        ] {
            let mut sender = SenderTransfer::new();
            sender.finished = true;
            sender.state.as_mut().expect("sender state").phase = ZmodemTransferPhase::Completed;
            sender.pending_wire.extend_from_slice(b"\r\n");
            sender.pending_wire.extend_from_slice(prompt);
            let mut controller = ZmodemController {
                session: Some(ZmodemSession::Sender(sender)),
                ..ZmodemController::default()
            };

            assert_eq!(controller.advance(), ZmodemAdvanceOutcome::Idle);
            assert_eq!(controller.take_released_terminal_bytes(), prompt);
        }
    }

    #[test]
    fn completed_receiver_consumes_only_expected_oo_then_releases_prompt() {
        for prompt in [
            b"root@host:~# ".as_slice(),
            b"OO-service# ",
            b"rz-admin# ",
            b"* prompt",
        ] {
            let mut receiver = ReceiverTransfer::new();
            receiver.finished = true;
            receiver.state.as_mut().expect("receiver state").phase = ZmodemTransferPhase::Completed;
            receiver.pending_wire.extend_from_slice(b"\r\nOO");
            receiver.pending_wire.extend_from_slice(prompt);
            let mut controller = ZmodemController {
                session: Some(ZmodemSession::Receiver(receiver)),
                ..ZmodemController::default()
            };

            assert_eq!(controller.advance(), ZmodemAdvanceOutcome::Idle);
            assert_eq!(controller.take_released_terminal_bytes(), prompt);
        }
    }

    #[test]
    fn sender_session_completed_waits_for_final_wire_drain() {
        let mut transfer = SenderTransfer::new();
        drain_sender_wire(&mut transfer.protocol);

        transfer
            .handle_event(SenderPollEvent::SessionCompleted)
            .expect("handle session completion");

        assert!(!transfer.finished);
        assert!(transfer.session_complete_pending);
        assert_ne!(
            transfer.state.as_ref().expect("sender state").phase,
            ZmodemTransferPhase::Completed,
            "upload completion must wait until final wire bytes are drained"
        );
        assert_eq!(
            transfer.advance().expect("finish after idle"),
            ZmodemAdvanceOutcome::Idle
        );
        assert!(transfer.finished);
        assert_eq!(
            transfer.state.as_ref().expect("sender state").phase,
            ZmodemTransferPhase::Completed
        );
    }

    #[test]
    fn receiver_session_completed_waits_for_final_wire_drain() {
        let mut transfer = ReceiverTransfer::new();
        if let ZmodemAdvanceOutcome::WriteWire(bytes) =
            transfer.advance().expect("drain initial zrinit")
        {
            transfer.protocol.wire_written(bytes.len());
        }

        transfer
            .handle_event(ReceiverPollEvent::SessionCompleted)
            .expect("handle session completion");

        assert!(!transfer.finished);
        assert!(transfer.session_complete_pending);
        assert_ne!(
            transfer.state.as_ref().expect("receiver state").phase,
            ZmodemTransferPhase::Completed,
            "download completion must wait until final wire bytes are drained"
        );
        assert_eq!(
            transfer.advance().expect("finish after idle"),
            ZmodemAdvanceOutcome::Idle
        );
        assert!(transfer.finished);
        assert_eq!(
            transfer.state.as_ref().expect("receiver state").phase,
            ZmodemTransferPhase::Completed
        );
    }

    #[test]
    fn receiver_waits_for_directory_before_consuming_file_offer() {
        let mut transfer = seed_pending_receiver_transfer();

        assert_eq!(
            transfer.advance().expect("hold pending download"),
            ZmodemAdvanceOutcome::Idle
        );
        assert_eq!(
            transfer.state.as_ref().expect("receiver state").phase,
            ZmodemTransferPhase::AwaitingDownloadDirectory
        );
        assert!(!transfer.finished);
        assert!(matches!(
            transfer.deferred_event,
            Some(ReceiverPollEvent::FileStarted { .. })
        ));
    }

    #[test]
    fn receiver_resumes_pending_file_offer_after_directory_selection() {
        let mut transfer = seed_pending_receiver_transfer();
        let local_dir =
            std::env::temp_dir().join(format!("mica-term-zmodem-{}", uuid::Uuid::new_v4()));
        transfer
            .start(local_dir.clone(), ZmodemDownloadConflictPolicy::AutoRename)
            .expect("start download");

        assert_eq!(
            transfer.advance().expect("resume pending download"),
            ZmodemAdvanceOutcome::Continue
        );
        assert_eq!(
            transfer.state.as_ref().expect("receiver state").phase,
            ZmodemTransferPhase::Running
        );
        assert_eq!(transfer.current_file_name, "queued.bin");

        drop(transfer);
        let _ = fs::remove_dir_all(local_dir);
    }

    #[test]
    fn receiver_ignores_duplicate_file_started_for_active_download() {
        let mut transfer = seed_pending_receiver_transfer();
        let local_dir =
            std::env::temp_dir().join(format!("mica-term-zmodem-{}", uuid::Uuid::new_v4()));
        transfer
            .start(local_dir.clone(), ZmodemDownloadConflictPolicy::Overwrite)
            .expect("start download");

        assert_eq!(
            transfer.advance().expect("handle deferred file start"),
            ZmodemAdvanceOutcome::Continue
        );
        assert_eq!(transfer.files_started, 1);
        let first_target = transfer.current_target_path.clone();

        transfer
            .handle_event(ReceiverPollEvent::FileStarted {
                name: b"queued.bin".to_vec(),
                size: Some(transfer_size_bytes().into()),
            })
            .expect("ignore duplicate file start");
        assert_eq!(
            transfer.files_started, 1,
            "a repeated FileStarted event from zmodem2 must not create a second local target"
        );
        assert_eq!(transfer.current_target_path, first_target);

        drop(transfer);
        let _ = fs::remove_dir_all(local_dir);
    }

    #[test]
    fn receiver_ignores_duplicate_file_started_after_file_completed() {
        let mut transfer = ReceiverTransfer::new();
        let local_dir =
            std::env::temp_dir().join(format!("mica-term-zmodem-{}", uuid::Uuid::new_v4()));
        transfer
            .start(local_dir.clone(), ZmodemDownloadConflictPolicy::Overwrite)
            .expect("start download");

        transfer
            .handle_event(ReceiverPollEvent::FileStarted {
                name: b"queued.bin".to_vec(),
                size: Some(transfer_size_bytes().into()),
            })
            .expect("start file");
        transfer
            .handle_event(ReceiverPollEvent::FileCompleted)
            .expect("complete file");
        transfer
            .handle_event(ReceiverPollEvent::FileStarted {
                name: b"queued.bin".to_vec(),
                size: Some(transfer_size_bytes().into()),
            })
            .expect("ignore duplicate after completion");

        assert_eq!(transfer.files_started, 1);
        assert_eq!(transfer.files_completed, 1);
        assert_eq!(transfer.completed_local_paths.len(), 1);
        assert!(transfer.current_file.is_none());

        drop(transfer);
        let _ = fs::remove_dir_all(local_dir);
    }

    #[test]
    fn receiver_overwrite_conflict_reuses_existing_download_path() {
        let mut transfer = seed_pending_receiver_transfer();
        let local_dir =
            std::env::temp_dir().join(format!("mica-term-zmodem-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(local_dir.as_path()).expect("create zmodem temp dir");
        let existing_path = local_dir.join("queued.bin");
        fs::write(existing_path.as_path(), b"old").expect("seed existing download");

        transfer
            .start(local_dir.clone(), ZmodemDownloadConflictPolicy::Overwrite)
            .expect("start download");
        assert_eq!(
            transfer.advance().expect("resume pending download"),
            ZmodemAdvanceOutcome::Continue
        );

        assert_eq!(
            transfer.current_target_path.as_deref(),
            Some(existing_path.as_path())
        );
        assert_eq!(
            fs::metadata(existing_path.as_path())
                .expect("read overwritten metadata")
                .len(),
            0
        );

        drop(transfer);
        let _ = fs::remove_dir_all(local_dir);
    }

    #[test]
    fn receiver_can_cancel_while_waiting_for_directory() {
        let mut transfer = seed_pending_receiver_transfer();
        transfer.cancel();

        assert_eq!(
            transfer.advance().expect("cancel pending receiver"),
            ZmodemAdvanceOutcome::Idle
        );
        assert_eq!(
            transfer.state.as_ref().expect("receiver state").phase,
            ZmodemTransferPhase::Cancelled
        );
        assert!(transfer.finished);
    }

    #[test]
    fn controller_cancel_queues_abort_wire_and_finishes_locally() {
        let mut controller = ZmodemController::default();
        let header = initial_header(ZmodemTransferDirection::Download);

        assert!(
            controller
                .intercept_remote_bytes(header.as_slice())
                .is_empty()
        );
        controller.cancel().expect("cancel zmodem controller");

        let state = controller
            .take_modal_state_change()
            .expect("modal state dirty after cancel")
            .expect("cancelled state");
        assert_eq!(state.phase, ZmodemTransferPhase::Cancelled);

        assert_eq!(
            controller.advance(),
            ZmodemAdvanceOutcome::WriteWire(ZMODEM_ABORT_WIRE.to_vec())
        );
        assert_eq!(controller.advance(), ZmodemAdvanceOutcome::Idle);
        assert!(controller.dismiss());
    }

    #[test]
    fn controller_dismiss_if_matches_never_clears_different_state() {
        let mut controller = ZmodemController::default();
        let header = initial_header(ZmodemTransferDirection::Download);
        assert!(
            controller
                .intercept_remote_bytes(header.as_slice())
                .is_empty()
        );
        let actual = controller.current_state().expect("current state").clone();
        let mut different = actual.clone();
        different.headline = "Different transfer".into();

        assert!(!controller.dismiss_if_matches(&different));
        assert_eq!(controller.current_state(), Some(&actual));
    }
}
