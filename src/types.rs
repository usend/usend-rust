//! Request and response types. Timestamps are RFC 3339 strings (the crate has no date
//! dependency); parse them with `chrono` or `time` if you need to. Response structs tolerate
//! missing fields (`#[serde(default)]`) and unknown ones, so the API can grow without breaking you.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::crypto::base64_encode;

fn is_none<T>(o: &Option<T>) -> bool {
    o.is_none()
}

// ------------------------------------------------------------------ common

/// The envelope of list endpoints. `has_more` is only set by paginated lists.
#[derive(Debug, Clone, Deserialize)]
pub struct List<T> {
    #[serde(default)]
    pub object: String,
    #[serde(default = "Vec::new")]
    pub data: Vec<T>,
    #[serde(default)]
    pub has_more: bool,
}

/// The minimal `{ object, id }` response of create and update calls.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct IdResult {
    pub object: String,
    pub id: String,
}

/// The response of delete calls on most resources.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DeleteResult {
    pub object: String,
    pub id: String,
    pub deleted: bool,
}

/// Pagination parameters of list endpoints. Rows are newest first: `after` returns rows older
/// than the given ID, `before` newer ones.
#[derive(Debug, Clone, Default)]
pub struct ListParams {
    pub limit: Option<u32>,
    pub after: Option<String>,
    pub before: Option<String>,
}

impl ListParams {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn limit(mut self, n: u32) -> Self {
        self.limit = Some(n);
        self
    }
    pub fn after(mut self, id: impl Into<String>) -> Self {
        self.after = Some(id.into());
        self
    }
    pub fn before(mut self, id: impl Into<String>) -> Self {
        self.before = Some(id.into());
        self
    }
    pub(crate) fn pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            ("limit", self.limit.map(|n| n.to_string()).unwrap_or_default()),
            ("after", self.after.clone().unwrap_or_default()),
            ("before", self.before.clone().unwrap_or_default()),
        ]
    }
}

// ------------------------------------------------------------------ emails

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Tag {
    pub name: String,
    pub value: String,
}

/// A file sent with an email. `content` is base64; use [`Attachment::new`] to encode bytes.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Attachment {
    pub filename: String,
    pub content: String,
    #[serde(skip_serializing_if = "is_none")]
    pub content_type: Option<String>,
    /// Makes the attachment inline: reference it as `<img src="cid:...">`.
    #[serde(skip_serializing_if = "is_none")]
    pub content_id: Option<String>,
}

impl Attachment {
    pub fn new(filename: impl Into<String>, bytes: &[u8]) -> Self {
        Attachment { filename: filename.into(), content: base64_encode(bytes), content_type: None, content_id: None }
    }
    pub fn content_type(mut self, ct: impl Into<String>) -> Self {
        self.content_type = Some(ct.into());
        self
    }
    pub fn content_id(mut self, id: impl Into<String>) -> Self {
        self.content_id = Some(id.into());
        self
    }
}

/// Sends a published template instead of `html`/`text`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct EmailTemplate {
    /// Template ID or alias.
    pub id: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub variables: BTreeMap<String, Value>,
}

impl EmailTemplate {
    pub fn new(id: impl Into<String>) -> Self {
        EmailTemplate { id: id.into(), variables: BTreeMap::new() }
    }
    pub fn variable(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.variables.insert(key.into(), value.into());
        self
    }
}

/// An email to send, built fluently:
///
/// ```
/// use usend::SendEmail;
/// let email = SendEmail::new("Acme <hello@acme.com>", ["ann@example.com"])
///     .subject("Hello")
///     .html("<p>It works.</p>")
///     .idempotency_key("welcome-ann-1");
/// ```
#[derive(Debug, Clone, Default, Serialize)]
pub struct SendEmail {
    pub from: String,
    pub to: Vec<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cc: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bcc: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reply_to: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<Tag>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
    /// RFC 3339 time or a phrase such as `"in 1 hour"`.
    #[serde(skip_serializing_if = "is_none")]
    pub scheduled_at: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub template: Option<EmailTemplate>,
    /// Sent as the `Idempotency-Key` header, not in the body.
    #[serde(skip)]
    pub idempotency_key: Option<String>,
}

