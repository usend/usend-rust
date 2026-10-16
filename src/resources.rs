//! Resource services: `client.emails()`, `client.domains()`, ... Each is a cheap borrow of the
//! client. Every method is blocking and returns `usend::Result<T>`.

use serde_json::Value;

use crate::client::{enc, with_query, Client, RequestOptions};
use crate::error::Result;
use crate::types::*;

fn pairs_of(list: &ListParams) -> Vec<(&'static str, String)> {
    list.pairs()
}

fn opt(v: &Option<String>) -> String {
    v.clone().unwrap_or_default()
}

fn csv(v: &[String]) -> String {
    v.join(",")
}

impl Client {
    pub fn emails(&self) -> Emails<'_> {
        Emails(self)
    }
    pub fn inbound(&self) -> Inbound<'_> {
        Inbound(self)
    }
    pub fn domains(&self) -> Domains<'_> {
        Domains(self)
    }
    pub fn api_keys(&self) -> ApiKeys<'_> {
        ApiKeys(self)
    }
    pub fn templates(&self) -> Templates<'_> {
        Templates(self)
    }
    pub fn contacts(&self) -> Contacts<'_> {
        Contacts(self)
    }
    /// Segments (`/segments`).
    pub fn segments(&self) -> Groups<'_> {
        Groups(self, "/segments")
    }
    /// Audiences (`/audiences`), the older name of segments.
    pub fn audiences(&self) -> Groups<'_> {
        Groups(self, "/audiences")
    }
    pub fn properties(&self) -> Properties<'_> {
        Properties(self)
    }
    pub fn topics(&self) -> Topics<'_> {
        Topics(self)
    }
    pub fn broadcasts(&self) -> Broadcasts<'_> {
        Broadcasts(self)
    }
    pub fn webhooks(&self) -> Webhooks<'_> {
        Webhooks(self)
    }
    pub fn suppressions(&self) -> Suppressions<'_> {
        Suppressions(self)
    }
    /// Custom events (`order.paid`); email events are `emails().events(id)`.
    pub fn events(&self) -> Events<'_> {
        Events(self)
    }
    pub fn automations(&self) -> Automations<'_> {
        Automations(self)
    }
    pub fn inboxes(&self) -> Inboxes<'_> {
        Inboxes(self)
    }

    /// `GET /stats` for the last `days` days (`None` = server default, 30).
    pub fn stats(&self, days: Option<u32>) -> Result<Stats> {
        let path = with_query("/stats", vec![("days", days.map(|d| d.to_string()).unwrap_or_default())]);
        self.get(&path)
    }

    /// Resolves a domain's DMARC, SPF, DKIM, BIMI, MTA-STS and TLS-RPT records.
    pub fn dmarc_check(&self, domain: &str) -> Result<Value> {
        self.post("/tools/dmarc-check", &serde_json::json!({ "domain": domain }))
    }
}

// ------------------------------------------------------------------ emails

pub struct Emails<'a>(&'a Client);

