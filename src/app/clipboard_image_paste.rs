use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::app::clipboard::{ClipboardImagePreview, EncodedClipboardImage};

pub(crate) const CLIPBOARD_IMAGE_PASTE_QUEUE_CAPACITY: usize = 8;
pub(crate) const CLIPBOARD_IMAGE_SUCCESS_LIFETIME: Duration = Duration::from_millis(3_200);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClipboardImagePasteRegisterError {
    QueueFull,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClipboardImagePathAction {
    pub request_id: Uuid,
    pub session_id: Uuid,
    pub binding_id: Uuid,
    pub remote_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClipboardImageCompletion {
    AutoInsert(ClipboardImagePathAction),
    Stale,
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClipboardImageBindingContext {
    pub request_id: Uuid,
    pub session_id: Uuid,
    pub binding_id: Uuid,
}

pub(crate) struct ClipboardImageUploadJob<R> {
    pub request_id: Uuid,
    pub session_id: Uuid,
    pub binding_id: Uuid,
    pub runtime: R,
    pub png_bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub encoded_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClipboardImagePasteProjection {
    pub request_id: Uuid,
    pub status: &'static str,
    pub preview: Option<ClipboardImagePreview>,
    pub source_width: u32,
    pub source_height: u32,
    pub detail: String,
    pub paste_enabled: bool,
    pub copy_enabled: bool,
    pub collapsed: bool,
    pub bytes_transferred: u64,
    pub bytes_total: u64,
    pub bytes_per_second: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardImagePasteState {
    Preparing,
    Queued,
    Uploading,
    AwaitingInsert,
    Success { expires_at: Instant },
    Stale,
    Error,
    Tombstone,
}

struct ClipboardImagePasteRequest<R> {
    request_id: Uuid,
    session_id: Uuid,
    binding_id: Uuid,
    captured_input_epoch: u64,
    runtime: Option<R>,
    png_bytes: Option<Vec<u8>>,
    preview: Option<ClipboardImagePreview>,
    source_width: u32,
    source_height: u32,
    remote_path: Option<String>,
    detail: String,
    dismissed: bool,
    state: ClipboardImagePasteState,
    bytes_transferred: u64,
    bytes_total: u64,
    bytes_per_second: u64,
}

pub(crate) struct ClipboardImagePasteController<R> {
    input_epochs: HashMap<Uuid, u64>,
    requests: VecDeque<ClipboardImagePasteRequest<R>>,
    active_upload_id: Option<Uuid>,
    revision: u64,
}

impl<R> Default for ClipboardImagePasteController<R> {
    fn default() -> Self {
        Self {
            input_epochs: HashMap::new(),
            requests: VecDeque::new(),
            active_upload_id: None,
            revision: 0,
        }
    }
}

impl<R> ClipboardImagePasteController<R> {
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn register(
        &mut self,
        session_id: Uuid,
        binding_id: Uuid,
        runtime: R,
    ) -> Result<Uuid, ClipboardImagePasteRegisterError> {
        if self.retained_request_count() >= CLIPBOARD_IMAGE_PASTE_QUEUE_CAPACITY {
            return Err(ClipboardImagePasteRegisterError::QueueFull);
        }

        let request_id = Uuid::new_v4();
        let captured_input_epoch = *self.input_epochs.entry(session_id).or_default();
        self.requests.push_back(ClipboardImagePasteRequest {
            request_id,
            session_id,
            binding_id,
            captured_input_epoch,
            runtime: Some(runtime),
            png_bytes: None,
            preview: None,
            source_width: 0,
            source_height: 0,
            remote_path: None,
            detail: "Preparing image".into(),
            dismissed: false,
            state: ClipboardImagePasteState::Preparing,
            bytes_transferred: 0,
            bytes_total: 0,
            bytes_per_second: 0,
        });
        self.bump_revision();
        Ok(request_id)
    }

    pub(crate) fn note_terminal_input(&mut self, session_id: Uuid) -> u64 {
        let epoch = self.input_epochs.entry(session_id).or_default();
        *epoch = epoch.wrapping_add(1);
        *epoch
    }

    pub(crate) fn mark_prepared(
        &mut self,
        request_id: Uuid,
        encoded: EncodedClipboardImage,
    ) -> bool {
        let Some(request) = self.request_mut(request_id) else {
            return false;
        };
        if request.state != ClipboardImagePasteState::Preparing {
            return false;
        }

        let EncodedClipboardImage {
            png_bytes,
            width,
            height,
            preview,
        } = encoded;
        request.bytes_total = png_bytes.len() as u64;
        request.png_bytes = Some(png_bytes);
        request.source_width = width;
        request.source_height = height;
        if !request.dismissed {
            request.preview = Some(preview);
        }
        request.detail = "Waiting to upload".into();
        request.state = ClipboardImagePasteState::Queued;
        self.bump_revision();
        true
    }

    pub(crate) fn mark_preparation_failed(&mut self, request_id: Uuid, error: String) -> bool {
        let Some(request) = self.request_mut(request_id) else {
            return false;
        };
        if request.state != ClipboardImagePasteState::Preparing {
            return false;
        }
        transition_to_error(request, error);
        self.bump_revision();
        true
    }

    pub(crate) fn take_next_upload(&mut self) -> Option<ClipboardImageUploadJob<R>> {
        if self.active_upload_id.is_some() {
            return None;
        }

        let mut queued_index = None;
        for (index, request) in self.requests.iter().enumerate() {
            match request.state {
                ClipboardImagePasteState::Success { .. }
                | ClipboardImagePasteState::Stale
                | ClipboardImagePasteState::Error
                | ClipboardImagePasteState::Tombstone => {}
                ClipboardImagePasteState::Queued => {
                    queued_index = Some(index);
                    break;
                }
                ClipboardImagePasteState::Preparing
                | ClipboardImagePasteState::Uploading
                | ClipboardImagePasteState::AwaitingInsert => return None,
            }
        }

        let index = queued_index?;
        let request = self
            .requests
            .get_mut(index)
            .expect("queued clipboard image request index must remain valid");
        let Some(runtime) = request.runtime.take() else {
            transition_to_error(
                request,
                "Clipboard image upload runtime is unavailable".into(),
            );
            self.bump_revision();
            return None;
        };
        let Some(png_bytes) = request.png_bytes.take() else {
            request.runtime = Some(runtime);
            transition_to_error(
                request,
                "Prepared clipboard image data is unavailable".into(),
            );
            self.bump_revision();
            return None;
        };

        let encoded_bytes = png_bytes.len();
        request.state = ClipboardImagePasteState::Uploading;
        request.detail = "Uploading image".into();
        self.active_upload_id = Some(request.request_id);
        let job = ClipboardImageUploadJob {
            request_id: request.request_id,
            session_id: request.session_id,
            binding_id: request.binding_id,
            runtime,
            png_bytes,
            width: request.source_width,
            height: request.source_height,
            encoded_bytes,
        };
        self.bump_revision();
        Some(job)
    }

    pub(crate) fn mark_upload_progress(
        &mut self,
        request_id: Uuid,
        bytes_transferred: u64,
        bytes_total: u64,
        elapsed: Duration,
    ) -> bool {
        if self.active_upload_id != Some(request_id) {
            return false;
        }
        let Some(index) = self.request_index(request_id) else {
            return false;
        };
        let request = &mut self.requests[index];
        if request.state != ClipboardImagePasteState::Uploading
            || request.bytes_total != bytes_total
        {
            return false;
        }

        let bytes_transferred = bytes_transferred.min(bytes_total);
        if bytes_transferred < request.bytes_transferred {
            return false;
        }
        let bytes_per_second = average_bytes_per_second(bytes_transferred, elapsed);
        if request.bytes_transferred == bytes_transferred
            && request.bytes_per_second == bytes_per_second
        {
            return false;
        }

        request.bytes_transferred = bytes_transferred;
        request.bytes_per_second = bytes_per_second;
        self.bump_revision();
        true
    }

    pub(crate) fn mark_upload_succeeded(
        &mut self,
        request_id: Uuid,
        remote_path: String,
    ) -> ClipboardImageCompletion {
        if self.active_upload_id != Some(request_id) {
            return ClipboardImageCompletion::Ignored;
        }
        self.active_upload_id = None;

        let Some(index) = self.request_index(request_id) else {
            self.bump_revision();
            return ClipboardImageCompletion::Ignored;
        };
        if self.requests[index].state == ClipboardImagePasteState::Tombstone {
            self.requests.remove(index);
            self.bump_revision();
            return ClipboardImageCompletion::Ignored;
        }
        if self.requests[index].state != ClipboardImagePasteState::Uploading {
            self.bump_revision();
            return ClipboardImageCompletion::Ignored;
        }

        let current_epoch = self
            .input_epochs
            .get(&self.requests[index].session_id)
            .copied()
            .unwrap_or_default();
        let request = &mut self.requests[index];
        request.remote_path = Some(remote_path.clone());
        let completion = if current_epoch == request.captured_input_epoch {
            request.state = ClipboardImagePasteState::AwaitingInsert;
            request.detail = "Upload complete".into();
            ClipboardImageCompletion::AutoInsert(ClipboardImagePathAction {
                request_id,
                session_id: request.session_id,
                binding_id: request.binding_id,
                remote_path,
            })
        } else {
            request.state = ClipboardImagePasteState::Stale;
            request.dismissed = false;
            request.detail = "Terminal input changed; choose where to use the uploaded path".into();
            ClipboardImageCompletion::Stale
        };
        self.bump_revision();
        completion
    }

    pub(crate) fn active_upload_binding_context(
        &self,
        request_id: Uuid,
    ) -> Option<ClipboardImageBindingContext> {
        let request = self.request(request_id)?;
        (self.active_upload_id == Some(request_id)
            && request.state == ClipboardImagePasteState::Uploading)
            .then_some(ClipboardImageBindingContext {
                request_id,
                session_id: request.session_id,
                binding_id: request.binding_id,
            })
    }

    pub(crate) fn stale_binding_contexts(&self) -> Vec<ClipboardImageBindingContext> {
        self.requests
            .iter()
            .filter(|request| request.state == ClipboardImagePasteState::Stale)
            .map(|request| ClipboardImageBindingContext {
                request_id: request.request_id,
                session_id: request.session_id,
                binding_id: request.binding_id,
            })
            .collect()
    }

    pub(crate) fn mark_upload_failed(&mut self, request_id: Uuid, error: String) -> bool {
        if self.active_upload_id != Some(request_id) {
            return false;
        }
        self.active_upload_id = None;
        let Some(index) = self.request_index(request_id) else {
            self.bump_revision();
            return false;
        };
        if self.requests[index].state == ClipboardImagePasteState::Tombstone {
            self.requests.remove(index);
            self.bump_revision();
            return false;
        }
        if self.requests[index].state != ClipboardImagePasteState::Uploading {
            self.bump_revision();
            return false;
        }
        transition_to_error(&mut self.requests[index], error);
        self.bump_revision();
        true
    }

    pub(crate) fn mark_inserted(&mut self, request_id: Uuid, now: Instant) -> bool {
        let Some(request) = self.request_mut(request_id) else {
            return false;
        };
        if !matches!(
            request.state,
            ClipboardImagePasteState::AwaitingInsert | ClipboardImagePasteState::Stale
        ) {
            return false;
        }
        request.preview = None;
        request.remote_path = None;
        request.detail = "Path pasted".into();
        request.state = ClipboardImagePasteState::Success {
            expires_at: now + CLIPBOARD_IMAGE_SUCCESS_LIFETIME,
        };
        self.bump_revision();
        true
    }

    pub(crate) fn mark_connection_invalid(&mut self, request_id: Uuid, error: String) -> bool {
        let Some(index) = self.request_index(request_id) else {
            return false;
        };
        if self.requests[index].state == ClipboardImagePasteState::Tombstone {
            if self.active_upload_id == Some(request_id) {
                self.active_upload_id = None;
            }
            self.requests.remove(index);
            self.bump_revision();
            return true;
        }
        if matches!(
            self.requests[index].state,
            ClipboardImagePasteState::Success { .. } | ClipboardImagePasteState::Error
        ) {
            return false;
        }
        if self.active_upload_id == Some(request_id) {
            self.active_upload_id = None;
        }
        transition_to_error(&mut self.requests[index], error);
        self.bump_revision();
        true
    }

    pub(crate) fn stale_paste_action(
        &self,
        request_id: Uuid,
        active_session_id: Uuid,
    ) -> Option<ClipboardImagePathAction> {
        let request = self.request(request_id)?;
        if request.state != ClipboardImagePasteState::Stale
            || request.session_id != active_session_id
        {
            return None;
        }
        Some(ClipboardImagePathAction {
            request_id,
            session_id: request.session_id,
            binding_id: request.binding_id,
            remote_path: request.remote_path.clone()?,
        })
    }

    pub(crate) fn copy_path(&self, request_id: Uuid) -> Option<String> {
        let request = self.request(request_id)?;
        (request.state == ClipboardImagePasteState::Stale)
            .then(|| request.remote_path.clone())
            .flatten()
    }

    pub(crate) fn mark_copy_failed(&mut self, request_id: Uuid, error: String) -> bool {
        let Some(request) = self.request_mut(request_id) else {
            return false;
        };
        if request.state != ClipboardImagePasteState::Stale || request.remote_path.is_none() {
            return false;
        }
        request.detail = bounded_detail(format!("Copy failed: {error}"));
        self.bump_revision();
        true
    }

    pub(crate) fn dismiss(&mut self, request_id: Uuid) -> bool {
        let Some(index) = self.request_index(request_id) else {
            return false;
        };
        match self.requests[index].state {
            ClipboardImagePasteState::Success { .. }
            | ClipboardImagePasteState::Stale
            | ClipboardImagePasteState::Error => {
                self.requests.remove(index);
            }
            ClipboardImagePasteState::Preparing
            | ClipboardImagePasteState::Queued
            | ClipboardImagePasteState::Uploading
            | ClipboardImagePasteState::AwaitingInsert => {
                let request = &mut self.requests[index];
                request.dismissed = true;
                request.preview = None;
            }
            ClipboardImagePasteState::Tombstone => return false,
        }
        self.bump_revision();
        true
    }

    pub(crate) fn expire_success(&mut self, now: Instant) -> bool {
        let original_len = self.requests.len();
        self.requests.retain(|request| {
            !matches!(
                request.state,
                ClipboardImagePasteState::Success { expires_at } if expires_at <= now
            )
        });
        if self.requests.len() == original_len {
            return false;
        }
        self.bump_revision();
        true
    }

    pub(crate) fn remove_session(&mut self, session_id: Uuid) -> bool {
        let epoch_removed = self.input_epochs.remove(&session_id).is_some();
        let mut removed_request = false;
        let mut retained = VecDeque::with_capacity(self.requests.len());
        while let Some(mut request) = self.requests.pop_front() {
            if request.session_id != session_id {
                retained.push_back(request);
                continue;
            }
            removed_request = true;
            if self.active_upload_id == Some(request.request_id)
                && request.state == ClipboardImagePasteState::Uploading
            {
                request.runtime = None;
                request.png_bytes = None;
                request.preview = None;
                request.remote_path = None;
                request.detail.clear();
                request.dismissed = true;
                request.state = ClipboardImagePasteState::Tombstone;
                retained.push_back(request);
            }
        }
        self.requests = retained;
        if !epoch_removed && !removed_request {
            return false;
        }
        self.bump_revision();
        true
    }

    pub(crate) fn retained_request_count(&self) -> usize {
        self.requests
            .iter()
            .filter(|request| request.state != ClipboardImagePasteState::Tombstone)
            .count()
    }

    pub(crate) fn session_ids(&self) -> Vec<Uuid> {
        let mut session_ids = self.input_epochs.keys().copied().collect::<Vec<_>>();
        for request in &self.requests {
            if request.state != ClipboardImagePasteState::Tombstone
                && !session_ids.contains(&request.session_id)
            {
                session_ids.push(request.session_id);
            }
        }
        session_ids
    }

    pub(crate) fn projections(
        &self,
        active_session_id: Uuid,
    ) -> Vec<ClipboardImagePasteProjection> {
        self.requests
            .iter()
            .filter(|request| {
                request.session_id == active_session_id
                    && !request.dismissed
                    && request.state != ClipboardImagePasteState::Tombstone
            })
            .map(|request| {
                let (status, paste_enabled, copy_enabled, collapsed) = match request.state {
                    ClipboardImagePasteState::Preparing => ("preparing", false, false, false),
                    ClipboardImagePasteState::Queued => ("queued", false, false, false),
                    ClipboardImagePasteState::Uploading
                    | ClipboardImagePasteState::AwaitingInsert => {
                        ("uploading", false, false, false)
                    }
                    ClipboardImagePasteState::Success { .. } => ("success", false, false, true),
                    ClipboardImagePasteState::Stale => ("stale", true, true, false),
                    ClipboardImagePasteState::Error => ("error", false, false, false),
                    ClipboardImagePasteState::Tombstone => unreachable!(),
                };
                ClipboardImagePasteProjection {
                    request_id: request.request_id,
                    status,
                    preview: request.preview.clone(),
                    source_width: request.source_width,
                    source_height: request.source_height,
                    detail: request.detail.clone(),
                    paste_enabled,
                    copy_enabled,
                    collapsed,
                    bytes_transferred: request.bytes_transferred,
                    bytes_total: request.bytes_total,
                    bytes_per_second: request.bytes_per_second,
                }
            })
            .collect()
    }

    fn request_index(&self, request_id: Uuid) -> Option<usize> {
        self.requests
            .iter()
            .position(|request| request.request_id == request_id)
    }

    fn request(&self, request_id: Uuid) -> Option<&ClipboardImagePasteRequest<R>> {
        self.requests
            .iter()
            .find(|request| request.request_id == request_id)
    }

    fn request_mut(&mut self, request_id: Uuid) -> Option<&mut ClipboardImagePasteRequest<R>> {
        self.requests
            .iter_mut()
            .find(|request| request.request_id == request_id)
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}

fn average_bytes_per_second(bytes: u64, elapsed: Duration) -> u64 {
    let nanos = elapsed.as_nanos();
    if bytes == 0 || nanos == 0 {
        return 0;
    }
    u64::try_from(
        u128::from(bytes)
            .saturating_mul(1_000_000_000)
            .checked_div(nanos)
            .unwrap_or_default(),
    )
    .unwrap_or(u64::MAX)
}

fn transition_to_error<R>(request: &mut ClipboardImagePasteRequest<R>, error: String) {
    request.runtime = None;
    request.png_bytes = None;
    request.preview = None;
    request.remote_path = None;
    request.detail = bounded_detail(error);
    request.dismissed = false;
    request.state = ClipboardImagePasteState::Error;
}

fn bounded_detail(detail: String) -> String {
    const MAX_DETAIL_CHARS: usize = 320;
    detail.chars().take(MAX_DETAIL_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use uuid::Uuid;

    use super::*;
    use crate::app::clipboard::{ClipboardImagePreview, EncodedClipboardImage};

    fn prepared(seed: u8) -> EncodedClipboardImage {
        EncodedClipboardImage {
            png_bytes: vec![seed; 16],
            width: 80,
            height: 40,
            preview: ClipboardImagePreview {
                width: 80,
                height: 40,
                rgba: vec![seed; 80 * 40 * 4],
            },
        }
    }

    fn uploading_controller_fixture(
        bytes_total: usize,
    ) -> (ClipboardImagePasteController<&'static str>, Uuid, Uuid) {
        let session_id = Uuid::new_v4();
        let binding_id = Uuid::new_v4();
        let mut controller = ClipboardImagePasteController::default();
        let request_id = controller
            .register(session_id, binding_id, "runtime")
            .expect("register request");
        let mut encoded = prepared(1);
        encoded.png_bytes = vec![1; bytes_total];
        assert!(controller.mark_prepared(request_id, encoded));
        assert_eq!(
            controller.take_next_upload().map(|job| job.encoded_bytes),
            Some(bytes_total)
        );
        (controller, session_id, request_id)
    }

    #[test]
    fn upload_progress_is_monotonic_and_retained_after_success() {
        let (mut controller, session_id, request_id) = uploading_controller_fixture(1_024);

        assert!(controller.mark_upload_progress(request_id, 512, 1_024, Duration::from_secs(1),));
        assert!(!controller.mark_upload_progress(request_id, 256, 1_024, Duration::from_secs(2),));
        assert!(matches!(
            controller.mark_upload_succeeded(request_id, "/tmp/image.png".into()),
            ClipboardImageCompletion::AutoInsert(_),
        ));

        let item = controller.projections(session_id)[0].clone();
        assert_eq!(item.bytes_transferred, 512);
        assert_eq!(item.bytes_total, 1_024);
        assert_eq!(item.bytes_per_second, 512);

        assert!(controller.mark_inserted(request_id, Instant::now()));
        let item = controller.projections(session_id)[0].clone();
        assert_eq!(item.status, "success");
        assert_eq!(item.bytes_transferred, 512);
        assert_eq!(item.bytes_total, 1_024);
        assert_eq!(item.bytes_per_second, 512);
    }

    #[test]
    fn upload_progress_rejects_stale_total_and_regressing_samples_and_clamps_bytes() {
        let (mut controller, session_id, request_id) = uploading_controller_fixture(1_024);

        assert!(!controller.mark_upload_progress(
            Uuid::new_v4(),
            256,
            1_024,
            Duration::from_secs(1),
        ));
        assert!(!controller.mark_upload_progress(request_id, 256, 512, Duration::from_secs(1),));
        assert!(controller.mark_upload_progress(request_id, 2_048, 1_024, Duration::from_secs(2),));
        assert!(
            !controller.mark_upload_progress(request_id, 1_023, 1_024, Duration::from_secs(3),)
        );

        let item = controller.projections(session_id)[0].clone();
        assert_eq!(item.bytes_transferred, 1_024);
        assert_eq!(item.bytes_total, 1_024);
        assert_eq!(item.bytes_per_second, 512);
    }

    #[test]
    fn upload_progress_handles_zero_elapsed_without_division_failure() {
        let (mut controller, session_id, request_id) = uploading_controller_fixture(1_024);

        assert!(controller.mark_upload_progress(request_id, 512, 1_024, Duration::ZERO));

        let item = controller.projections(session_id)[0].clone();
        assert_eq!(item.bytes_transferred, 512);
        assert_eq!(item.bytes_total, 1_024);
        assert_eq!(item.bytes_per_second, 0);
    }

    #[test]
    fn later_preparation_waits_for_the_oldest_request() {
        let session = Uuid::new_v4();
        let binding = Uuid::new_v4();
        let mut controller = ClipboardImagePasteController::default();
        let first = controller
            .register(session, binding, "runtime-1")
            .expect("register first");
        let second = controller
            .register(session, binding, "runtime-2")
            .expect("register second");

        assert!(controller.mark_prepared(second, prepared(2)));
        assert!(controller.take_next_upload().is_none());
        assert!(controller.mark_prepared(first, prepared(1)));
        assert_eq!(
            controller.take_next_upload().map(|job| job.request_id),
            Some(first),
        );
    }

    #[test]
    fn changed_input_turns_successful_upload_into_stale_state() {
        let session = Uuid::new_v4();
        let binding = Uuid::new_v4();
        let mut controller = ClipboardImagePasteController::default();
        let request = controller
            .register(session, binding, "runtime")
            .expect("register request");
        assert!(controller.mark_prepared(request, prepared(1)));
        assert!(controller.take_next_upload().is_some());
        assert_eq!(
            controller.active_upload_binding_context(request),
            Some(ClipboardImageBindingContext {
                request_id: request,
                session_id: session,
                binding_id: binding,
            })
        );

        controller.note_terminal_input(session);
        let completion = controller.mark_upload_succeeded(request, "/tmp/image.png".into());

        assert_eq!(completion, ClipboardImageCompletion::Stale);
        assert_eq!(
            controller.stale_binding_contexts(),
            vec![ClipboardImageBindingContext {
                request_id: request,
                session_id: session,
                binding_id: binding,
            }]
        );
        assert_eq!(controller.projections(session)[0].status, "stale");
    }

    #[test]
    fn first_insert_makes_the_next_same_epoch_request_stale() {
        let session = Uuid::new_v4();
        let binding = Uuid::new_v4();
        let mut controller = ClipboardImagePasteController::default();
        let first = controller.register(session, binding, "one").unwrap();
        let second = controller.register(session, binding, "two").unwrap();
        controller.mark_prepared(first, prepared(1));
        controller.mark_prepared(second, prepared(2));

        let first_job = controller.take_next_upload().unwrap();
        assert_eq!(first_job.request_id, first);
        assert!(matches!(
            controller.mark_upload_succeeded(first, "/tmp/one.png".into()),
            ClipboardImageCompletion::AutoInsert(_),
        ));
        controller.note_terminal_input(session);
        controller.mark_inserted(first, Instant::now());

        let second_job = controller.take_next_upload().unwrap();
        assert_eq!(second_job.request_id, second);
        assert_eq!(
            controller.mark_upload_succeeded(second, "/tmp/two.png".into()),
            ClipboardImageCompletion::Stale,
        );
    }

    #[test]
    fn queue_rejects_the_ninth_retained_request() {
        let session = Uuid::new_v4();
        let binding = Uuid::new_v4();
        let mut controller = ClipboardImagePasteController::default();
        for index in 0..CLIPBOARD_IMAGE_PASTE_QUEUE_CAPACITY {
            controller
                .register(session, binding, index)
                .expect("queue slot should be available");
        }
        assert_eq!(
            controller.register(session, binding, 9),
            Err(ClipboardImagePasteRegisterError::QueueFull),
        );
    }

    #[test]
    fn dismissed_pending_item_reappears_without_pixels_when_it_becomes_stale() {
        let session = Uuid::new_v4();
        let binding = Uuid::new_v4();
        let mut controller = ClipboardImagePasteController::default();
        let request = controller.register(session, binding, "runtime").unwrap();
        controller.mark_prepared(request, prepared(1));
        assert!(controller.take_next_upload().is_some());
        assert!(controller.projections(session)[0].preview.is_some());

        assert!(controller.dismiss(request));
        assert!(controller.projections(session).is_empty());
        controller.note_terminal_input(session);
        assert_eq!(
            controller.mark_upload_succeeded(request, "/tmp/image.png".into()),
            ClipboardImageCompletion::Stale,
        );

        let projection = &controller.projections(session)[0];
        assert_eq!(projection.status, "stale");
        assert!(projection.preview.is_none());
        assert!(projection.paste_enabled);
        assert!(projection.copy_enabled);
    }

    #[test]
    fn dismissed_pending_failure_reappears_as_dismissible_error() {
        let session = Uuid::new_v4();
        let binding = Uuid::new_v4();
        let mut controller = ClipboardImagePasteController::default();
        let request = controller.register(session, binding, "runtime").unwrap();
        controller.mark_prepared(request, prepared(1));
        assert!(controller.take_next_upload().is_some());

        assert!(controller.dismiss(request));
        assert!(controller.projections(session).is_empty());
        assert!(controller.mark_upload_failed(request, "network failed".into()));

        let projection = &controller.projections(session)[0];
        assert_eq!(projection.status, "error");
        assert!(projection.preview.is_none());
        assert!(controller.dismiss(request));
        assert_eq!(controller.retained_request_count(), 0);
    }

    #[test]
    fn success_expires_after_the_feedback_lifetime() {
        let session = Uuid::new_v4();
        let binding = Uuid::new_v4();
        let mut controller = ClipboardImagePasteController::default();
        let request = controller.register(session, binding, "runtime").unwrap();
        controller.mark_prepared(request, prepared(1));
        controller.take_next_upload().unwrap();
        assert!(matches!(
            controller.mark_upload_succeeded(request, "/tmp/image.png".into()),
            ClipboardImageCompletion::AutoInsert(_),
        ));
        controller.note_terminal_input(session);
        let inserted_at = Instant::now();
        assert!(controller.mark_inserted(request, inserted_at));

        assert!(!controller.expire_success(
            inserted_at + CLIPBOARD_IMAGE_SUCCESS_LIFETIME - Duration::from_millis(1)
        ));
        assert_eq!(controller.retained_request_count(), 1);
        assert!(controller.expire_success(inserted_at + CLIPBOARD_IMAGE_SUCCESS_LIFETIME));
        assert_eq!(controller.retained_request_count(), 0);
    }

    #[test]
    fn removing_a_session_keeps_only_the_active_upload_tombstone() {
        let removed_session = Uuid::new_v4();
        let other_session = Uuid::new_v4();
        let binding = Uuid::new_v4();
        let mut controller = ClipboardImagePasteController::default();
        let active = controller
            .register(removed_session, binding, "removed-active")
            .unwrap();
        let queued = controller
            .register(removed_session, binding, "removed-queued")
            .unwrap();
        controller.mark_prepared(active, prepared(1));
        controller.mark_prepared(queued, prepared(2));
        controller.take_next_upload().unwrap();

        assert!(controller.remove_session(removed_session));
        assert_eq!(controller.retained_request_count(), 0);

        let other = controller
            .register(other_session, binding, "other-runtime")
            .unwrap();
        controller.mark_prepared(other, prepared(3));
        assert!(controller.take_next_upload().is_none());
        assert_eq!(
            controller.mark_upload_succeeded(active, "/tmp/ignored.png".into()),
            ClipboardImageCompletion::Ignored,
        );
        assert_eq!(controller.take_next_upload().unwrap().request_id, other);
    }

    #[test]
    fn failed_head_preparation_unblocks_the_next_ready_request() {
        let session = Uuid::new_v4();
        let binding = Uuid::new_v4();
        let mut controller = ClipboardImagePasteController::default();
        let first = controller.register(session, binding, "one").unwrap();
        let second = controller.register(session, binding, "two").unwrap();
        controller.mark_prepared(second, prepared(2));

        assert!(controller.mark_preparation_failed(first, "decode failed".into()));
        assert_eq!(
            controller.take_next_upload().map(|job| job.request_id),
            Some(second),
        );
        assert!(controller.take_next_upload().is_none());
    }

    #[test]
    fn stale_path_actions_require_the_originating_active_session_and_copy_is_local() {
        let session = Uuid::new_v4();
        let other_session = Uuid::new_v4();
        let binding = Uuid::new_v4();
        let mut controller = ClipboardImagePasteController::default();
        let request = controller.register(session, binding, "runtime").unwrap();
        controller.mark_prepared(request, prepared(1));
        controller.take_next_upload().unwrap();
        controller.note_terminal_input(session);
        assert_eq!(
            controller.mark_upload_succeeded(request, "/tmp/image.png".into()),
            ClipboardImageCompletion::Stale,
        );

        assert!(
            controller
                .stale_paste_action(request, other_session)
                .is_none()
        );
        let action = controller
            .stale_paste_action(request, session)
            .expect("originating active session may paste");
        assert_eq!(action.binding_id, binding);
        assert_eq!(action.remote_path, "/tmp/image.png");
        assert_eq!(
            controller.copy_path(request).as_deref(),
            Some("/tmp/image.png")
        );

        let later = controller.register(session, binding, "later").unwrap();
        controller.mark_prepared(later, prepared(2));
        controller.take_next_upload().unwrap();
        assert_eq!(
            controller.copy_path(request).as_deref(),
            Some("/tmp/image.png")
        );
        assert!(matches!(
            controller.mark_upload_succeeded(later, "/tmp/later.png".into()),
            ClipboardImageCompletion::AutoInsert(_),
        ));

        controller.note_terminal_input(session);
        let inserted_at = Instant::now();
        assert!(controller.mark_inserted(request, inserted_at));
        let projection = controller
            .projections(session)
            .into_iter()
            .find(|projection| projection.request_id == request)
            .expect("explicit insertion remains visible as success feedback");
        assert_eq!(projection.status, "success");
        assert!(projection.collapsed);
    }

    #[test]
    fn clipboard_copy_error_detail_stays_bounded() {
        let session = Uuid::new_v4();
        let binding = Uuid::new_v4();
        let mut controller = ClipboardImagePasteController::default();
        let request = controller.register(session, binding, "runtime").unwrap();
        controller.mark_prepared(request, prepared(1));
        controller.take_next_upload().unwrap();
        controller.note_terminal_input(session);
        assert_eq!(
            controller.mark_upload_succeeded(request, "/tmp/image.png".into()),
            ClipboardImageCompletion::Stale,
        );

        assert!(controller.mark_copy_failed(request, "x".repeat(500)));
        assert_eq!(
            controller.projections(session)[0].detail.chars().count(),
            320
        );
    }

    #[test]
    fn removing_a_session_releases_queued_pixels_and_runtime_tokens() {
        let session = Uuid::new_v4();
        let binding = Uuid::new_v4();
        let mut controller = ClipboardImagePasteController::default();
        let request = controller.register(session, binding, "runtime").unwrap();
        controller.mark_prepared(request, prepared(7));

        assert_eq!(controller.session_ids(), vec![session]);
        assert!(controller.remove_session(session));
        assert!(controller.projections(session).is_empty());
        assert!(controller.session_ids().is_empty());
        assert_eq!(controller.retained_request_count(), 0);
    }

    #[test]
    fn input_epoch_only_session_remains_discoverable_for_cleanup() {
        let session = Uuid::new_v4();
        let mut controller = ClipboardImagePasteController::<()>::default();

        controller.note_terminal_input(session);
        assert_eq!(controller.session_ids(), vec![session]);
        assert!(controller.remove_session(session));
        assert!(controller.session_ids().is_empty());
    }
}
