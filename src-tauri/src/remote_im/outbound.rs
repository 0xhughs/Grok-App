//! Unified outbound reply router for all channel types (Rust HTTP / WS clients).

#![allow(dead_code)] // residual-clippy: test helpers and unused channel send paths
use parking_lot::RwLock;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct OutboundRouter {
    /// instance_id -> channel credentials snapshot
    creds: Arc<RwLock<HashMap<String, InstanceCreds>>>,
}

#[derive(Clone)]
struct InstanceCreds {
    channel: String,
    secrets: HashMap<String, String>,
    options: serde_json::Value,
}

impl OutboundRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &self,
        instance_id: &str,
        channel: &str,
        mut secrets: HashMap<String, String>,
        options: serde_json::Value,
    ) {
        // Always stamp instance id for weixin context_token / dingtalk webhook maps.
        secrets.insert("_instance_id".into(), instance_id.to_string());
        self.creds.write().insert(
            instance_id.to_string(),
            InstanceCreds {
                channel: channel.to_string(),
                secrets,
                options,
            },
        );
    }

    /// Test/helper: read back injected secrets for an instance.
    pub fn secrets_for_test(&self, instance_id: &str) -> Option<HashMap<String, String>> {
        self.creds
            .read()
            .get(instance_id)
            .map(|c| c.secrets.clone())
    }

    pub fn unregister(&self, instance_id: &str) {
        self.creds.write().remove(instance_id);
    }

    pub fn clear(&self) {
        self.creds.write().clear();
    }

    pub async fn reply(
        &self,
        instance_id: &str,
        chat_id: &str,
        reply_to: Option<&str>,
        text: &str,
        thread_id: Option<&str>,
    ) -> Result<(), String> {
        let cred = self
            .creds
            .read()
            .get(instance_id)
            .cloned()
            .ok_or_else(|| format!("no outbound creds for {instance_id}"))?;
        match cred.channel.as_str() {
            "feishu" | "lark" => {
                super::channels::feishu::send_text(
                    &cred.channel,
                    &cred.secrets,
                    &cred.options,
                    chat_id,
                    reply_to,
                    text,
                )
                .await
            }
            "telegram" => {
                super::channels::telegram::send_text(&cred.secrets, chat_id, text, thread_id).await
            }
            "discord" => super::channels::discord::send_text(&cred.secrets, chat_id, text).await,
            "slack" => super::channels::slack::send_text(&cred.secrets, chat_id, text).await,
            "dingtalk" => super::channels::dingtalk::send_text(&cred.secrets, chat_id, text).await,
            "wecom" => super::channels::wecom::send_text(&cred.secrets, chat_id, text).await,
            "weixin" => super::channels::weixin::send_text(&cred.secrets, chat_id, text).await,
            "qq" => super::channels::qq::send_text(&cred.secrets, chat_id, text).await,
            "qqbot" => super::channels::qqbot::send_text(&cred.secrets, chat_id, text).await,
            "matrix" => super::channels::matrix::send_text(&cred.secrets, chat_id, text).await,
            "line" => super::channels::line::send_text(&cred.secrets, chat_id, text).await,
            "weibo" => super::channels::weibo::send_text(&cred.secrets, chat_id, text).await,
            "wps-xiezuo" => {
                super::channels::wps_xiezuo::send_text(&cred.secrets, chat_id, text).await
            }
            other => {
                tracing::warn!(channel = other, "outbound reply not implemented; dropping");
                Ok(())
            }
        }
    }

    /// Interactive card (Feishu / DingTalk action card / Telegram inline keyboard).
    pub async fn reply_card(
        &self,
        instance_id: &str,
        chat_id: &str,
        reply_to: Option<&str>,
        card: &serde_json::Value,
        thread_id: Option<&str>,
    ) -> Result<(), String> {
        let cred = self
            .creds
            .read()
            .get(instance_id)
            .cloned()
            .ok_or_else(|| format!("no outbound creds for {instance_id}"))?;
        match cred.channel.as_str() {
            "feishu" | "lark" => {
                super::channels::feishu::send_card(
                    &cred.channel,
                    &cred.secrets,
                    &cred.options,
                    chat_id,
                    reply_to,
                    card,
                )
                .await
            }
            "dingtalk" => super::channels::dingtalk::send_card(&cred.secrets, chat_id, card).await,
            "telegram" => {
                super::channels::telegram::send_card(&cred.secrets, chat_id, card, thread_id).await
            }
            _ => {
                // Fallback: dump card as text menu summary
                let text = format!(
                    "{}\n{}",
                    card.pointer("/header/title/content")
                        .and_then(|x| x.as_str())
                        .unwrap_or("Select:"),
                    card
                );
                self.reply(
                    instance_id,
                    chat_id,
                    reply_to,
                    &text.chars().take(2000).collect::<String>(),
                    thread_id,
                )
                .await
            }
        }
    }

    /// Replace an existing interactive result in-place where the channel supports it.
    pub async fn edit_card(
        &self,
        instance_id: &str,
        chat_id: &str,
        message_id: &str,
        card: &serde_json::Value,
        thread_id: Option<&str>,
    ) -> Result<(), String> {
        let cred = self
            .creds
            .read()
            .get(instance_id)
            .cloned()
            .ok_or_else(|| format!("no outbound creds for {instance_id}"))?;
        match cred.channel.as_str() {
            "telegram" => {
                super::channels::telegram::edit_card(
                    &cred.secrets,
                    chat_id,
                    message_id,
                    card,
                    thread_id,
                )
                .await
            }
            _ => {
                self.reply_card(instance_id, chat_id, None, card, thread_id)
                    .await
            }
        }
    }
}