impl Emails<'_> {
    /// Sends one email. Set `idempotency_key` on the email to make retries safe.
    pub fn send(&self, email: &SendEmail) -> Result<SendEmailResponse> {
        let mut opts = RequestOptions::new();
        if let Some(k) = &email.idempotency_key {
            opts = opts.idempotency_key(k.clone());
        }
        self.0.send_json("POST", "/emails", email, &opts)
    }

    /// Sends up to 100 emails (no attachments or `scheduled_at`). Use
    /// `RequestOptions::new().batch_validation("permissive")` to send the valid ones and get the
    /// rejected ones in `errors`.
    pub fn batch(&self, emails: &[SendEmail], opts: &RequestOptions) -> Result<BatchResponse> {
        self.0.send_json("POST", "/emails/batch", emails, opts)
    }

    pub fn get(&self, id: &str) -> Result<Email> {
        self.0.get(&format!("/emails/{}", enc(id)))
    }

    /// A page of emails, newest first.
    pub fn list(&self, params: &ListParams) -> Result<List<Email>> {
        self.0.get(&with_query("/emails", pairs_of(params)))
    }

    /// Reschedules a scheduled email (`scheduled_at` is RFC 3339 or `"in 1 hour"`).
    pub fn update(&self, id: &str, scheduled_at: &str) -> Result<IdResult> {
        self.0.patch(&format!("/emails/{}", enc(id)), &serde_json::json!({ "scheduled_at": scheduled_at }))
    }

    /// Cancels a scheduled email.
    pub fn cancel(&self, id: &str) -> Result<IdResult> {
        self.0.post_empty(&format!("/emails/{}/cancel", enc(id)))
    }

    /// The event history of an email, oldest first.
    pub fn events(&self, id: &str) -> Result<List<EmailEvent>> {
        self.0.get(&format!("/emails/{}/events", enc(id)))
    }

    pub fn attachments(&self, email_id: &str) -> Result<List<EmailAttachment>> {
        self.0.get(&format!("/emails/{}/attachments", enc(email_id)))
    }

    pub fn attachment(&self, email_id: &str, id: &str) -> Result<EmailAttachment> {
        self.0.get(&format!("/emails/{}/attachments/{}", enc(email_id), enc(id)))
    }

    /// Aggregated email counts (full-access key).
    pub fn metrics(&self, p: &EmailMetricsParams) -> Result<EmailMetrics> {
        let pairs = vec![
            ("start_date", opt(&p.start_date)),
            ("end_date", opt(&p.end_date)),
            ("timezone", opt(&p.timezone)),
            ("granularity", opt(&p.granularity)),
            ("metrics", csv(&p.metrics)),
            ("dimensions", csv(&p.dimensions)),
            ("domain_id", csv(&p.domain_ids)),
            ("email_id", csv(&p.email_ids)),
            ("broadcast_id", csv(&p.broadcast_ids)),
        ];
        self.0.get(&with_query("/emails/metrics", pairs))
    }

    /// A public read-only link to an email. `expires_in` is `"10m"`, `"2 hours"`, `"1 day"`...
    /// (`None` = 48 hours, also the maximum).
    pub fn share(&self, id: &str, expires_in: Option<&str>) -> Result<ShareResult> {
        let body = match expires_in {
            Some(e) => serde_json::json!({ "expires_in": e }),
            None => serde_json::json!({}),
        };
        self.0.post(&format!("/emails/{}/share", enc(id)), &body)
    }

    /// Sends the content to the signed-in user's own address with `[Test] ` in the subject.
    /// Dashboard session only: API keys get `403`.
    pub fn send_test(&self, p: &TestEmailParams) -> Result<TestEmailResult> {
        self.0.post("/emails/test", p)
    }

    /// Deliverability findings for a stored email.
    pub fn insights(&self, id: &str) -> Result<Insights> {
        self.0.get(&format!("/emails/{}/insights", enc(id)))
    }
}

pub struct Inbound<'a>(&'a Client);

impl Inbound<'_> {
    pub fn list(&self, params: &ListParams) -> Result<List<InboundEmail>> {
        self.0.get(&with_query("/inbound/emails", pairs_of(params)))
    }

    pub fn get(&self, id: &str) -> Result<InboundEmail> {
        self.0.get(&format!("/inbound/emails/{}", enc(id)))
    }
}

// ----------------------------------------------------------------- domains

pub struct Domains<'a>(&'a Client);

impl Domains<'_> {
    pub fn create(&self, p: &CreateDomain) -> Result<Domain> {
        self.0.post("/domains", p)
    }
    pub fn list(&self) -> Result<List<Domain>> {
        self.0.get("/domains")
    }
    pub fn get(&self, id: &str) -> Result<Domain> {
        self.0.get(&format!("/domains/{}", enc(id)))
    }
    pub fn update(&self, id: &str, p: &UpdateDomain) -> Result<Domain> {
        self.0.patch(&format!("/domains/{}", enc(id)), p)
    }
    /// Starts verification (async); poll `get` for the result.
    pub fn verify(&self, id: &str) -> Result<IdResult> {
        self.0.post_empty(&format!("/domains/{}/verify", enc(id)))
    }
    /// Publishes the domain's records in your Cloudflare zone. The token is used for this request
    /// only and never stored by usend.
    pub fn auto_configure(&self, id: &str, cloudflare_token: &str) -> Result<AutoConfigureResult> {
        self.0.post(
            &format!("/domains/{}/auto-configure", enc(id)),
            &serde_json::json!({ "provider": "cloudflare", "api_token": cloudflare_token }),
        )
    }
    pub fn remove(&self, id: &str) -> Result<DeleteResult> {
        self.0.delete(&format!("/domains/{}", enc(id)))
    }
    /// Claims a name another organization already verified (after `create` answered `409`).
    /// Publish the TXT record in `record`, then call `verify_claim`.
    pub fn claim(&self, p: &CreateDomain) -> Result<DomainClaim> {
        self.0.post("/domains/claim", p)
    }
    pub fn get_claim(&self, id: &str) -> Result<DomainClaim> {
        self.0.get(&format!("/domains/claim/{}", enc(id)))
    }
    pub fn verify_claim(&self, id: &str) -> Result<DomainClaim> {
        self.0.post_empty(&format!("/domains/claim/{}/verify", enc(id)))
    }
    /// Deliverability findings for a domain.
    pub fn insights(&self, id: &str) -> Result<Insights> {
        self.0.get(&format!("/domains/{}/insights", enc(id)))
    }
}

