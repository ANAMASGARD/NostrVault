//! Signer policy and protocol state shared by every host. No sockets or UI objects.
use crate::vault::{random, Entropy, Error, Result};
use nostr::{
    event::{Event, UnsignedEvent},
    key::{Keys, PublicKey, SecretKey},
    nips::{nip04, nip44},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use zeroize::{Zeroize, Zeroizing};

pub const WIRE_LIMIT: usize = 262_144;
pub const RECORD_NAME: &str = "identity-v1";
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Adapter {
    Browser,
    Remote,
    Android,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capabilities {
    pub public_key: bool,
    pub nip04: Option<bool>,
    pub nip44: Option<bool>,
    pub relay_auth: Option<bool>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Grants {
    pub readable: bool,
    pub relay_auth: Vec<String>,
    #[serde(default)]
    pub lookup: Vec<String>,
    pub capture: Vec<String>,
    // Reserved scopes are deliberately never dispatched until later milestones.
    pub replication: Vec<String>,
    pub attachments: Vec<String>,
    pub background: bool,
    pub routing_metadata: bool,
}
impl Grants {
    fn validate(&self) -> Result<()> {
        if self.relay_auth.len() > 4
            || self.lookup.len() > crate::collection::MAX_LOOKUP_RELAYS
            || self.capture.len() > crate::collection::MAX_CAPTURE_RELAYS
            || !self.replication.is_empty()
            || !self.attachments.is_empty()
            || self.background
            || self.routing_metadata
        {
            return Err(Error::Unsupported);
        }
        for url in self
            .relay_auth
            .iter()
            .chain(&self.lookup)
            .chain(&self.capture)
        {
            relay(url)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
    pub vault_id: String,
    pub token: String,
    pub generation: u32,
    pub account: Option<String>,
    pub signer_generation: u32,
    pub consent_revision: u32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub request_id: String,
    pub binding: Binding,
    pub action: Action,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Status,
    Connect {
        adapter: Adapter,
        remember: bool,
        pairing: Option<String>,
        relays: Vec<String>,
        package: Option<String>,
    },
    Confirm {
        account: String,
    },
    Grants {
        grants: Grants,
    },
    Probe {
        method: String,
    },
    Reply {
        id: String,
        value: Value,
    },
    Failure {
        id: String,
        code: Failure,
    },
    Cancel,
    Disconnect,
    Reconnect,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    MissingSigner,
    Unsupported,
    Denied,
    Unavailable,
    Cancelled,
    Timeout,
    WrongAccount,
    Malformed,
    Revoked,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Effect {
    pub id: String,
    pub binding: Binding,
    pub adapter: Adapter,
    pub method: String,
    pub params: Vec<String>,
    pub package: Option<String>,
    pub relays: Vec<String>,
    pub messages: Vec<Value>,
    pub deadline: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub state: String,
    pub account: Option<String>,
    pub adapter: Option<Adapter>,
    pub capabilities: Capabilities,
    pub grants: Grants,
    pub remember: bool,
    pub failure: Option<Failure>,
    pub pairing: Option<String>,
    pub package: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub request_id: String,
    pub binding: Binding,
    pub view: View,
    pub effect: Option<Effect>,
}
// Credential-bearing structures intentionally have no Debug implementation.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Remembered {
    pub version: u32,
    pub account: String,
    pub adapter: Adapter,
    pub package: Option<String>,
    pub grants: Grants,
    pub remote: Option<RemoteSecret>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteSecret {
    key: String,
    signer: String,
    relays: Vec<String>,
}
impl Drop for RemoteSecret {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}
struct Remote {
    keys: Keys,
    signer: Option<PublicKey>,
    relays: Vec<String>,
    secret: Zeroizing<String>,
    started: u64,
}
struct Pending {
    effect: Effect,
    expected: Option<Zeroizing<String>>,
    auth: Option<AuthChallenge>,
    started: u64,
}
pub struct Identity {
    pub generation: u32,
    pub consent_revision: u32,
    view: View,
    pending: Option<Pending>,
    remote: Option<Remote>,
    confirmed: bool,
    authenticated: Option<VerifiedAuth>,
}
impl Default for Identity {
    fn default() -> Self {
        Self {
            generation: 0,
            consent_revision: 0,
            pending: None,
            remote: None,
            confirmed: false,
            authenticated: None,
            view: View {
                state: "disconnected".into(),
                account: None,
                adapter: None,
                capabilities: Default::default(),
                grants: Default::default(),
                remember: false,
                failure: None,
                pairing: None,
                package: None,
            },
        }
    }
}
fn public(value: &str) -> Result<PublicKey> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err(Error::Malformed);
    }
    PublicKey::from_hex(value).map_err(|_| Error::Malformed)
}
pub fn relay(value: &str) -> Result<()> {
    if value.len() > 2048 {
        return Err(Error::Limit);
    }
    let url = url::Url::parse(value).map_err(|_| Error::Malformed)?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.host_str().is_none()
        || !(url.scheme() == "wss"
            || (url.scheme() == "ws"
                && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))))
    {
        return Err(Error::Unsupported);
    }
    Ok(())
}
fn keys(entropy: &mut dyn Entropy) -> Result<Keys> {
    let bytes = Zeroizing::new(random::<32>(entropy)?);
    Ok(Keys::new(
        SecretKey::from_slice(&*bytes).map_err(|_| Error::Randomness)?,
    ))
}
fn sign(value: Value, keys: &Keys, entropy: &mut dyn Entropy) -> Result<Event> {
    let mut unsigned: UnsignedEvent =
        serde_json::from_value(value).map_err(|_| Error::Malformed)?;
    let id = unsigned.id();
    let signature = keys.sign_schnorr_with_aux_rand(
        &secp256k1::Secp256k1::new(),
        id.as_bytes(),
        &random(entropy)?,
    );
    unsigned.add_signature(signature).map_err(|_| Error::Crypto)
}
impl Identity {
    pub fn view(&self) -> View {
        self.view.clone()
    }
    pub fn account(&self) -> Option<&str> {
        self.view.account.as_deref()
    }
    pub fn confirmed(&self) -> bool {
        self.confirmed
    }
    pub fn consent_revision(&self) -> u32 {
        self.consent_revision
    }
    pub fn pending(&self) -> Option<Effect> {
        self.pending.as_ref().map(|p| p.effect.clone())
    }
    pub fn invalidate(&mut self) {
        let generation = self.generation.wrapping_add(1);
        *self = Self::default();
        self.generation = generation;
    }
    pub fn remembered(&self) -> Result<Option<Remembered>> {
        if !self.view.remember || !self.confirmed {
            return Ok(None);
        }
        let Some(account) = self.view.account.clone() else {
            return Ok(None);
        };
        Ok(Some(Remembered {
            version: 1,
            account,
            adapter: self.view.adapter.ok_or(Error::Malformed)?,
            package: self.view.package.clone(),
            grants: self.view.grants.clone(),
            remote: self.remote.as_ref().map(|r| RemoteSecret {
                key: r.keys.secret_key().to_secret_hex(),
                signer: r.signer.map(|p| p.to_hex()).unwrap_or_default(),
                relays: r.relays.clone(),
            }),
        }))
    }
    pub fn restore(&mut self, value: Remembered) -> Result<()> {
        if value.version != 1 {
            return Err(Error::Unsupported);
        }
        public(&value.account)?;
        value.grants.validate()?;
        self.invalidate();
        self.view.account = Some(value.account);
        self.view.adapter = Some(value.adapter);
        self.view.package = value.package;
        self.view.grants = value.grants;
        self.view.remember = true;
        self.confirmed = true;
        self.view.state = "disconnected".into();
        if let Some(r) = value.remote {
            if value.adapter != Adapter::Remote || r.relays.is_empty() || r.relays.len() > 4 {
                return Err(Error::Malformed);
            }
            for u in &r.relays {
                relay(u)?;
            }
            self.remote = Some(Remote {
                keys: Keys::new(SecretKey::from_hex(&r.key).map_err(|_| Error::Malformed)?),
                signer: Some(public(&r.signer)?),
                relays: r.relays.clone(),
                secret: Zeroizing::new(String::new()),
                started: 0,
            });
        }
        Ok(())
    }
    fn begin(
        &mut self,
        method: &str,
        params: Vec<String>,
        binding: &Binding,
        now: u64,
        entropy: &mut dyn Entropy,
        expected: Option<Zeroizing<String>>,
    ) -> Result<()> {
        if self.pending.is_some() {
            return Err(Error::Busy);
        }
        let id = hex::encode(random::<16>(entropy)?);
        let adapter = self.view.adapter.ok_or(Error::Malformed)?;
        let mut effect = Effect {
            id: id.clone(),
            binding: binding.clone(),
            adapter,
            method: method.into(),
            params: params.clone(),
            package: self.view.package.clone(),
            relays: vec![],
            messages: vec![],
            deadline: now + if method == "pair" { 300 } else { 120 },
        };
        if adapter == Adapter::Remote {
            let remote = self.remote.as_ref().ok_or(Error::Malformed)?;
            effect.params.clear();
            effect.relays = remote.relays.clone();
            effect.messages.push(json!(["REQ",id,{"kinds":[24133],"#p":[remote.keys.public_key().to_hex()],"since":now.saturating_sub(30),"limit":16}]));
            if method != "pair" {
                let recipient = remote.signer.ok_or(Error::Malformed)?;
                let plain = Zeroizing::new(
                    serde_json::to_string(&json!({"id":id,"method":method,"params":params}))
                        .map_err(|_| Error::Malformed)?,
                );
                let cipher = nip44::encrypt_with_nonce(
                    remote.keys.secret_key(),
                    &recipient,
                    plain.as_bytes(),
                    nip44::Nonce::V2(random(entropy)?),
                )
                .map_err(|_| Error::Crypto)?;
                let event = sign(
                    json!({"pubkey":remote.keys.public_key(),"created_at":now,"kind":24133,"tags":[["p",recipient.to_hex()]],"content":cipher}),
                    &remote.keys,
                    entropy,
                )?;
                effect.messages.push(json!(["EVENT", event]));
            }
        }
        self.pending = Some(Pending {
            effect,
            expected,
            auth: None,
            started: now,
        });
        self.view.state = "waiting_for_approval".into();
        self.view.failure = None;
        Ok(())
    }
    /// Called by the engine's approved connection owner, never by a generic UI signing command.
    pub fn begin_auth(
        &mut self,
        context: (&str, &str, &str),
        binding: &Binding,
        now: u64,
        entropy: &mut dyn Entropy,
    ) -> Result<()> {
        if !self.confirmed || self.view.state != "connected" {
            return Err(Error::Authentication);
        }
        let (relay, challenge, connection) = context;
        let auth = AuthChallenge::new(
            relay,
            challenge,
            connection,
            binding.clone(),
            &self.view.grants,
            now,
        )?;
        self.begin(
            "sign_event",
            vec![auth.request()?.to_string()],
            binding,
            now,
            entropy,
            None,
        )?;
        self.pending.as_mut().ok_or(Error::Cancelled)?.auth = Some(auth);
        self.authenticated = None;
        Ok(())
    }
    /// A connection close or challenge replacement invalidates its in-flight signature.
    pub fn cancel_auth(&mut self) {
        self.authenticated = None;
        if self.pending.as_ref().is_some_and(|p| p.auth.is_some()) {
            self.pending = None;
            self.generation = self.generation.wrapping_add(1);
            self.view.state = "connected".into();
        }
    }
    pub fn take_auth(&mut self) -> Option<VerifiedAuth> {
        self.authenticated.take()
    }
    pub fn handle(
        &mut self,
        action: Action,
        binding: &Binding,
        now: u64,
        entropy: &mut dyn Entropy,
    ) -> Result<bool> {
        match action {
            Action::Status => Ok(false),
            Action::Cancel => {
                self.pending = None;
                if self.view.account.is_none() {
                    self.remote = None;
                }
                self.view.pairing = None;
                self.generation = self.generation.wrapping_add(1);
                self.view.state = "needs_authorization".into();
                self.view.failure = Some(Failure::Cancelled);
                Ok(false)
            }
            Action::Disconnect => {
                self.invalidate();
                Ok(true)
            }
            Action::Connect {
                adapter,
                remember,
                pairing,
                relays,
                package,
            } => {
                if self.pending.is_some() {
                    return Err(Error::Busy);
                }
                if self.view.account.is_some() {
                    return Err(Error::Conflict);
                }
                self.view.adapter = Some(adapter);
                self.view.remember = remember;
                self.view.package = package;
                if adapter == Adapter::Remote {
                    if relays.is_empty() || relays.len() > 4 {
                        return Err(Error::Limit);
                    }
                    for url in &relays {
                        relay(url)?;
                    }
                    let mut remote = Remote {
                        keys: keys(entropy)?,
                        signer: None,
                        relays,
                        secret: Zeroizing::new(hex::encode(random::<32>(entropy)?)),
                        started: now,
                    };
                    if let Some(pairing) = pairing.filter(|p| !p.is_empty()) {
                        if pairing.len() > 8192 {
                            return Err(Error::Limit);
                        }
                        let url = url::Url::parse(&pairing).map_err(|_| Error::Malformed)?;
                        if url.scheme() != "bunker" {
                            return Err(Error::Malformed);
                        }
                        remote.signer = Some(public(url.host_str().ok_or(Error::Malformed)?)?);
                        let mut secret = String::new();
                        let mut found = vec![];
                        for (k, v) in url.query_pairs() {
                            match k.as_ref() {
                                "relay" => found.push(v.into_owned()),
                                "secret" => secret = v.into_owned(),
                                _ => return Err(Error::Malformed),
                            }
                        }
                        if found != remote.relays {
                            return Err(Error::Conflict);
                        }
                        let params = vec![
                            remote.signer.ok_or(Error::Malformed)?.to_hex(),
                            secret,
                            String::new(),
                            "{\"name\":\"NostrVault\"}".into(),
                        ];
                        self.remote = Some(remote);
                        self.begin("connect", params, binding, now, entropy, None)?;
                    } else {
                        let mut url = url::Url::parse(&format!(
                            "nostrconnect://{}",
                            remote.keys.public_key()
                        ))
                        .map_err(|_| Error::Malformed)?;
                        {
                            let mut q = url.query_pairs_mut();
                            for r in &remote.relays {
                                q.append_pair("relay", r);
                            }
                            q.append_pair("secret", &remote.secret);
                            q.append_pair("name", "NostrVault");
                        }
                        self.view.pairing = Some(url.to_string());
                        self.remote = Some(remote);
                        self.begin("pair", vec![], binding, now, entropy, None)?;
                    }
                } else {
                    self.begin("get_public_key", vec![], binding, now, entropy, None)?;
                }
                Ok(false)
            }
            Action::Reconnect => {
                if self.pending.is_some() {
                    return Err(Error::Busy);
                }
                if self.view.adapter == Some(Adapter::Android) && self.view.account.is_some() {
                    self.view.state = "connected".into();
                    self.view.failure = None;
                } else {
                    self.begin("get_public_key", vec![], binding, now, entropy, None)?;
                }
                Ok(false)
            }
            Action::Confirm { account } => {
                if self.view.state != "awaiting_confirmation"
                    || self.view.account.as_ref() != Some(&account)
                {
                    return Err(Error::Authentication);
                }
                self.view.state = "connected".into();
                self.confirmed = true;
                self.generation = self.generation.wrapping_add(1);
                Ok(true)
            }
            Action::Grants { grants } => {
                if !self.confirmed {
                    return Err(Error::Authentication);
                }
                grants.validate()?;
                if grants.readable && self.view.adapter == Some(Adapter::Browser) {
                    return Err(Error::Unsupported);
                }
                self.pending = None;
                self.authenticated = None;
                self.view.grants = grants;
                self.view.state = "connected".into();
                self.consent_revision = self.consent_revision.checked_add(1).ok_or(Error::Limit)?;
                Ok(true)
            }
            Action::Probe { method } => {
                if self.view.adapter == Some(Adapter::Browser) {
                    return Err(Error::Unsupported);
                }
                if self.view.state != "connected"
                    || !self.view.grants.readable
                    || !matches!(method.as_str(), "nip04_decrypt" | "nip44_decrypt")
                {
                    return Err(Error::Authentication);
                }
                let available = if method == "nip04_decrypt" {
                    self.view.capabilities.nip04
                } else {
                    self.view.capabilities.nip44
                };
                if available == Some(false) {
                    return Err(Error::Unsupported);
                }
                let ephemeral = keys(entropy)?;
                let account = public(self.view.account.as_deref().ok_or(Error::Authentication)?)?;
                let expected = Zeroizing::new(hex::encode(random::<32>(entropy)?));
                let cipher = if method == "nip04_decrypt" {
                    nip04::encrypt_with_iv(
                        ephemeral.secret_key(),
                        &account,
                        expected.as_bytes(),
                        random(entropy)?,
                    )
                } else {
                    nip44::encrypt_with_nonce(
                        ephemeral.secret_key(),
                        &account,
                        expected.as_bytes(),
                        nip44::Nonce::V2(random(entropy)?),
                    )
                }
                .map_err(|_| Error::Crypto)?;
                self.begin(
                    &method,
                    vec![ephemeral.public_key().to_hex(), cipher],
                    binding,
                    now,
                    entropy,
                    Some(expected),
                )?;
                Ok(false)
            }
            Action::Failure { id, code } => {
                if self.pending.as_ref().map(|p| p.effect.id.as_str()) != Some(&id) {
                    return Err(Error::Cancelled);
                }
                self.pending = None;
                self.view.state = "needs_authorization".into();
                self.view.failure = Some(code);
                self.view.pairing = None;
                Ok(false)
            }
            Action::Reply { id, value } => {
                // Unsolicited IDs must not cancel the legitimate pending request.
                if self.pending.as_ref().is_none_or(|p| p.effect.id != id) {
                    return Err(Error::Cancelled);
                }
                match self.reply(&id, value, binding, now, entropy) {
                    Ok(persist) => Ok(persist),
                    Err(error) => {
                        self.pending = None;
                        self.view.pairing = None;
                        self.view.state = "needs_authorization".into();
                        self.view.failure = Some(match error {
                            Error::Cancelled => Failure::Timeout,
                            Error::Authentication => Failure::WrongAccount,
                            _ => Failure::Malformed,
                        });
                        Ok(false)
                    }
                }
            }
        }
    }
    fn reply(
        &mut self,
        id: &str,
        mut value: Value,
        binding: &Binding,
        now: u64,
        entropy: &mut dyn Entropy,
    ) -> Result<bool> {
        let pending = self.pending.as_ref().ok_or(Error::Cancelled)?;
        if pending.effect.id != id
            || pending.effect.binding != *binding
            || now > pending.effect.deadline
        {
            return Err(Error::Cancelled);
        }
        let method = pending.effect.method.clone();
        if self.view.adapter == Some(Adapter::Remote) {
            let event: Event = serde_json::from_value(value).map_err(|_| Error::Malformed)?;
            event.verify().map_err(|_| Error::Authentication)?;
            let r = self.remote.as_mut().ok_or(Error::Malformed)?;
            if event.kind.as_u16() != 24133
                || event.created_at.as_secs() < pending.started.saturating_sub(30)
                || event.created_at.as_secs() > now + 30
                || (method != "pair" && Some(event.pubkey) != r.signer)
                || !event
                    .tags
                    .iter()
                    .any(|t| t.as_slice() == ["p", r.keys.public_key().to_hex().as_str()])
            {
                return Err(Error::Authentication);
            }
            let plain = Zeroizing::new(
                nip44::decrypt(r.keys.secret_key(), &event.pubkey, &event.content)
                    .map_err(|_| Error::Authentication)?,
            );
            if plain.len() > WIRE_LIMIT {
                return Err(Error::Limit);
            }
            let response: Value = serde_json::from_str(&plain).map_err(|_| Error::Malformed)?;
            if method == "pair" {
                if now > r.started + 300 || response["result"].as_str() != Some(r.secret.as_str()) {
                    return Err(Error::Authentication);
                }
                r.signer = Some(event.pubkey);
                r.secret.zeroize();
            } else if response["id"].as_str() != Some(id) {
                return Err(Error::Cancelled);
            }
            if response.get("error").is_some_and(|v| !v.is_null()) {
                self.pending = None;
                self.view.state = "needs_authorization".into();
                self.view.failure = Some(Failure::Unavailable);
                return Ok(false);
            }
            value = response["result"].clone();
        }
        let pending = self.pending.take().ok_or(Error::Cancelled)?;
        if method == "connect" || method == "pair" {
            if method == "connect" && value.as_str() != Some("ack") {
                return Err(Error::Authentication);
            }
            self.view.pairing = None;
            self.begin("get_public_key", vec![], binding, now, entropy, None)?;
            return Ok(false);
        }
        if method == "sign_event" {
            let auth = pending.auth.ok_or(Error::Authentication)?;
            let event: Event = if let Some(encoded) = value.as_str() {
                serde_json::from_str(encoded).map_err(|_| Error::Malformed)?
            } else {
                serde_json::from_value(value).map_err(|_| Error::Malformed)?
            };
            let connection = auth.connection.clone();
            let challenge = auth.challenge.clone();
            self.authenticated = Some(auth.verify(event, binding, &connection, &challenge, now)?);
            self.view.capabilities.relay_auth = Some(true);
            self.view.state = "connected".into();
            return Ok(false);
        }
        if method == "get_public_key" {
            let (account, capabilities, package) = if self.view.adapter == Some(Adapter::Remote) {
                (
                    value.as_str().ok_or(Error::Malformed)?.to_owned(),
                    Capabilities {
                        public_key: true,
                        ..Default::default()
                    },
                    None,
                )
            } else {
                let a = value["account"]
                    .as_str()
                    .ok_or(Error::Malformed)?
                    .to_owned();
                let c = serde_json::from_value(value["capabilities"].clone())
                    .map_err(|_| Error::Malformed)?;
                let p = value["package"].as_str().map(str::to_owned);
                (a, c, p)
            };
            public(&account)?;
            if self.view.account.as_ref().is_some_and(|a| a != &account) {
                self.view.state = "needs_authorization".into();
                self.view.failure = Some(Failure::WrongAccount);
                return Ok(false);
            }
            if self.view.adapter == Some(Adapter::Android)
                && package.as_ref().is_none_or(|p| {
                    p.len() > 256
                        || !p.contains('.')
                        || !p
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_')
                })
            {
                return Err(Error::Malformed);
            }
            if !capabilities.public_key
                || (self.view.adapter == Some(Adapter::Android) && self.view.package != package)
            {
                return Err(Error::Authentication);
            }
            let known = self.confirmed;
            self.view.account = Some(account);
            self.view.capabilities = capabilities;
            self.view.package = package;
            self.view.state = if known {
                "connected"
            } else {
                "awaiting_confirmation"
            }
            .into();
        } else {
            if value.as_str() != pending.expected.as_deref().map(String::as_str) {
                self.view.state = "needs_authorization".into();
                self.view.failure = Some(Failure::WrongAccount);
                return Ok(false);
            }
            if method == "nip04_decrypt" {
                self.view.capabilities.nip04 = Some(true);
            } else {
                self.view.capabilities.nip44 = Some(true);
            }
            self.view.state = "connected".into();
        }
        Ok(false)
    }
}

/// Only this wrapper can encode a validated authentication event for transport.
/// It deliberately provides no ordinary EVENT encoder.
pub struct VerifiedAuth(Event);
impl VerifiedAuth {
    pub fn auth_message(&self) -> Value {
        json!(["AUTH", self.0])
    }
}
pub struct AuthChallenge {
    relay: String,
    challenge: String,
    account: PublicKey,
    connection: String,
    requested: UnsignedEvent,
    binding: Binding,
}
impl AuthChallenge {
    pub fn new(
        relay_url: &str,
        challenge: &str,
        connection: &str,
        binding: Binding,
        grants: &Grants,
        now: u64,
    ) -> Result<Self> {
        relay(relay_url)?;
        if !grants.relay_auth.iter().any(|r| r == relay_url)
            || challenge.is_empty()
            || challenge.len() > 1024
            || connection.is_empty()
            || connection.len() > 64
        {
            return Err(Error::Authentication);
        }
        let account = public(binding.account.as_deref().ok_or(Error::Authentication)?)?;
        let requested=serde_json::from_value(json!({"pubkey":account,"created_at":now,"kind":22242,"tags":[["relay",relay_url],["challenge",challenge]],"content":""})).map_err(|_|Error::Malformed)?;
        Ok(Self {
            relay: relay_url.into(),
            challenge: challenge.into(),
            account,
            connection: connection.into(),
            requested,
            binding,
        })
    }
    pub fn request(&self) -> Result<Value> {
        serde_json::to_value(&self.requested).map_err(|_| Error::Malformed)
    }
    pub fn verify(
        self,
        event: Event,
        binding: &Binding,
        connection: &str,
        current_challenge: &str,
        now: u64,
    ) -> Result<VerifiedAuth> {
        event.verify().map_err(|_| Error::Authentication)?;
        if &self.binding != binding
            || self.connection != connection
            || self.challenge != current_challenge
            || event.pubkey != self.account
            || event.kind.as_u16() != 22242
            || !event.content.is_empty()
            || event.created_at != self.requested.created_at
            || event.tags != self.requested.tags
            || event.created_at.as_secs() > now + 30
            || now.saturating_sub(event.created_at.as_secs()) > 120
            || self
                .requested
                .tags
                .first()
                .is_none_or(|t| t.as_slice() != ["relay", self.relay.as_str()])
        {
            return Err(Error::Authentication);
        }
        Ok(VerifiedAuth(event))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Random(u8);
    impl Entropy for Random {
        fn fill(&mut self, b: &mut [u8]) -> Result<()> {
            self.0 = self.0.wrapping_add(1);
            b.fill(self.0);
            Ok(())
        }
    }
    fn binding(i: &Identity) -> Binding {
        Binding {
            vault_id: "11".repeat(16),
            token: "22".repeat(16),
            generation: 1,
            account: i.account().map(str::to_owned),
            signer_generation: i.generation,
            consent_revision: i.consent_revision,
        }
    }
    fn connected(adapter: Adapter) -> (Identity, Random, Keys) {
        let mut e = Random(1);
        let k = keys(&mut e).unwrap();
        let mut i = Identity::default();
        i.handle(
            Action::Connect {
                adapter,
                remember: true,
                pairing: None,
                relays: vec![],
                package: if adapter == Adapter::Android {
                    Some("test.signer".into())
                } else {
                    None
                },
            },
            &binding(&i),
            100,
            &mut e,
        )
        .unwrap();
        let p = i.pending().unwrap();
        let b = binding(&i);
        i.handle(Action::Reply {id:p.id,value:json!({"account":k.public_key().to_hex(),"package":if adapter==Adapter::Android{Some("test.signer")}else{None},"capabilities":{"publicKey":true,"nip04":true,"nip44":true,"relayAuth":true}})},&b,101,&mut e).unwrap();
        let b = binding(&i);
        i.handle(
            Action::Confirm {
                account: k.public_key().to_hex(),
            },
            &b,
            102,
            &mut e,
        )
        .unwrap();
        (i, e, k)
    }
    #[test]
    fn consent_probe_replay_and_revocation() {
        let (mut i, mut e, k) = connected(Adapter::Android);
        let b = binding(&i);
        assert!(i
            .handle(
                Action::Probe {
                    method: "nip44_decrypt".into()
                },
                &b,
                103,
                &mut e
            )
            .is_err());
        let grants = Grants {
            readable: true,
            ..Default::default()
        };
        i.handle(Action::Grants { grants }, &b, 104, &mut e)
            .unwrap();
        let b = binding(&i);
        i.handle(
            Action::Probe {
                method: "nip44_decrypt".into(),
            },
            &b,
            105,
            &mut e,
        )
        .unwrap();
        let p = i.pending().unwrap();
        let value =
            nip44::decrypt(k.secret_key(), &public(&p.params[0]).unwrap(), &p.params[1]).unwrap();
        i.handle(
            Action::Reply {
                id: p.id.clone(),
                value: json!(value),
            },
            &b,
            106,
            &mut e,
        )
        .unwrap();
        assert!(i
            .handle(
                Action::Reply {
                    id: p.id,
                    value: json!(value)
                },
                &b,
                107,
                &mut e
            )
            .is_err());
        let stored = i.remembered().unwrap().unwrap();
        let mut other = Identity::default();
        other.restore(stored).unwrap();
        assert_eq!(other.view.state, "disconnected");
        assert!(other.pending().is_none());
        i.handle(Action::Disconnect, &b, 108, &mut e).unwrap();
        assert!(i.remembered().unwrap().is_none());
        assert!(i.account().is_none());
    }
    #[test]
    fn android_reconnect_does_not_prompt_and_future_grants_deny() {
        let (mut i, mut e, _) = connected(Adapter::Android);
        let b = binding(&i);
        i.handle(Action::Reconnect, &b, 110, &mut e).unwrap();
        assert!(i.pending().is_none());
        assert!(Grants {
            background: true,
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(Grants {
            replication: vec!["wss://example.com".into()],
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(relay("ws://example.com").is_err());
        assert!(relay("wss://user:pass@example.com").is_err());
    }
    #[test]
    fn remote_pair_secret_context_and_freshness() {
        let mut i = Identity::default();
        let mut e = Random(10);
        let signer = keys(&mut e).unwrap();
        i.handle(
            Action::Connect {
                adapter: Adapter::Remote,
                remember: true,
                pairing: None,
                relays: vec!["ws://127.0.0.1:9999".into()],
                package: None,
            },
            &binding(&i),
            100,
            &mut e,
        )
        .unwrap();
        let p = i.pending().unwrap();
        let r = i.remote.as_ref().unwrap();
        let body = json!({"id":"signer-connect","result":r.secret.as_str()});
        let cipher = nip44::encrypt_with_nonce(
            signer.secret_key(),
            &r.keys.public_key(),
            body.to_string(),
            nip44::Nonce::V2([9; 32]),
        )
        .unwrap();
        let event=sign(json!({"pubkey":signer.public_key(),"kind":24133,"created_at":100,"tags":[["p",r.keys.public_key().to_hex()]],"content":cipher}),&signer,&mut e).unwrap();
        i.handle(
            Action::Reply {
                id: p.id.clone(),
                value: json!(event),
            },
            &binding(&i),
            101,
            &mut e,
        )
        .unwrap();
        assert_eq!(i.pending().unwrap().method, "get_public_key");
        assert!(i
            .handle(
                Action::Reply {
                    id: p.id,
                    value: json!(event)
                },
                &binding(&i),
                102,
                &mut e
            )
            .is_err());
        i.handle(Action::Cancel, &binding(&i), 103, &mut e).unwrap();
        assert!(i.pending().is_none());
    }
    #[test]
    fn failures_cancellation_and_revocation_never_adopt_unconfirmed_identity() {
        for code in [
            Failure::Denied,
            Failure::Unavailable,
            Failure::Revoked,
            Failure::Timeout,
        ] {
            let (mut i, mut e, _) = connected(Adapter::Browser);
            i.handle(Action::Reconnect, &binding(&i), 120, &mut e)
                .unwrap();
            let pending = i.pending().unwrap();
            i.handle(
                Action::Failure {
                    id: pending.id.clone(),
                    code,
                },
                &binding(&i),
                121,
                &mut e,
            )
            .unwrap();
            assert_eq!(i.view.failure, Some(code));
            assert!(i.pending().is_none());
            assert!(i
                .handle(
                    Action::Reply {
                        id: pending.id,
                        value: json!("late")
                    },
                    &binding(&i),
                    122,
                    &mut e
                )
                .is_err());
        }
        let (mut i, mut e, _) = connected(Adapter::Browser);
        i.handle(Action::Reconnect, &binding(&i), 120, &mut e)
            .unwrap();
        let pending = i.pending().unwrap();
        i.handle(
            Action::Grants {
                grants: Grants::default(),
            },
            &binding(&i),
            121,
            &mut e,
        )
        .unwrap();
        assert!(i.pending().is_none());
        assert!(i
            .handle(
                Action::Reply {
                    id: pending.id,
                    value: json!("late")
                },
                &binding(&i),
                122,
                &mut e
            )
            .is_err());
        i.handle(Action::Disconnect, &binding(&i), 123, &mut e)
            .unwrap();
        i.handle(
            Action::Connect {
                adapter: Adapter::Android,
                remember: true,
                pairing: None,
                relays: vec![],
                package: Some("right.package".into()),
            },
            &binding(&i),
            124,
            &mut e,
        )
        .unwrap();
        let pending = i.pending().unwrap();
        let account = keys(&mut e).unwrap().public_key().to_hex();
        i.handle(Action::Reply{id:pending.id,value:json!({"account":account,"package":"wrong.package","capabilities":{"publicKey":true,"nip04":null,"nip44":null,"relayAuth":null}})}, &binding(&i), 125, &mut e).unwrap();
        assert!(i.account().is_none());
        assert!(i.remembered().unwrap().is_none());
        assert_eq!(i.view.state, "needs_authorization");
    }
    #[test]
    fn expired_response_and_failed_randomness_fail_closed() {
        let (mut i, mut e, _) = connected(Adapter::Browser);
        i.handle(Action::Reconnect, &binding(&i), 120, &mut e)
            .unwrap();
        let pending = i.pending().unwrap();
        i.handle(
            Action::Reply {
                id: pending.id,
                value: json!("ignored"),
            },
            &binding(&i),
            241,
            &mut e,
        )
        .unwrap();
        assert_eq!(i.view.failure, Some(Failure::Timeout));
        assert!(i.pending().is_none());
        struct Broken;
        impl Entropy for Broken {
            fn fill(&mut self, _: &mut [u8]) -> Result<()> {
                Err(Error::Randomness)
            }
        }
        assert_eq!(
            i.handle(Action::Reconnect, &binding(&i), 242, &mut Broken),
            Err(Error::Randomness)
        );
        assert!(i.pending().is_none());
    }
    #[test]
    fn auth_dispatch_and_cancel_are_bound_to_active_scope() {
        let (mut i, mut e, k) = connected(Adapter::Browser);
        assert!(i
            .begin_auth(
                ("wss://relay.example/", "challenge", "connection"),
                &binding(&i),
                100,
                &mut e
            )
            .is_err());
        i.handle(
            Action::Grants {
                grants: Grants {
                    relay_auth: vec!["wss://relay.example/".into()],
                    ..Default::default()
                },
            },
            &binding(&i),
            100,
            &mut e,
        )
        .unwrap();
        let b = binding(&i);
        i.begin_auth(
            ("wss://relay.example/", "challenge", "connection"),
            &b,
            101,
            &mut e,
        )
        .unwrap();
        let pending = i.pending().unwrap();
        let event = sign(
            serde_json::from_str(&pending.params[0]).unwrap(),
            &k,
            &mut e,
        )
        .unwrap();
        i.cancel_auth();
        assert!(i
            .handle(
                Action::Reply {
                    id: pending.id,
                    value: json!(event)
                },
                &b,
                102,
                &mut e
            )
            .is_err());
        assert!(i.take_auth().is_none());
        // Browser decryption remains fail-closed even when the method exists.
        assert_eq!(
            i.handle(
                Action::Grants {
                    grants: Grants {
                        readable: true,
                        ..Default::default()
                    }
                },
                &binding(&i),
                103,
                &mut e
            ),
            Err(Error::Unsupported)
        );
    }
    #[test]
    fn auth_only_exact_challenge_and_event() {
        let (i, mut e, k) = connected(Adapter::Browser);
        let b = binding(&i);
        let grants = Grants {
            relay_auth: vec!["wss://relay.example/".into()],
            ..Default::default()
        };
        let challenge = AuthChallenge::new(
            "wss://relay.example/",
            "active",
            "connection",
            b.clone(),
            &grants,
            100,
        )
        .unwrap();
        let event = sign(challenge.request().unwrap(), &k, &mut e).unwrap();
        let verified = challenge
            .verify(event.clone(), &b, "connection", "active", 101)
            .unwrap();
        assert_eq!(verified.auth_message()[0], "AUTH");
        let challenge = AuthChallenge::new(
            "wss://relay.example/",
            "active",
            "connection",
            b.clone(),
            &grants,
            100,
        )
        .unwrap();
        assert!(challenge
            .verify(event, &b, "connection", "replaced", 101)
            .is_err());
        assert!(AuthChallenge::new(
            "wss://other.example/",
            "active",
            "connection",
            b,
            &grants,
            100
        )
        .is_err());
    }
}