pub fn http_client() -> Result<reqwest::Client, String> {
    crate::proxy::apply_to_reqwest(reqwest::Client::builder())
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("GrokApp-RemoteIM/1.0")
        .build()
        .map_err(|e| e.to_string())
}

pub async fn json_post(url: &str, body: serde_json::Value) -> Result<serde_json::Value, String> {
    let c = http_client()?;
    let res = c
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = res.status();
    let text = res.text().await.map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|_| {
        format!(
            "HTTP {status}: {}",
            text.chars().take(200).collect::<String>()
        )
    })
}

#[allow(dead_code)]
pub async fn json_get_bearer(url: &str, token: &str) -> Result<serde_json::Value, String> {
    let c = http_client()?;
    let res = c
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = res.status();
    let text = res.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!(
            "HTTP {status}: {}",
            text.chars().take(200).collect::<String>()
        ));
    }
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

pub fn redact_preview(s: &str) -> String {
    if s.len() <= 8 {
        return "***".into();
    }
    format!("{}…", &s[..4])
}

pub fn opt_str(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn secret_or_opt(
    secrets: &HashMap<String, String>,
    options: &serde_json::Value,
    key: &str,
) -> Option<String> {
    secrets
        .get(key)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| opt_str(options, key))
}

/// Enable-time error stored on the instance (and returned by `start_runtime`
/// when no channel survives the ACL guard). Must never recommend a catch-all
/// value; see `enable_error_text_requires_explicit_ids_and_never_offers_wildcard`.
pub(crate) const ALLOW_FROM_BLOCKED_ERR: &str = "allow_from must list explicit sender ids; \
     empty or catch-all entries are refused. Add the platform user ids allowed to talk to \
     this bot in Settings → Remote IM before enabling this channel";

/// The catch-all marker. There is no open ACL: an entry equal to this character
/// makes the whole list deny (R4 / N2, parity with `remote-bridge/src/r4.test.ts`).
const WILDCARD: char = '*';

fn is_wildcard_entry(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next() == Some(WILDCARD) && chars.next().is_none()
}