// ---------------------------------------------------------------- api keys

pub struct ApiKeys<'a>(&'a Client);

impl ApiKeys<'_> {
    /// Creates a key. Store `token` now; it cannot be read again.
    pub fn create(&self, p: &CreateApiKey) -> Result<CreateApiKeyResponse> {
        self.0.post("/api-keys", p)
    }
    pub fn list(&self) -> Result<List<ApiKey>> {
        self.0.get("/api-keys")
    }
    pub fn update(&self, id: &str, p: &UpdateApiKey) -> Result<IdResult> {
        self.0.patch(&format!("/api-keys/{}", enc(id)), p)
    }
    pub fn remove(&self, id: &str) -> Result<()> {
        self.0.delete_ok(&format!("/api-keys/{}", enc(id)))
    }
}

// --------------------------------------------------------------- templates

/// Methods take a template ID or alias.
pub struct Templates<'a>(&'a Client);

impl Templates<'_> {
    pub fn create(&self, p: &TemplateParams) -> Result<IdResult> {
        self.0.post("/templates", p)
    }
    pub fn list(&self, params: &ListParams) -> Result<List<Template>> {
        self.0.get(&with_query("/templates", pairs_of(params)))
    }
    pub fn get(&self, id_or_alias: &str) -> Result<Template> {
        self.0.get(&format!("/templates/{}", enc(id_or_alias)))
    }
    /// Edits the draft.
    pub fn update(&self, id_or_alias: &str, p: &TemplateParams) -> Result<IdResult> {
        self.0.patch(&format!("/templates/{}", enc(id_or_alias)), p)
    }
    /// Makes the draft the version that sends use.
    pub fn publish(&self, id_or_alias: &str) -> Result<Value> {
        self.0.post_empty(&format!("/templates/{}/publish", enc(id_or_alias)))
    }
    pub fn duplicate(&self, id_or_alias: &str) -> Result<IdResult> {
        self.0.post_empty(&format!("/templates/{}/duplicate", enc(id_or_alias)))
    }
    pub fn remove(&self, id_or_alias: &str) -> Result<DeleteResult> {
        self.0.delete(&format!("/templates/{}", enc(id_or_alias)))
    }
    /// Published versions, newest first.
    pub fn versions(&self, id_or_alias: &str) -> Result<List<TemplateVersion>> {
        self.0.get(&format!("/templates/{}/versions", enc(id_or_alias)))
    }
    pub fn get_version(&self, id_or_alias: &str, version_id: &str) -> Result<TemplateVersion> {
        self.0.get(&format!("/templates/{}/versions/{}", enc(id_or_alias), enc(version_id)))
    }
    /// Copies a version into the draft (it does not publish).
    pub fn restore_version(&self, id_or_alias: &str, version_id: &str) -> Result<Value> {
        self.0.post_empty(&format!("/templates/{}/versions/{}/restore", enc(id_or_alias), enc(version_id)))
    }
}

// ------------------------------------------------- segments and audiences

/// Segments or audiences: the same table under two names.
pub struct Groups<'a>(&'a Client, &'static str);