impl SendEmail {
    pub fn new<S: Into<String>>(from: impl Into<String>, to: impl IntoIterator<Item = S>) -> Self {
        SendEmail { from: from.into(), to: to.into_iter().map(Into::into).collect(), ..Default::default() }
    }
    pub fn subject(mut self, v: impl Into<String>) -> Self {
        self.subject = Some(v.into());
        self
    }
    pub fn html(mut self, v: impl Into<String>) -> Self {
        self.html = Some(v.into());
        self
    }
    pub fn text(mut self, v: impl Into<String>) -> Self {
        self.text = Some(v.into());
        self
    }
    pub fn cc<S: Into<String>>(mut self, v: impl IntoIterator<Item = S>) -> Self {
        self.cc = v.into_iter().map(Into::into).collect();
        self
    }
    pub fn bcc<S: Into<String>>(mut self, v: impl IntoIterator<Item = S>) -> Self {
        self.bcc = v.into_iter().map(Into::into).collect();
        self
    }
    pub fn reply_to<S: Into<String>>(mut self, v: impl IntoIterator<Item = S>) -> Self {
        self.reply_to = v.into_iter().map(Into::into).collect();
        self
    }
    pub fn header(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.headers.insert(k.into(), v.into());
        self
    }
    pub fn tag(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.push(Tag { name: name.into(), value: value.into() });
        self
    }
    pub fn attachment(mut self, a: Attachment) -> Self {
        self.attachments.push(a);
        self
    }
    pub fn scheduled_at(mut self, v: impl Into<String>) -> Self {
        self.scheduled_at = Some(v.into());
        self
    }
    pub fn template(mut self, t: EmailTemplate) -> Self {
        self.template = Some(t);
        self
    }
    pub fn idempotency_key(mut self, k: impl Into<String>) -> Self {
        self.idempotency_key = Some(k.into());
        self
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SendEmailResponse {
    pub id: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct BatchError {
    pub index: usize,
    pub message: String,
}

/// The response of a batch send. `errors` is only set in permissive mode.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct BatchResponse {
    pub data: Vec<SendEmailResponse>,
    pub errors: Vec<BatchError>,
}

/// A sent email. Lists omit `html` and `text`. `last_event` is one of queued, scheduled, sent,
/// delivered, delivery_delayed, bounced, complained, opened, clicked, failed, canceled.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Email {
    pub object: String,
    pub id: String,
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub reply_to: Vec<String>,
    pub subject: String,
    pub html: Option<String>,
    pub text: Option<String>,
    pub tags: Vec<Tag>,
    pub created_at: String,
    pub scheduled_at: Option<String>,
    pub last_event: String,
    pub broadcast_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct EmailEvent {
    pub id: String,
    /// `email.delivered`, `email.bounced`, ...
    #[serde(rename = "type")]
    pub event_type: String,
    pub recipient: String,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct EmailAttachment {
    pub object: String,
    pub id: String,
    pub filename: String,
    pub content_type: String,
    pub content_disposition: String,
    pub size: u64,
    /// Needs no API key, valid for one hour.
    pub download_url: String,
    pub expires_at: String,
}

/// Filters for `GET /emails/metrics`. List values are sent comma separated.
#[derive(Debug, Clone, Default)]
pub struct EmailMetricsParams {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub timezone: Option<String>,
    /// `hourly`, `daily`, `weekly` or `monthly`.
    pub granularity: Option<String>,
    pub metrics: Vec<String>,
    /// `period`, `domain`, `email`, `broadcast`.
    pub dimensions: Vec<String>,
    pub domain_ids: Vec<String>,
    pub email_ids: Vec<String>,
    pub broadcast_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct EmailTotals {
    pub sent: u64,
    pub delivered: u64,
    pub delivery_delayed: u64,
    pub bounced: u64,
    pub complained: u64,
    pub opened: u64,
    pub clicked: u64,
    pub failed: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct EmailMetrics {
    pub object: String,
    pub start_date: String,
    pub end_date: String,
    pub metrics: Vec<String>,
    pub dimensions: Vec<String>,
    pub granularity: String,
    pub totals: EmailTotals,
    /// One row per dimension combination; only set when `dimensions` was given.
    pub data: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ShareResult {
    pub object: String,
    pub id: String,
    /// Public, read-only and expiring.
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TestEmailParams {
    pub from: String,
    #[serde(skip_serializing_if = "is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TestEmailResult {
    pub id: String,
    pub to: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Insight {
    pub id: String,
    /// `critical`, `warning`, `info` or `pass`.
    pub severity: String,
    pub title: String,
    pub detail: String,
    pub fix: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct InsightSummary {
    pub critical: u32,
    pub warning: u32,
    pub info: u32,
    pub pass: u32,
}

/// Deliverability findings for an email or a domain.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Insights {
    pub object: String,
    pub email_id: Option<String>,
    pub domain_id: Option<String>,
    pub summary: InsightSummary,
    pub insights: Vec<Insight>,
}

/// A received email. `html`, `text`, `headers` and `attachments` are set by `get`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct InboundEmail {
    pub object: String,
    pub id: String,
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub reply_to: Vec<String>,
    pub subject: String,
    pub html: Option<String>,
    pub text: Option<String>,
    pub headers: Value,
    pub attachments: Vec<InboundAttachment>,
    pub attachment_count: u32,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct InboundAttachment {
    pub id: String,
    pub filename: String,
    pub content_type: String,
    pub size: u64,
}

// ----------------------------------------------------------------- domains

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DomainRecord {
    /// `DKIM`, `SPF`, `DMARC`, `Receiving`, `BIMI`.
    pub record: String,
    /// Relative to the domain; `@` is the apex.
    pub name: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub ttl: String,
    pub status: String,
    pub value: String,
    pub priority: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DomainHint {
    pub record: String,
    pub message: String,
}

/// A domain. `status` is not_started, pending, verified, failed or temporary_failure.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Domain {
    pub object: String,
    pub id: String,
    pub name: String,
    pub status: String,
    pub created_at: String,
    pub region: String,
    pub open_tracking: bool,
    pub click_tracking: bool,
    pub receiving_enabled: bool,
    /// "all" (catch-all, the default) or "inboxes".
    #[serde(default)]
    pub receiving_mode: String,
    pub custom_return_path: String,
    pub tls: String,
    pub dns_provider: String,
    pub records: Vec<DomainRecord>,
    pub hints: Vec<DomainHint>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CreateDomain {
    pub name: String,
    #[serde(skip_serializing_if = "is_none")]
    pub custom_return_path: Option<String>,
    /// `opportunistic` (default) or `enforced`.
    #[serde(skip_serializing_if = "is_none")]
    pub tls: Option<String>,
}

impl CreateDomain {
    pub fn new(name: impl Into<String>) -> Self {
        CreateDomain { name: name.into(), ..Default::default() }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateDomain {
    #[serde(skip_serializing_if = "is_none")]
    pub open_tracking: Option<bool>,
    #[serde(skip_serializing_if = "is_none")]
    pub click_tracking: Option<bool>,
    #[serde(skip_serializing_if = "is_none")]
    pub receiving_enabled: Option<bool>,
    #[serde(skip_serializing_if = "is_none")]
    pub receiving_mode: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub custom_return_path: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub tls: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AutoConfigureRecord {
    pub name: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub value: String,
    /// `created`, `updated`, `unchanged`, `skipped` or `conflict`.
    pub action: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AutoConfigureResult {
    pub object: String,
    pub id: String,
    pub provider: String,
    pub zone: String,
    pub records: Vec<AutoConfigureRecord>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ClaimRecord {
    #[serde(rename = "type")]
    pub record_type: String,
    /// `_usend-claim.<name>`
    pub name: String,
    pub value: String,
    pub ttl: Value,
}

/// A claim on a name another organization already verified.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DomainClaim {
    pub object: String,
    pub id: String,
    pub name: String,
    /// `pending`, `verified`, `failed` or `expired`.
    pub status: String,
    pub domain_id: Option<String>,
    pub region: String,
    pub record: ClaimRecord,
    pub blocked_reason: Option<String>,
    pub failure_reason: Option<String>,
    pub created_at: String,
    pub expires_at: String,
}

// ---------------------------------------------------------------- api keys

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ApiKey {
    pub id: String,
    pub name: String,
    /// `full_access` or `sending_access`.
    pub permission: String,
    pub domain_id: Option<String>,
    pub token_prefix: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CreateApiKey {
    pub name: String,
    /// `"full_access"` or `"sending_access"`.
    pub permission: String,
    #[serde(skip_serializing_if = "is_none")]
    pub domain_id: Option<String>,
}

/// The new key's token is shown only once.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CreateApiKeyResponse {
    pub id: String,
    pub token: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateApiKey {
    #[serde(skip_serializing_if = "is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub permission: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub domain_id: Option<String>,
}

// --------------------------------------------------------------- templates

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TemplateVariable {
    pub key: String,
    /// `string` or `number`.
    #[serde(rename = "type")]
    pub var_type: String,
    #[serde(skip_serializing_if = "is_none")]
    pub fallback_value: Option<Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Template {
    pub object: String,
    pub id: String,
    pub name: String,
    pub alias: Option<String>,
    /// `draft` or `published`.
    pub status: String,
    pub published_at: Option<String>,
    pub has_unpublished_versions: bool,
    pub from: Option<String>,
    pub subject: Option<String>,
    pub reply_to: Vec<String>,
    pub html: Option<String>,
    pub text: Option<String>,
    pub variables: Vec<TemplateVariable>,
    pub created_at: String,
    pub updated_at: String,
}

/// Body of template create and update. On update every field is optional.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TemplateParams {
    #[serde(skip_serializing_if = "is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reply_to: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<TemplateVariable>,
}

impl TemplateParams {
    pub fn new(name: impl Into<String>, html: impl Into<String>) -> Self {
        TemplateParams { name: Some(name.into()), html: Some(html.into()), ..Default::default() }
    }
}

/// One published version of a template; the content fields are set by `get_version`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TemplateVersion {
    pub object: String,
    pub id: String,
    pub template_id: String,
    pub version: u32,
    pub name: String,
    /// Identical to what sends use.
    pub current: bool,
    pub created_by: Option<String>,
    pub created_at: String,
    pub from: Option<String>,
    pub subject: Option<String>,
    pub reply_to: Vec<String>,
    pub html: Option<String>,
    pub text: Option<String>,
    pub variables: Vec<TemplateVariable>,
}

// ---------------------------------------------- segments, audiences, contacts

/// A group of contacts. Audiences are the same thing under the older name.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Segment {
    pub object: String,
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub contact_count: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SegmentMetrics {
    pub object: String,
    pub id: String,
    pub contacts: u64,
    pub subscribed: u64,
    pub unsubscribed: u64,
    pub sends: u64,
    pub delivered: u64,
    pub bounced: u64,
    pub opens: u64,
    pub clicks: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Contact {
    pub object: String,
    pub id: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub unsubscribed: bool,
    pub properties: BTreeMap<String, Value>,
    pub created_at: String,
}

/// Body of `POST /contacts`. Without `segments` the contact joins the default `General` segment.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CreateContact {
    pub email: String,
    #[serde(skip_serializing_if = "is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub last_name: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub unsubscribed: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub segments: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, Value>,
}

impl CreateContact {
    pub fn new(email: impl Into<String>) -> Self {
        CreateContact { email: email.into(), ..Default::default() }
    }
}

/// Body of a contact update. A `Value::Null` property removes it.
#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateContact {
    #[serde(skip_serializing_if = "is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub last_name: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub unsubscribed: Option<bool>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default)]
pub struct ListContactsParams {
    pub list: ListParams,
    pub segment_id: Option<String>,
    pub topic_id: Option<String>,
    /// `subscribed` or `unsubscribed`.
    pub subscription: Option<String>,
    /// Search text.
    pub q: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ContactStats {
    pub object: String,
    pub total: u64,
    pub subscribers: u64,
    pub unsubscribers: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ImportRowError {
    pub row: u64,
    pub email: Option<String>,
    pub message: String,
}

/// The result of a synchronous CSV import.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ImportResult {
    pub imported: u64,
    pub updated: u64,
    pub skipped: u64,
    pub errors: Vec<ImportRowError>,
    pub ignored_columns: Vec<String>,
}

/// A background contact import. `status` is pending, processing, completed or failed.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ContactImport {
    pub object: String,
    pub id: String,
    pub segment_id: Option<String>,
    pub status: String,
    pub imported: u64,
    pub updated: u64,
    pub skipped: u64,
    pub errors: Vec<ImportRowError>,
    pub ignored_columns: Vec<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

/// Body of `POST /contacts/imports`: set either `csv` or `contacts`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CreateContactImport {
    #[serde(skip_serializing_if = "is_none")]
    pub segment_id: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub csv: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub contacts: Vec<CreateContact>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ContactTopic {
    pub id: String,
    pub name: String,
    /// `opt_in` or `opt_out`.
    pub subscription: String,
    pub explicit: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TopicChoice {
    pub id: String,
    /// `opt_in` or `opt_out`.
    pub subscription: String,
}

/// A typed custom field on contacts.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ContactProperty {
    pub object: String,
    pub id: String,
    pub key: String,
    /// `string`, `number`, `boolean` or `date`.
    #[serde(rename = "type")]
    pub property_type: String,
    pub fallback_value: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CreateContactProperty {
    pub key: String,
    #[serde(rename = "type")]
    pub property_type: String,
    #[serde(skip_serializing_if = "is_none")]
    pub fallback_value: Option<Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Topic {
    pub object: String,
    pub id: String,
    pub name: String,
    pub description: String,
    /// `opt_in` or `opt_out`.
    pub default_subscription: String,
    pub subscribers: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TopicParams {
    #[serde(skip_serializing_if = "is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub default_subscription: Option<String>,
}

// -------------------------------------------------------------- broadcasts

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct BroadcastStats {
    pub total: u64,
    pub sent: u64,
    pub delivered: u64,
    pub bounced: u64,
    pub opened: u64,
    pub clicked: u64,
    pub unsubscribed: u64,
}

/// A broadcast. `status` is draft, queued, sending, sent, failed, paused or cancelled.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Broadcast {
    pub object: String,
    pub id: String,
    pub name: Option<String>,
    pub audience_id: String,
    pub topic_id: Option<String>,
    pub from: String,
    pub subject: String,
    pub reply_to: Vec<String>,
    pub html: Option<String>,
    pub text: Option<String>,
    pub status: String,
    pub created_at: String,
    pub scheduled_at: Option<String>,
    pub sent_at: Option<String>,
    pub stats: BroadcastStats,
}

/// Body of broadcast create and (draft-only) update.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BroadcastParams {
    #[serde(skip_serializing_if = "is_none")]
    pub audience_id: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reply_to: Vec<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub name: Option<String>,
    /// `Some("")` clears the topic on update.
    #[serde(skip_serializing_if = "is_none")]
    pub topic_id: Option<String>,
}

/// The event types `Broadcasts::recipients` requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipientType {
    Sent,
    Delivered,
    Opened,
    Clicked,
    Bounced,
    Complained,
    Unsubscribed,
    Suppressed,
}

impl RecipientType {
    pub fn as_str(self) -> &'static str {
        match self {
            RecipientType::Sent => "sent",
            RecipientType::Delivered => "delivered",
            RecipientType::Opened => "opened",
            RecipientType::Clicked => "clicked",
            RecipientType::Bounced => "bounced",
            RecipientType::Complained => "complained",
            RecipientType::Unsubscribed => "unsubscribed",
            RecipientType::Suppressed => "suppressed",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BroadcastRecipientsParams {
    pub list: ListParams,
    /// Substring match.
    pub email: Option<String>,
    /// With `Bounced`: `permanent`, `transient` or `undetermined`.
    pub bounce_type: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ClickedLink {
    pub url: String,
    pub clicks: u64,
}

/// A recipient who had the requested event. `id` is the email's ID.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct BroadcastRecipient {
    pub id: String,
    pub contact_id: Option<String>,
    pub email: String,
    /// Opened and clicked only.
    pub count: Option<u64>,
    /// Bounced only.
    pub bounce_type: Option<String>,
    /// Clicked only.
    pub clicked_links: Vec<ClickedLink>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct BroadcastClickedLink {
    pub object: String,
    pub link: String,
    pub clicks: u64,
    pub unique_clicks: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TimelinePoint {
    pub hour: String,
    pub opened: u64,
    pub clicked: u64,
}

/// Counts are unique per email; rates are percentages.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct BroadcastPerformance {
    pub object: String,
    pub broadcast_id: String,
    pub counts: BTreeMap<String, u64>,
    pub rates: BTreeMap<String, f64>,
    pub timeline: Vec<TimelinePoint>,
}

// ---------------------------------------------------------------- webhooks

/// A webhook endpoint. `signing_secret` (`whsec_...`) is what `verify_webhook` needs.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Webhook {
    pub object: String,
    pub id: String,
    pub endpoint: String,
    pub events: Vec<String>,
    pub enabled: bool,
    pub signing_secret: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CreateWebhook {
    pub endpoint: String,
    pub events: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateWebhook {
    #[serde(skip_serializing_if = "is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub events: Option<Vec<String>>,
    #[serde(skip_serializing_if = "is_none")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct WebhookDelivery {
    pub id: String,
    pub event_type: String,
    /// `pending`, `succeeded` or `failed`.
    pub status: String,
    pub attempts: u32,
    pub response_status: Option<u16>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub delivered_at: Option<String>,
}

/// One delivery of an event to one endpoint. `payload` is set by `get_event`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct WebhookEventRecord {
    pub object: String,
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub webhook_id: String,
    pub endpoint: String,
    pub status: String,
    pub attempts: u32,
    pub response_status: Option<u16>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub delivered_at: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Clone, Default)]
pub struct WebhookEventsParams {
    pub list: ListParams,
    pub webhook_id: Option<String>,
    pub event_type: Option<String>,
    /// `pending`, `succeeded` or `failed`.
    pub status: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct WebhookAttempt {
    pub object: String,
    pub id: String,
    pub attempt: u32,
    pub status: String,
    pub response_status: Option<u16>,
    pub error: Option<String>,
    pub created_at: String,
}

// ------------------------------------------------------------ suppressions

/// `reason` is hard_bounce, complaint, unsubscribe or manual.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Suppression {
    pub id: String,
    pub email: String,
    pub reason: String,
    pub created_at: String,
}

// ------------------------------------------------------------------- stats

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct StatsTotals {
    pub sent: u64,
    pub delivered: u64,
    pub bounced: u64,
    pub complained: u64,
    pub opened: u64,
    pub clicked: u64,
    pub failed: u64,
    pub delayed: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DayStats {
    pub date: String,
    pub sent: u64,
    pub delivered: u64,
    pub bounced: u64,
    pub complained: u64,
    pub opened: u64,
    pub clicked: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Stats {
    pub days: u32,
    pub totals: StatsTotals,
    pub daily: Vec<DayStats>,
}

// ----------------------------------------------------------- custom events

/// A named event (for example `order.paid`) that automations can start on.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CustomEvent {
    pub object: String,
    pub id: String,
    pub name: String,
    /// key -> `string`, `number`, `boolean` or `date`.
    pub schema: Option<BTreeMap<String, String>>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CreateEvent {
    pub name: String,
    #[serde(skip_serializing_if = "is_none")]
    pub schema: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateEvent {
    #[serde(skip_serializing_if = "is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub schema: Option<BTreeMap<String, String>>,
}

/// Fires an event for a contact. Give `contact_id` or `email`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SendEvent {
    pub event: String,
    #[serde(skip_serializing_if = "is_none")]
    pub contact_id: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub payload: BTreeMap<String, Value>,
    /// Sent as the `Idempotency-Key` header: a retry returns the first occurrence.
    #[serde(skip)]
    pub idempotency_key: Option<String>,
}

impl SendEvent {
    pub fn for_email(event: impl Into<String>, email: impl Into<String>) -> Self {
        SendEvent { event: event.into(), email: Some(email.into()), ..Default::default() }
    }
    pub fn for_contact(event: impl Into<String>, contact_id: impl Into<String>) -> Self {
        SendEvent { event: event.into(), contact_id: Some(contact_id.into()), ..Default::default() }
    }
    pub fn payload(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.payload.insert(key.into(), value.into());
        self
    }
    pub fn idempotency_key(mut self, k: impl Into<String>) -> Self {
        self.idempotency_key = Some(k.into());
        self
    }
}

/// The `202` response of `events().send`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SendEventResult {
    pub object: String,
    pub event: String,
    pub id: String,
}

// ------------------------------------------------------------- automations

/// One node of an automation graph. `config` depends on `step_type` (trigger, delay, condition,
/// wait_for_event, send_email, add_to_segment, contact_update, contact_delete).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AutomationStep {
    pub key: String,
    #[serde(rename = "type")]
    pub step_type: String,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub config: Value,
}

/// Links two steps. `connection_type` defaults to `default`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AutomationConnection {
    pub from: String,
    pub to: String,
    #[serde(rename = "type", skip_serializing_if = "is_none")]
    pub connection_type: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Automation {
    pub object: String,
    pub id: String,
    pub name: String,
    /// `draft`, `enabled` or `disabled`.
    pub status: String,
    pub trigger_event: Option<String>,
    pub version: u32,
    pub steps: Vec<AutomationStep>,
    pub connections: Vec<AutomationConnection>,
    pub created_at: String,
    pub updated_at: String,
}

/// Body of automation create and update (all optional on update).
#[derive(Debug, Clone, Default, Serialize)]
pub struct AutomationParams {
    #[serde(skip_serializing_if = "is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<AutomationStep>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub connections: Vec<AutomationConnection>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AutomationRunStep {
    pub key: String,
    #[serde(rename = "type")]
    pub step_type: String,
    /// `completed`, `skipped`, `waiting`, `failed` or `cancelled`.
    pub status: String,
    pub branch: String,
    pub output: Value,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

/// One execution for one contact. `steps` is set by `get_run`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AutomationRun {
    pub object: String,
    pub id: String,
    pub automation_id: String,
    pub version: u32,
    /// `running`, `completed`, `failed` or `cancelled`.
    pub status: String,
    pub email: String,
    pub contact_id: Option<String>,
    pub event_id: Option<String>,
    pub current_step: Option<String>,
    pub error: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub steps: Vec<AutomationRunStep>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct StopResult {
    pub id: String,
    pub status: String,
    pub cancelled_runs: u64,
}

// ----------------------------------------------------------------- inboxes

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Inbox {
    pub object: String,
    pub id: String,
    pub name: String,
    pub address: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CreateInbox {
    pub name: String,
    pub address: String,
    #[serde(skip_serializing_if = "is_none")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateInbox {
    #[serde(skip_serializing_if = "is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Label {
    pub object: String,
    pub id: String,
    pub inbox_id: String,
    pub name: String,
    pub color: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct LabelParams {
    #[serde(skip_serializing_if = "is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ThreadLabel {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
}

/// A conversation. `folder` is inbox, archive, spam or trash.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Thread {
    pub object: String,
    pub id: String,
    pub inbox_id: String,
    pub subject: String,
    pub folder: String,
    pub assignee_user_id: Option<String>,
    pub labels: Vec<ThreadLabel>,
    pub snippet: String,
    pub message_count: u32,
    pub unread_count: u32,
    pub last_message_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct ThreadListParams {
    pub list: ListParams,
    pub folder: Option<String>,
    /// Label ID.
    pub label: Option<String>,
    /// User ID or `none`.
    pub assignee: Option<String>,
    pub unread: bool,
    pub q: Option<String>,
}

/// Body of a thread update. `labels` replaces the set when non-empty.
#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateThread {
    #[serde(skip_serializing_if = "is_none")]
    pub folder: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub assignee_user_id: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub read: Option<bool>,
}

/// One message of a thread. The content fields are set by `get_email`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct InboxEmail {
    pub object: String,
    pub id: String,
    pub thread_id: String,
    /// `inbound` or `outbound`.
    pub direction: String,
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub snippet: String,
    pub read: bool,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub inbound_email_id: Option<String>,
    pub email_id: Option<String>,
    pub created_at: String,
    pub reply_to: Vec<String>,
    pub html: Option<String>,
    pub text: Option<String>,
    pub attachments: Vec<InboundAttachment>,
}

/// With no `to`, a reply goes to Reply-To (else the sender), or with `reply_all` to everyone but the inbox.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ReplyParams {
    #[serde(skip_serializing_if = "is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub to: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cc: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bcc: Vec<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub subject: Option<String>,
    pub reply_all: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ForwardParams {
    pub to: Vec<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cc: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bcc: Vec<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub subject: Option<String>,
}

/// The response of reply, forward and draft send.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct InboxSendResult {
    pub object: String,
    pub id: String,
    /// The sent email in `/emails`.
    pub email_id: String,
    pub thread_id: String,
}

/// An unsent message. `kind` is new, reply or forward.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Draft {
    pub object: String,
    pub id: String,
    pub inbox_id: String,
    pub kind: String,
    pub thread_id: Option<String>,
    pub reply_to_email_id: Option<String>,
    pub forward_email_id: Option<String>,
    pub reply_all: bool,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: Option<String>,
    pub html: Option<String>,
    pub text: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DraftParams {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub to: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cc: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bcc: Vec<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub reply_to_email_id: Option<String>,
    #[serde(skip_serializing_if = "is_none")]
    pub forward_email_id: Option<String>,
    pub reply_all: bool,
}

// ------------------------------------------------------------ short links

/// A link on the shared tracking host (`short_url`) that redirects to `target_url` and counts clicks.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ShortLink {
    pub object: String,
    pub id: String,
    pub code: String,
    pub short_url: String,
    pub target_url: String,
    pub title: String,
    pub clicks: u64,
    pub human_clicks: u64,
    pub expires_at: Option<String>,
    pub disabled: bool,
    pub disabled_by_admin: bool,
    pub created_at: String,
}

/// The body of `short_links().create`. `code` is 3 to 32 characters of `A-Za-z0-9-_` (random when unset).
#[derive(Debug, Clone, Default, Serialize)]
pub struct ShortLinkParams {
    pub target_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// RFC 3339.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

impl ShortLinkParams {
    pub fn new(target_url: impl Into<String>) -> Self {
        ShortLinkParams { target_url: target_url.into(), ..Default::default() }
    }
    pub fn code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
    pub fn expires_at(mut self, rfc3339: impl Into<String>) -> Self {
        self.expires_at = Some(rfc3339.into());
        self
    }
}

/// The body of `short_links().update`; unset fields are left alone. The code cannot change.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ShortLinkUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    /// `Some(Some(t))` sets the expiry, `Some(None)` clears it, `None` leaves it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Option<String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ShortLinkClicks {
    pub clicks: u64,
    pub human_clicks: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ShortLinkDay {
    pub date: String,
    pub clicks: u64,
    pub human_clicks: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ShortLinkReferrer {
    pub referrer: String,
    pub clicks: u64,
}

/// Click statistics over a period (zero-filled UTC days).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ShortLinkStats {
    pub object: String,
    pub id: String,
    pub days: u32,
    pub totals: ShortLinkClicks,
    pub in_period: ShortLinkClicks,
    pub daily: Vec<ShortLinkDay>,
    pub referrers: Vec<ShortLinkReferrer>,
}