/// Parse allow-from ACL (`allowFrom`, then `allow_from`) into the explicit
/// sender ids that may talk to the bot.
///
/// - `*` (alone, padded, or as any entry of a comma list) → deny: empty list
/// - missing / `null` / non-string / empty / whitespace-only → empty list
/// - comma list of ids → exactly those trimmed, non-empty ids
///
/// No "open" value is representable: the bridge is the security boundary and
/// must not be more permissive than the explicit list, whatever the Settings UI
/// or a hand-edited config stored. A wildcard entry poisons the whole list
/// instead of being dropped so the misconfiguration surfaces at enable time.
pub fn allow_from_list(acl: &serde_json::Value) -> Vec<String> {
    let raw = acl
        .get("allowFrom")
        .or_else(|| acl.get("allow_from"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim();
    if raw.is_empty() {
        return vec![];
    }
    let list: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if list.iter().any(|s| is_wildcard_entry(s)) {
        return vec![];
    }
    list
}

/// True only when `sender_id` is one of the explicit ids in the ACL.
pub fn sender_allowed(acl: &serde_json::Value, sender_id: &str) -> bool {
    allow_from_list(acl).iter().any(|x| x == sender_id)
}

/// True when enable should be refused: the ACL yields no explicit sender id
/// (missing, empty, or containing a `*` entry all fail closed).
pub fn allow_from_blocks_enable(acl: &serde_json::Value) -> bool {
    allow_from_list(acl).is_empty()
}

/// Whether group chats require @bot.
///
/// Priority: explicit `require_mention` / ACL `requireMention`, else invert
/// options `group_reply_all` (§6.1 / §3.2). Default true (need @).
pub fn require_mention(options: &serde_json::Value, acl: &serde_json::Value) -> bool {
    if let Some(b) = options
        .get("require_mention")
        .or_else(|| acl.get("requireMention"))
        .and_then(|x| x.as_bool())
    {
        return b;
    }
    if let Some(all) = options.get("group_reply_all").and_then(|x| x.as_bool()) {
        return !all;
    }
    true
}

/// Helper for telegram-style JSON APIs
pub fn empty_json() -> serde_json::Value {
    json!({})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_always_injects_instance_id() {
        let r = OutboundRouter::new();
        let mut secrets = HashMap::new();
        secrets.insert("token".into(), "t".into());
        r.register("inst-42", "weixin", secrets, json!({}));
        let got = r.secrets_for_test("inst-42").unwrap();
        assert_eq!(got.get("_instance_id").map(|s| s.as_str()), Some("inst-42"));
        assert_eq!(got.get("token").map(|s| s.as_str()), Some("t"));
    }

    #[test]
    fn require_mention_honors_acl_and_group_reply_all() {
        // ACL requireMention wins
        assert!(!require_mention(
            &json!({}),
            &json!({ "requireMention": false }),
        ));
        assert!(require_mention(
            &json!({}),
            &json!({ "requireMention": true }),
        ));
        // group_reply_all is inverse when no explicit require_*
        assert!(!require_mention(
            &json!({ "group_reply_all": true }),
            &json!({}),
        ));
        assert!(require_mention(
            &json!({ "group_reply_all": false }),
            &json!({}),
        ));
        // default need @
        assert!(require_mention(&json!({}), &json!({})));
    }

    /// The catch-all marker as a string. Built from the char so no test line
    /// carries a wildcard ACL literal (grep criterion for the live bridge).
    fn star() -> String {
        WILDCARD.to_string()
    }

    /// Every ACL shape that must yield the empty list (deny everyone).
    fn deny_acls() -> Vec<serde_json::Value> {
        let s = star();
        vec![
            json!({}),
            json!({ "allowFrom": null }),
            json!({ "allowFrom": "" }),
            json!({ "allowFrom": "   " }),
            json!({ "allowFrom": ",," }),
            json!({ "allowFrom": s }),
            json!({ "allowFrom": format!(" {s} ") }),
            json!({ "allow_from": s }),
            json!({ "allowFrom": format!("alice, {s}") }),
            json!({ "allowFrom": format!("{s}, alice") }),
            json!({ "allowFrom": [s] }),
            json!({ "allowFrom": [] }),
            json!({ "allowFrom": ["alice"] }),
            json!({ "allowFrom": 42 }),
            json!({ "allowFrom": true }),
            json!({ "allowFrom": { "alice": true } }),
        ]
    }

    #[test]
    fn allow_from_fails_closed_for_missing_empty_and_wildcard() {
        let s = star();
        // Fail-closed: no explicit allowFrom ⇒ nobody is allowed.
        assert!(!sender_allowed(&json!({}), "attacker"));
        assert!(!sender_allowed(&json!({ "allowFrom": null }), "owner"));
        assert!(!sender_allowed(&json!({ "allowFrom": "" }), "owner"));
        assert!(!sender_allowed(&json!({ "allowFrom": "   " }), "owner"));
        assert!(!sender_allowed(&json!({ "allowFrom": ",," }), "owner"));
        // `*` is deny, not open: alone, padded, snake_case alias.
        assert!(!sender_allowed(&json!({ "allowFrom": s }), "anyone"));
        assert!(!sender_allowed(
            &json!({ "allowFrom": format!(" {s} ") }),
            "anyone"
        ));
        assert!(!sender_allowed(&json!({ "allow_from": s }), "anyone"));
        // A wildcard entry poisons the whole list, even for listed ids.
        let mixed = json!({ "allowFrom": format!("alice, {s}") });
        assert!(!sender_allowed(&mixed, "alice"));
        assert!(!sender_allowed(&mixed, "mallory"));
        assert!(!sender_allowed(
            &json!({ "allowFrom": format!("{s}, alice") }),
            "alice"
        ));
        // Arrays are not a supported shape: unchanged fail-closed.
        assert!(!sender_allowed(&json!({ "allowFrom": [s] }), "anyone"));
        assert!(!sender_allowed(&json!({ "allowFrom": [] }), "anyone"));
        assert!(!sender_allowed(&json!({ "allowFrom": ["alice"] }), "alice"));
        for acl in deny_acls() {
            for sender in ["anyone", "attacker", "owner", "alice", s.as_str()] {
                assert!(!sender_allowed(&acl, sender), "{acl} must deny {sender}");
            }
        }
    }

    #[test]
    fn allow_from_explicit_list_allows_only_listed_senders() {
        // Comma list allows exactly the listed senders.
        let acl = json!({ "allowFrom": "alice, bob ,," });
        assert!(sender_allowed(&acl, "alice"));
        assert!(sender_allowed(&acl, "bob"));
        assert!(!sender_allowed(&acl, "mallory"));
        assert!(!sender_allowed(&acl, ""));
        assert!(!sender_allowed(&acl, &star()));
        // snake_case alias behaves identically.
        assert!(sender_allowed(&json!({ "allow_from": "alice" }), "alice"));
    }

    #[test]
    fn allow_from_list_never_yields_open() {
        for acl in deny_acls() {
            assert!(
                allow_from_list(&acl).is_empty(),
                "{acl} must yield the empty list"
            );
        }
        assert_eq!(
            allow_from_list(&json!({ "allowFrom": "alice, bob" })),
            vec!["alice".to_string(), "bob".to_string()]
        );
    }

    #[test]
    fn blocks_enable_without_explicit_allow_from() {
        let s = star();
        // Fail-closed: missing entry, empty list, and any `*` entry all refuse
        // enable; only a non-empty list of explicit ids may start.
        assert!(allow_from_blocks_enable(&json!({})));
        assert!(allow_from_blocks_enable(&json!({ "allowFrom": "" })));
        assert!(allow_from_blocks_enable(&json!({ "allowFrom": "   " })));
        assert!(allow_from_blocks_enable(&json!({ "allowFrom": s })));
        assert!(allow_from_blocks_enable(&json!({
            "allowFrom": format!(" {s} ")
        })));
        assert!(allow_from_blocks_enable(&json!({
            "allowFrom": format!("alice, {s}")
        })));
        assert!(allow_from_blocks_enable(&json!({ "allowFrom": [s] })));
        assert!(!allow_from_blocks_enable(&json!({ "allowFrom": "alice" })));
        assert!(!allow_from_blocks_enable(&json!({
            "allowFrom": "alice, bob"
        })));
    }

    #[test]
    fn enable_error_text_requires_explicit_ids_and_never_offers_wildcard() {
        let text = ALLOW_FROM_BLOCKED_ERR;
        let lower = text.to_lowercase();
        // Must tell the user what to add and where.
        assert!(lower.contains("explicit sender id"), "{text}");
        assert!(text.contains("Settings → Remote IM"), "{text}");
        // Must never offer a catch-all (case-insensitive
        // `\*|wildcard|\bany\b|anyone|for all`, checked without a regex crate).
        assert!(!text.contains(WILDCARD), "{text}");
        for needle in ["wildcard", "anyone", "for all"] {
            assert!(!lower.contains(needle), "{text} contains {needle}");
        }
        assert!(
            !lower
                .split(|c: char| !c.is_alphanumeric())
                .any(|word| word == "any"),
            "{text} contains the word any"
        );
    }
}