impl Groups<'_> {
    pub fn create(&self, name: &str) -> Result<Segment> {
        self.0.post(self.1, &serde_json::json!({ "name": name }))
    }
    pub fn list(&self) -> Result<List<Segment>> {
        self.0.get(self.1)
    }
    pub fn get(&self, id: &str) -> Result<Segment> {
        self.0.get(&format!("{}/{}", self.1, enc(id)))
    }
    /// Renames a segment.
    pub fn update(&self, id: &str, name: &str) -> Result<Segment> {
        self.0.patch(&format!("{}/{}", self.1, enc(id)), &serde_json::json!({ "name": name }))
    }
    /// Refused while a broadcast uses it.
    pub fn remove(&self, id: &str) -> Result<DeleteResult> {
        self.0.delete(&format!("{}/{}", self.1, enc(id)))
    }
    /// A page of contacts in the group. `subscription` is `subscribed` or `unsubscribed`.
    pub fn contacts(&self, id: &str, subscription: Option<&str>, params: &ListParams) -> Result<List<Contact>> {
        let mut pairs = pairs_of(params);
        pairs.push(("subscription", subscription.unwrap_or("").to_string()));
        self.0.get(&with_query(&format!("{}/{}/contacts", self.1, enc(id)), pairs))
    }
    pub fn metrics(&self, id: &str) -> Result<SegmentMetrics> {
        self.0.get(&format!("{}/{}/metrics", self.1, enc(id)))
    }
    // Audience-nested contact calls (use on `audiences()`).
    pub fn create_contact(&self, group_id: &str, p: &CreateContact) -> Result<IdResult> {
        self.0.post(&format!("{}/{}/contacts", self.1, enc(group_id)), p)
    }
    pub fn get_contact(&self, group_id: &str, id_or_email: &str) -> Result<Contact> {
        self.0.get(&format!("{}/{}/contacts/{}", self.1, enc(group_id), enc(id_or_email)))
    }
    pub fn update_contact(&self, group_id: &str, id_or_email: &str, p: &UpdateContact) -> Result<IdResult> {
        self.0.patch(&format!("{}/{}/contacts/{}", self.1, enc(group_id), enc(id_or_email)), p)
    }
    pub fn remove_contact(&self, group_id: &str, id_or_email: &str) -> Result<DeleteResult> {
        self.0.delete(&format!("{}/{}/contacts/{}", self.1, enc(group_id), enc(id_or_email)))
    }
    pub fn import_contacts(&self, group_id: &str, csv: &str) -> Result<ImportResult> {
        self.0.post(&format!("{}/{}/contacts/import", self.1, enc(group_id)), &serde_json::json!({ "csv": csv }))
    }
}

// ---------------------------------------------------------------- contacts

/// Account-wide contacts, one row per address. Methods take a contact ID or an email address.
pub struct Contacts<'a>(&'a Client);

impl Contacts<'_> {
    pub fn create(&self, p: &CreateContact) -> Result<IdResult> {
        self.0.post("/contacts", p)
    }
    pub fn list(&self, p: &ListContactsParams) -> Result<List<Contact>> {
        let mut pairs = pairs_of(&p.list);
        pairs.push(("segment_id", opt(&p.segment_id)));
        pairs.push(("topic_id", opt(&p.topic_id)));
        pairs.push(("subscription", opt(&p.subscription)));
        pairs.push(("q", opt(&p.q)));
        self.0.get(&with_query("/contacts", pairs))
    }
    pub fn stats(&self) -> Result<ContactStats> {
        self.0.get("/contacts/stats")
    }
    /// Synchronous CSV import into a segment (default `General`).
    pub fn import(&self, csv: &str, segment_id: Option<&str>) -> Result<ImportResult> {
        let mut body = serde_json::json!({ "csv": csv });
        if let Some(s) = segment_id {
            body["segment_id"] = Value::String(s.to_string());
        }
        self.0.post("/contacts/import", &body)
    }
    /// Starts a background import (up to 250,000 rows); poll `get_import`.
    pub fn create_import(&self, p: &CreateContactImport) -> Result<ContactImport> {
        self.0.post("/contacts/imports", p)
    }
    pub fn get_import(&self, id: &str) -> Result<ContactImport> {
        self.0.get(&format!("/contacts/imports/{}", enc(id)))
    }
    pub fn list_imports(&self, status: Option<&str>, params: &ListParams) -> Result<List<ContactImport>> {
        let mut pairs = pairs_of(params);
        pairs.push(("status", status.unwrap_or("").to_string()));
        self.0.get(&with_query("/contacts/imports", pairs))
    }
    pub fn get(&self, id_or_email: &str) -> Result<Contact> {
        self.0.get(&format!("/contacts/{}", enc(id_or_email)))
    }
    pub fn update(&self, id_or_email: &str, p: &UpdateContact) -> Result<IdResult> {
        self.0.patch(&format!("/contacts/{}", enc(id_or_email)), p)
    }
    pub fn remove(&self, id_or_email: &str) -> Result<DeleteResult> {
        self.0.delete(&format!("/contacts/{}", enc(id_or_email)))
    }
    pub fn segments(&self, id_or_email: &str) -> Result<List<Segment>> {
        self.0.get(&format!("/contacts/{}/segments", enc(id_or_email)))
    }
    pub fn add_to_segment(&self, id_or_email: &str, segment_id: &str) -> Result<()> {
        self.0.post_empty_ok(&format!("/contacts/{}/segments/{}", enc(id_or_email), enc(segment_id)))
    }
    pub fn remove_from_segment(&self, id_or_email: &str, segment_id: &str) -> Result<()> {
        self.0.delete_ok(&format!("/contacts/{}/segments/{}", enc(id_or_email), enc(segment_id)))
    }
    /// A contact's topic subscriptions.
    pub fn topics(&self, id_or_email: &str) -> Result<Value> {
        // The API answers with a bare array or a list envelope; keep it generic.
        self.0.get(&format!("/contacts/{}/topics", enc(id_or_email)))
    }
    pub fn update_topics(&self, id_or_email: &str, topics: &[TopicChoice]) -> Result<Value> {
        self.0.patch(&format!("/contacts/{}/topics", enc(id_or_email)), &serde_json::json!({ "topics": topics }))
    }
}

pub struct Properties<'a>(&'a Client);

impl Properties<'_> {
    pub fn list(&self) -> Result<List<ContactProperty>> {
        self.0.get("/contact-properties")
    }
    pub fn get(&self, id: &str) -> Result<ContactProperty> {
        self.0.get(&format!("/contact-properties/{}", enc(id)))
    }
    pub fn create(&self, p: &CreateContactProperty) -> Result<ContactProperty> {
        self.0.post("/contact-properties", p)
    }
    /// Key and type cannot change; only the fallback value.
    pub fn update_fallback(&self, id: &str, fallback: Value) -> Result<ContactProperty> {
        self.0.patch(&format!("/contact-properties/{}", enc(id)), &serde_json::json!({ "fallback_value": fallback }))
    }
    /// Removes the property and its values from every contact.
    pub fn remove(&self, id: &str) -> Result<()> {
        self.0.delete_ok(&format!("/contact-properties/{}", enc(id)))
    }
}

pub struct Topics<'a>(&'a Client);

impl Topics<'_> {
    pub fn list(&self) -> Result<List<Topic>> {
        self.0.get("/topics")
    }
    pub fn get(&self, id: &str) -> Result<Topic> {
        self.0.get(&format!("/topics/{}", enc(id)))
    }
    pub fn create(&self, p: &TopicParams) -> Result<Topic> {
        self.0.post("/topics", p)
    }
    pub fn update(&self, id: &str, p: &TopicParams) -> Result<Topic> {
        self.0.patch(&format!("/topics/{}", enc(id)), p)
    }
    /// Refused while a broadcast uses it.
    pub fn remove(&self, id: &str) -> Result<()> {
        self.0.delete_ok(&format!("/topics/{}", enc(id)))
    }
}

// -------------------------------------------------------------- broadcasts

pub struct Broadcasts<'a>(&'a Client);

impl Broadcasts<'_> {
    pub fn create(&self, p: &BroadcastParams) -> Result<SendEmailResponse> {
        self.0.post("/broadcasts", p)
    }
    pub fn list(&self) -> Result<List<Broadcast>> {
        self.0.get("/broadcasts")
    }
    pub fn get(&self, id: &str) -> Result<Broadcast> {
        self.0.get(&format!("/broadcasts/{}", enc(id)))
    }
    /// Edits a draft.
    pub fn update(&self, id: &str, p: &BroadcastParams) -> Result<Value> {
        self.0.patch(&format!("/broadcasts/{}", enc(id)), p)
    }
    /// Sends now, or at `scheduled_at` (RFC 3339).
    pub fn send(&self, id: &str, scheduled_at: Option<&str>) -> Result<Value> {
        let body = match scheduled_at {
            Some(t) => serde_json::json!({ "scheduled_at": t }),
            None => serde_json::json!({}),
        };
        self.0.post(&format!("/broadcasts/{}/send", enc(id)), &body)
    }
    /// Sends one copy to a single address.
    pub fn test(&self, id: &str, to: &str) -> Result<Value> {
        self.0.post(&format!("/broadcasts/{}/test", enc(id)), &serde_json::json!({ "to": to }))
    }
    /// Deletes a draft.
    pub fn remove(&self, id: &str) -> Result<()> {
        self.0.delete_ok(&format!("/broadcasts/{}", enc(id)))
    }
    /// Stops a queued, sending or paused broadcast.
    pub fn cancel(&self, id: &str) -> Result<Value> {
        self.0.post_empty(&format!("/broadcasts/{}/cancel", enc(id)))
    }
    /// Copies a broadcast into a new draft.
    pub fn duplicate(&self, id: &str) -> Result<IdResult> {
        self.0.post_empty(&format!("/broadcasts/{}/duplicate", enc(id)))
    }
    /// Recipients who had an event. The event type is required.
    pub fn recipients(&self, id: &str, kind: RecipientType, p: &BroadcastRecipientsParams) -> Result<List<BroadcastRecipient>> {
        let mut pairs = pairs_of(&p.list);
        pairs.push(("type", kind.as_str().to_string()));
        pairs.push(("email", opt(&p.email)));
        pairs.push(("bounce_type", opt(&p.bounce_type)));
        self.0.get(&with_query(&format!("/broadcasts/{}/recipients", enc(id)), pairs))
    }
    /// Links clicked in a broadcast, most clicked first.
    pub fn clicked_links(&self, id: &str) -> Result<List<BroadcastClickedLink>> {
        self.0.get(&format!("/broadcasts/{}/clicked-links", enc(id)))
    }
    /// Counts, rates and an hourly open/click timeline.
    pub fn performance(&self, id: &str) -> Result<BroadcastPerformance> {
        self.0.get(&format!("/broadcasts/{}/performance", enc(id)))
    }
}

// ---------------------------------------------------------------- webhooks

pub struct Webhooks<'a>(&'a Client);

impl Webhooks<'_> {
    pub fn create(&self, p: &CreateWebhook) -> Result<Webhook> {
        self.0.post("/webhooks", p)
    }
    pub fn list(&self) -> Result<List<Webhook>> {
        self.0.get("/webhooks")
    }
    pub fn get(&self, id: &str) -> Result<Webhook> {
        self.0.get(&format!("/webhooks/{}", enc(id)))
    }
    pub fn update(&self, id: &str, p: &UpdateWebhook) -> Result<Webhook> {
        self.0.patch(&format!("/webhooks/{}", enc(id)), p)
    }
    pub fn remove(&self, id: &str) -> Result<()> {
        self.0.delete_ok(&format!("/webhooks/{}", enc(id)))
    }
    pub fn deliveries(&self, id: &str, params: &ListParams) -> Result<List<WebhookDelivery>> {
        self.0.get(&with_query(&format!("/webhooks/{}/deliveries", enc(id)), pairs_of(params)))
    }
    pub fn replay(&self, id: &str, delivery_id: &str) -> Result<()> {
        self.0.post_empty_ok(&format!("/webhooks/{}/deliveries/{}/replay", enc(id), enc(delivery_id)))
    }
    /// Issues a new signing secret; the old one stops signing at once.
    pub fn rotate_secret(&self, id: &str) -> Result<Webhook> {
        self.0.post_empty(&format!("/webhooks/{}/rotate-secret", enc(id)))
    }
    /// Deliveries across all endpoints.
    pub fn events(&self, p: &WebhookEventsParams) -> Result<List<WebhookEventRecord>> {
        let mut pairs = pairs_of(&p.list);
        pairs.push(("webhook_id", opt(&p.webhook_id)));
        pairs.push(("type", opt(&p.event_type)));
        pairs.push(("status", opt(&p.status)));
        self.0.get(&with_query("/webhooks/events", pairs))
    }
    /// One delivery with the payload that is POSTed.
    pub fn get_event(&self, id: &str) -> Result<WebhookEventRecord> {
        self.0.get(&format!("/webhooks/events/{}", enc(id)))
    }
    pub fn replay_event(&self, id: &str) -> Result<IdResult> {
        self.0.post_empty(&format!("/webhooks/events/{}/replay", enc(id)))
    }
    pub fn event_attempts(&self, id: &str) -> Result<List<WebhookAttempt>> {
        self.0.get(&format!("/webhooks/events/{}/attempts", enc(id)))
    }
}

// ------------------------------------------------------------ suppressions

pub struct Suppressions<'a>(&'a Client);

impl Suppressions<'_> {
    pub fn list(&self, params: &ListParams) -> Result<List<Suppression>> {
        self.0.get(&with_query("/suppressions", pairs_of(params)))
    }
    /// By ID or email address.
    pub fn get(&self, id_or_email: &str) -> Result<Suppression> {
        self.0.get(&format!("/suppressions/{}", enc(id_or_email)))
    }
    pub fn create(&self, email: &str) -> Result<Suppression> {
        self.0.post("/suppressions", &serde_json::json!({ "email": email }))
    }
    /// Up to 100 addresses.
    pub fn batch_create(&self, emails: &[String]) -> Result<List<Suppression>> {
        self.0.post("/suppressions/batch", &serde_json::json!({ "emails": emails }))
    }
    /// Unknown addresses are skipped.
    pub fn batch_remove(&self, emails: &[String]) -> Result<List<DeleteResult>> {
        self.0.send_json("DELETE", "/suppressions/batch", &serde_json::json!({ "emails": emails }), &RequestOptions::default())
    }
    pub fn remove(&self, id_or_email: &str) -> Result<()> {
        self.0.delete_ok(&format!("/suppressions/{}", enc(id_or_email)))
    }
}

// ------------------------------------------------------------------ events

pub struct Events<'a>(&'a Client);

impl Events<'_> {
    pub fn create(&self, p: &CreateEvent) -> Result<IdResult> {
        self.0.post("/events", p)
    }
    pub fn list(&self, params: &ListParams) -> Result<List<CustomEvent>> {
        self.0.get(&with_query("/events", pairs_of(params)))
    }
    /// By ID or name.
    pub fn get(&self, id_or_name: &str) -> Result<CustomEvent> {
        self.0.get(&format!("/events/{}", enc(id_or_name)))
    }
    pub fn update(&self, id: &str, p: &UpdateEvent) -> Result<CustomEvent> {
        self.0.patch(&format!("/events/{}", enc(id)), p)
    }
    /// Refused (`409`) while an automation uses it.
    pub fn remove(&self, id: &str) -> Result<DeleteResult> {
        self.0.delete(&format!("/events/{}", enc(id)))
    }
    /// Fires an event for a contact. The API answers `202`.
    pub fn send(&self, e: &SendEvent) -> Result<SendEventResult> {
        let mut opts = RequestOptions::new();
        if let Some(k) = &e.idempotency_key {
            opts = opts.idempotency_key(k.clone());
        }
        self.0.send_json("POST", "/events/send", e, &opts)
    }
}

// ------------------------------------------------------------- automations

pub struct Automations<'a>(&'a Client);

impl Automations<'_> {
    pub fn create(&self, p: &AutomationParams) -> Result<IdResult> {
        self.0.post("/automations", p)
    }
    /// `status` filters (`draft`, `enabled`, `disabled`).
    pub fn list(&self, status: Option<&str>, params: &ListParams) -> Result<List<Automation>> {
        let mut pairs = pairs_of(params);
        pairs.push(("status", status.unwrap_or("").to_string()));
        self.0.get(&with_query("/automations", pairs))
    }
    pub fn get(&self, id: &str) -> Result<Automation> {
        self.0.get(&format!("/automations/{}", enc(id)))
    }
    /// A change to the graph raises `version`.
    pub fn update(&self, id: &str, p: &AutomationParams) -> Result<IdResult> {
        self.0.patch(&format!("/automations/{}", enc(id)), p)
    }
    pub fn remove(&self, id: &str) -> Result<()> {
        self.0.delete_ok(&format!("/automations/{}", enc(id)))
    }
    pub fn duplicate(&self, id: &str) -> Result<IdResult> {
        self.0.post_empty(&format!("/automations/{}/duplicate", enc(id)))
    }
    /// Halts every active run. Update the status to `enabled` or `disabled` to bring it back.
    pub fn stop(&self, id: &str) -> Result<StopResult> {
        self.0.post_empty(&format!("/automations/{}/stop", enc(id)))
    }
    /// `status`: `running`, `completed`, `failed` or `cancelled`.
    pub fn runs(&self, id: &str, status: Option<&str>, params: &ListParams) -> Result<List<AutomationRun>> {
        let mut pairs = pairs_of(params);
        pairs.push(("status", status.unwrap_or("").to_string()));
        self.0.get(&with_query(&format!("/automations/{}/runs", enc(id)), pairs))
    }
    /// A run with its steps.
    pub fn get_run(&self, id: &str, run_id: &str) -> Result<AutomationRun> {
        self.0.get(&format!("/automations/{}/runs/{}", enc(id), enc(run_id)))
    }
}

// ----------------------------------------------------------------- inboxes

/// Shared mailboxes (full-access keys and sessions only).
pub struct Inboxes<'a>(&'a Client);

fn ib(id: &str) -> String {
    format!("/inboxes/{}", enc(id))
}

impl Inboxes<'_> {
    pub fn create(&self, p: &CreateInbox) -> Result<Inbox> {
        self.0.post("/inboxes", p)
    }
    pub fn list(&self, params: &ListParams) -> Result<List<Inbox>> {
        self.0.get(&with_query("/inboxes", pairs_of(params)))
    }
    pub fn get(&self, id: &str) -> Result<Inbox> {
        self.0.get(&ib(id))
    }
    pub fn update(&self, id: &str, p: &UpdateInbox) -> Result<Inbox> {
        self.0.patch(&ib(id), p)
    }
    pub fn remove(&self, id: &str) -> Result<DeleteResult> {
        self.0.delete(&ib(id))
    }

    pub fn create_label(&self, inbox_id: &str, p: &LabelParams) -> Result<Label> {
        self.0.post(&format!("{}/labels", ib(inbox_id)), p)
    }
    pub fn labels(&self, inbox_id: &str) -> Result<List<Label>> {
        self.0.get(&format!("{}/labels", ib(inbox_id)))
    }
    pub fn update_label(&self, inbox_id: &str, label_id: &str, p: &LabelParams) -> Result<Label> {
        self.0.patch(&format!("{}/labels/{}", ib(inbox_id), enc(label_id)), p)
    }
    pub fn remove_label(&self, inbox_id: &str, label_id: &str) -> Result<()> {
        self.0.delete_ok(&format!("{}/labels/{}", ib(inbox_id), enc(label_id)))
    }

    pub fn threads(&self, inbox_id: &str, p: &ThreadListParams) -> Result<List<Thread>> {
        let mut pairs = pairs_of(&p.list);
        pairs.push(("folder", opt(&p.folder)));
        pairs.push(("label", opt(&p.label)));
        pairs.push(("assignee", opt(&p.assignee)));
        pairs.push(("unread", if p.unread { "true".to_string() } else { String::new() }));
        pairs.push(("q", opt(&p.q)));
        self.0.get(&with_query(&format!("{}/threads", ib(inbox_id)), pairs))
    }
    pub fn thread(&self, inbox_id: &str, thread_id: &str) -> Result<Thread> {
        self.0.get(&format!("{}/threads/{}", ib(inbox_id), enc(thread_id)))
    }
    pub fn update_thread(&self, inbox_id: &str, thread_id: &str, p: &UpdateThread) -> Result<Thread> {
        self.0.patch(&format!("{}/threads/{}", ib(inbox_id), enc(thread_id)), p)
    }
    pub fn remove_thread(&self, inbox_id: &str, thread_id: &str) -> Result<()> {
        self.0.delete_ok(&format!("{}/threads/{}", ib(inbox_id), enc(thread_id)))
    }
    /// The messages of a thread, newest first.
    pub fn emails(&self, inbox_id: &str, thread_id: &str, params: &ListParams) -> Result<List<InboxEmail>> {
        self.0.get(&with_query(&format!("{}/threads/{}/emails", ib(inbox_id), enc(thread_id)), pairs_of(params)))
    }
    /// One message with its body and attachments.
    pub fn get_email(&self, inbox_id: &str, thread_id: &str, email_id: &str) -> Result<InboxEmail> {
        self.0.get(&format!("{}/threads/{}/emails/{}", ib(inbox_id), enc(thread_id), enc(email_id)))
    }
    /// Answers a message through the normal sending pipeline.
    pub fn reply(&self, inbox_id: &str, thread_id: &str, email_id: &str, p: &ReplyParams) -> Result<InboxSendResult> {
        self.0.post(&format!("{}/threads/{}/emails/{}/reply", ib(inbox_id), enc(thread_id), enc(email_id)), p)
    }
    /// Forwards a message with its attachments.
    pub fn forward(&self, inbox_id: &str, thread_id: &str, email_id: &str, p: &ForwardParams) -> Result<InboxSendResult> {
        self.0.post(&format!("{}/threads/{}/emails/{}/forward", ib(inbox_id), enc(thread_id), enc(email_id)), p)
    }

    pub fn create_draft(&self, inbox_id: &str, p: &DraftParams) -> Result<Draft> {
        self.0.post(&format!("{}/drafts", ib(inbox_id)), p)
    }
    pub fn drafts(&self, inbox_id: &str, params: &ListParams) -> Result<List<Draft>> {
        self.0.get(&with_query(&format!("{}/drafts", ib(inbox_id)), pairs_of(params)))
    }
    pub fn get_draft(&self, inbox_id: &str, draft_id: &str) -> Result<Draft> {
        self.0.get(&format!("{}/drafts/{}", ib(inbox_id), enc(draft_id)))
    }
    pub fn update_draft(&self, inbox_id: &str, draft_id: &str, p: &DraftParams) -> Result<Draft> {
        self.0.patch(&format!("{}/drafts/{}", ib(inbox_id), enc(draft_id)), p)
    }
    pub fn remove_draft(&self, inbox_id: &str, draft_id: &str) -> Result<()> {
        self.0.delete_ok(&format!("{}/drafts/{}", ib(inbox_id), enc(draft_id)))
    }
    /// Sends and deletes the draft; one that fails validation stays.
    pub fn send_draft(&self, inbox_id: &str, draft_id: &str) -> Result<InboxSendResult> {
        self.0.post_empty(&format!("{}/drafts/{}/send", ib(inbox_id), enc(draft_id)))
    }
}
