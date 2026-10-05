//! Versioned production vault primitives. Foundation fixtures never enter this module.
use argon2::{Algorithm, Argon2, Block, Params, Version};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroizing;

pub const HEADER_LIMIT: usize = 4096;
pub const RECORD_LIMIT: usize = 1_048_576;
pub const BATCH_LIMIT: usize = 16;
pub const BYTES_LIMIT: usize = 4 * RECORD_LIMIT;
pub const CIPHER_LIMIT: usize = RECORD_LIMIT + 40;
pub const SETUP_ACCOUNT: &str = "vault-system";
pub const SETUP_KEY: &str = "setup";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Error {
    Malformed,
    Unsupported,
    Limit,
    Authentication,
    Randomness,
    Crypto,
    Storage,
    Quota,
    Busy,
    Conflict,
    Locked,
    Cancelled,
    Exists,
    PasswordPolicy,
    MigrationRequired,
}
pub type Result<T> = std::result::Result<T, Error>;
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "vault {self:?}")
    }
}
impl std::error::Error for Error {}

pub trait Entropy {
    fn fill(&mut self, bytes: &mut [u8]) -> Result<()>;
}
pub fn random<const N: usize>(entropy: &mut dyn Entropy) -> Result<[u8; N]> {
    let mut bytes = [0; N];
    entropy.fill(&mut bytes)?;
    Ok(bytes)
}
fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|_| Error::Malformed)
}
pub fn password(value: &str, new: bool) -> Result<()> {
    if value.len() > 1024 || value.is_empty() {
        return Err(Error::Limit);
    }
    if new && value.chars().count() < 12 {
        return Err(Error::PasswordPolicy);
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Header {
    pub format: u32,
    pub wrapper: u32,
    pub vault_id: String,
    pub memory_kib: u32,
    pub passes: u32,
    pub lanes: u32,
    pub revision: u32,
    pub salt: [u8; 16],
    pub nonce: [u8; 24],
    pub wrapped: Vec<u8>,
}
impl Header {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > HEADER_LIMIT {
            return Err(Error::Limit);
        }
        let header: Self = serde_json::from_slice(bytes).map_err(|_| Error::Malformed)?;
        header.validate()?;
        Ok(header)
    }
    pub fn bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        encode(self)
    }
    pub fn validate(&self) -> Result<()> {
        if self.format != 1
            || self.wrapper != 1
            || self.memory_kib != 65536
            || self.passes != 3
            || self.lanes != 4
        {
            return Err(Error::Unsupported);
        }
        if self.vault_id.len() != 32
            || !self.vault_id.bytes().all(|b| b.is_ascii_hexdigit())
            || self.revision == 0
            || self.wrapped.len() != 48
        {
            return Err(Error::Malformed);
        }
        Ok(())
    }
    fn aad(&self) -> Result<Vec<u8>> {
        encode(&(
            "nostrvault/wrap/1",
            self.format,
            self.wrapper,
            &self.vault_id,
            self.memory_kib,
            self.passes,
            self.lanes,
            self.revision,
            self.salt,
        ))
    }
}
pub fn derive(password_value: &str, salt: &[u8; 16]) -> Result<Zeroizing<[u8; 32]>> {
    password(password_value, false)?;
    let params = Params::new(65536, 3, 4, Some(32)).map_err(|_| Error::Crypto)?;
    let mut memory = Zeroizing::new(Vec::new());
    memory
        .try_reserve_exact(params.block_count())
        .map_err(|_| Error::Quota)?;
    memory.resize(params.block_count(), Block::new());
    let mut output = Zeroizing::new([0; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into_with_memory(password_value.as_bytes(), salt, &mut *output, &mut *memory)
        .map_err(|_| Error::Crypto)?;
    Ok(output)
}

pub struct Keys {
    root: Zeroizing<[u8; 32]>,
    pub header: Header,
}
impl Keys {
    pub fn create(value: &str, entropy: &mut dyn Entropy) -> Result<Self> {
        password(value, true)?;
        let root = Zeroizing::new(random(entropy)?);
        let mut header = Header {
            format: 1,
            wrapper: 1,
            vault_id: hex::encode(random::<16>(entropy)?),
            memory_kib: 65536,
            passes: 3,
            lanes: 4,
            revision: 1,
            salt: random(entropy)?,
            nonce: random(entropy)?,
            wrapped: vec![0; 48],
        };
        Self::wrap(&root, &mut header, value)?;
        Ok(Self { root, header })
    }
    fn wrap(root: &[u8; 32], header: &mut Header, value: &str) -> Result<()> {
        let wrapping = derive(value, &header.salt)?;
        header.wrapped = XChaCha20Poly1305::new((&*wrapping).into())
            .encrypt(
                &XNonce::from(header.nonce),
                Payload {
                    msg: root,
                    aad: &header.aad()?,
                },
            )
            .map_err(|_| Error::Crypto)?;
        Ok(())
    }
    pub fn unlock(header: Header, value: &str) -> Result<Self> {
        header.validate()?;
        let wrapping = derive(value, &header.salt)?;
        let plain = Zeroizing::new(
            XChaCha20Poly1305::new((&*wrapping).into())
                .decrypt(
                    &XNonce::from(header.nonce),
                    Payload {
                        msg: &header.wrapped,
                        aad: &header.aad()?,
                    },
                )
                .map_err(|_| Error::Authentication)?,
        );
        let mut root = Zeroizing::new([0; 32]);
        if plain.len() != 32 {
            return Err(Error::Authentication);
        }
        root.copy_from_slice(&plain);
        Ok(Self { root, header })
    }
    pub fn rewrap(&self, current: &str, next: &str, entropy: &mut dyn Entropy) -> Result<Header> {
        password(next, true)?;
        // Verification uses the authoritative wrapper, not a cached password.
        let verified = Self::unlock(self.header.clone(), current)?;
        let mut header = self.header.clone();
        header.revision = header.revision.checked_add(1).ok_or(Error::Limit)?;
        header.salt = random(entropy)?;
        header.nonce = random(entropy)?;
        Self::wrap(&verified.root, &mut header, next)?;
        Ok(header)
    }
    fn subkey(&self, domain: &str, account: &str) -> Result<Zeroizing<[u8; 32]>> {
        if account.is_empty() || account.len() > 256 {
            return Err(Error::Limit);
        }
        let mut output = Zeroizing::new([0; 32]);
        Hkdf::<Sha256>::new(Some(self.header.vault_id.as_bytes()), &*self.root)
            .expand(&encode(&(domain, account))?, &mut *output)
            .map_err(|_| Error::Crypto)?;
        Ok(output)
    }
    pub fn lookup(&self, account: &str, logical: &str) -> Result<String> {
        if logical.is_empty() || logical.len() > 256 {
            return Err(Error::Limit);
        }
        let key = self.subkey("nostrvault/lookup/1", account)?;
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&*key).map_err(|_| Error::Crypto)?;
        mac.update(&encode(&(account, logical))?);
        Ok(hex::encode(mac.finalize().into_bytes()))
    }
    fn record_aad(&self, account: &str, key: &str, revision: u32) -> Result<Vec<u8>> {
        encode(&(
            "nostrvault/record/1",
            &self.header.vault_id,
            account,
            key,
            1u32,
            revision,
        ))
    }
    pub fn seal(
        &self,
        account: &str,
        logical: &str,
        revision: u32,
        plain: &[u8],
        entropy: &mut dyn Entropy,
    ) -> Result<Record> {
        if plain.len() > RECORD_LIMIT || revision == 0 {
            return Err(Error::Limit);
        }
        let key = self.lookup(account, logical)?;
        let encryption = self.subkey("nostrvault/encryption/1", account)?;
        let nonce = random::<24>(entropy)?;
        let mut bytes = nonce.to_vec();
        bytes.extend(
            XChaCha20Poly1305::new((&*encryption).into())
                .encrypt(
                    &XNonce::from(nonce),
                    Payload {
                        msg: plain,
                        aad: &self.record_aad(account, &key, revision)?,
                    },
                )
                .map_err(|_| Error::Crypto)?,
        );
        Ok(Record {
            namespace: self.lookup(account, "namespace")?,
            key,
            version: 1,
            revision,
            bytes,
        })
    }
    pub fn open(
        &self,
        account: &str,
        logical: &str,
        record: &Record,
    ) -> Result<Zeroizing<Vec<u8>>> {
        record.validate()?;
        if self.lookup(account, logical)? != record.key
            || self.lookup(account, "namespace")? != record.namespace
        {
            return Err(Error::Authentication);
        }
        let key = self.subkey("nostrvault/encryption/1", account)?;
        XChaCha20Poly1305::new((&*key).into())
            .decrypt(
                &XNonce::try_from(&record.bytes[..24]).map_err(|_| Error::Malformed)?,
                Payload {
                    msg: &record.bytes[24..],
                    aad: &self.record_aad(account, &record.key, record.revision)?,
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| Error::Authentication)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Record {
    pub namespace: String,
    pub key: String,
    pub version: u32,
    pub revision: u32,
    pub bytes: Vec<u8>,
}
impl Record {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(Error::Unsupported);
        }
        if self.namespace.len() != 64
            || !self.namespace.bytes().all(|b| b.is_ascii_hexdigit())
            || self.key.len() != 64
            || !self.key.bytes().all(|b| b.is_ascii_hexdigit())
            || self.revision == 0
            || !(40..=CIPHER_LIMIT).contains(&self.bytes.len())
        {
            return Err(Error::Limit);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Setup {
    pub version: u32,
    pub protected: bool,
    pub loss_acknowledged: bool,
}
impl Default for Setup {
    fn default() -> Self {
        Self {
            version: 1,
            protected: true,
            loss_acknowledged: true,
        }
    }
}
impl Setup {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 || !self.protected || !self.loss_acknowledged {
            Err(Error::Malformed)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Absent,
    Creating,
    Locked,
    Unlocking,
    Unlocked,
    Locking,
    ChangingPassword,
    Migrating,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Status {
    pub state: State,
    pub vault_id: Option<String>,
    pub token: String,
    pub generation: u32,
    pub revision: u32,
    pub setup: Option<Setup>,
}
// Password-bearing requests intentionally implement neither Debug nor Clone.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub request_id: String,
    pub token: String,
    pub generation: u32,
    pub vault_id: Option<String>,
    pub operation: Operation,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Status,
    Create {
        password: String,
        confirmation: String,
    },
    Unlock {
        password: String,
    },
    Lock,
    SaveSetup {
        setup: Setup,
        revision: u32,
    },
    ChangePassword {
        current: String,
        password: String,
        confirmation: String,
    },
}
impl Drop for Operation {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        match self {
            Self::Create {
                password,
                confirmation,
            } => {
                password.zeroize();
                confirmation.zeroize();
            }
            Self::Unlock { password } => password.zeroize(),
            Self::ChangePassword {
                current,
                password,
                confirmation,
            } => {
                current.zeroize();
                password.zeroize();
                confirmation.zeroize();
            }
            _ => {}
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Mutation {
    pub expected_revision: u32,
    pub header: Option<Header>,
    pub records: Vec<Record>,
    pub create: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub header: Option<Header>,
    pub revision: u32,
    pub setup: Option<Record>,
}
pub struct Session {
    identity: crate::identity::Identity,
    identity_account: Option<String>,
    keys: Option<Keys>,
    status: Status,
    pending: Option<Status>,
}
impl Session {
    pub fn new(entropy: &mut dyn Entropy) -> Result<Self> {
        Ok(Self {
            keys: None,
            identity: Default::default(),
            identity_account: None,
            pending: None,
            status: Status {
                state: State::Absent,
                vault_id: None,
                token: hex::encode(random::<16>(entropy)?),
                generation: 0,
                revision: 0,
                setup: None,
            },
        })
    }
    pub fn status(&self) -> Status {
        self.status.clone()
    }
    pub fn lookup_setup(&self) -> Result<String> {
        self.keys
            .as_ref()
            .ok_or(Error::Locked)?
            .lookup(SETUP_ACCOUNT, SETUP_KEY)
    }
    pub fn lock(&mut self) {
        self.identity.invalidate();
        self.identity_account = None;
        self.keys = None;
        self.pending = None;
        self.status.setup = None;
        self.status.generation = self.status.generation.wrapping_add(1);
        self.status.state = if self.status.vault_id.is_some() {
            State::Locked
        } else {
            State::Absent
        };
    }
    pub fn observe(&mut self, snapshot: &Snapshot) -> Result<()> {
        if let Some(header) = &snapshot.header {
            header.validate()?;
        }
        if self.keys.is_none() {
            self.status.vault_id = snapshot.header.as_ref().map(|h| h.vault_id.clone());
            self.status.state = if snapshot.header.is_some() {
                State::Locked
            } else {
                State::Absent
            };
        }
        self.status.revision = snapshot.revision;
        Ok(())
    }
    pub fn prepare(
        &mut self,
        request: &Request,
        snapshot: &Snapshot,
        entropy: &mut dyn Entropy,
    ) -> Result<Option<Mutation>> {
        if request.version != 1 || request.request_id.is_empty() || request.request_id.len() > 64 {
            return Err(Error::Malformed);
        }
        if matches!(request.operation, Operation::Status) {
            self.observe(snapshot)?;
            return Ok(None);
        }
        if request.token != self.status.token
            || request.generation != self.status.generation
            || request.vault_id != self.status.vault_id
        {
            return Err(Error::Cancelled);
        }
        if matches!(request.operation, Operation::Lock) {
            self.lock();
            return Ok(None);
        }
        if self.pending.is_some() {
            return Err(Error::Busy);
        }
        match &request.operation {
            Operation::Create {
                password,
                confirmation,
            } => {
                if snapshot.header.is_some() {
                    return Err(Error::Exists);
                }
                if password != confirmation {
                    return Err(Error::PasswordPolicy);
                }
                self.status.state = State::Creating;
                let keys = Keys::create(password, entropy)?;
                let setup = Setup::default();
                let record = keys.seal(SETUP_ACCOUNT, SETUP_KEY, 1, &encode(&setup)?, entropy)?;
                let mutation = Mutation {
                    expected_revision: 0,
                    header: Some(keys.header.clone()),
                    records: vec![record],
                    create: true,
                };
                let mut next = self.status.clone();
                next.state = State::Unlocked;
                next.vault_id = Some(keys.header.vault_id.clone());
                next.setup = Some(setup);
                next.revision = 1;
                self.keys = Some(keys);
                self.pending = Some(next);
                Ok(Some(mutation))
            }
            Operation::Unlock { password } => {
                if self.keys.is_some() {
                    return Err(Error::Busy);
                }
                self.status.state = State::Unlocking;
                self.keys = Some(Keys::unlock(
                    snapshot.header.clone().ok_or(Error::Locked)?,
                    password,
                )?);
                Ok(None)
            }
            Operation::SaveSetup { setup, revision } => {
                if self.status.state != State::Unlocked {
                    return Err(Error::Locked);
                }
                setup.validate()?;
                if *revision != snapshot.revision {
                    return Err(Error::Conflict);
                }
                let next_revision = revision.checked_add(1).ok_or(Error::Limit)?;
                let record = self.keys.as_ref().ok_or(Error::Locked)?.seal(
                    SETUP_ACCOUNT,
                    SETUP_KEY,
                    next_revision,
                    &encode(setup)?,
                    entropy,
                )?;
                let mut next = self.status.clone();
                next.setup = Some(setup.clone());
                next.revision = next_revision;
                self.pending = Some(next);
                Ok(Some(Mutation {
                    expected_revision: *revision,
                    header: None,
                    records: vec![record],
                    create: false,
                }))
            }
            Operation::ChangePassword {
                current,
                password,
                confirmation,
            } => {
                if self.status.state != State::Unlocked {
                    return Err(Error::Locked);
                }
                if password != confirmation {
                    return Err(Error::PasswordPolicy);
                }
                self.status.state = State::ChangingPassword;
                let header = self
                    .keys
                    .as_ref()
                    .ok_or(Error::Locked)?
                    .rewrap(current, password, entropy)?;
                let mut next = self.status.clone();
                next.state = State::Locked;
                next.setup = None;
                next.generation = next.generation.wrapping_add(1);
                next.revision = snapshot.revision.checked_add(1).ok_or(Error::Limit)?;
                self.pending = Some(next);
                Ok(Some(Mutation {
                    expected_revision: snapshot.revision,
                    header: Some(header),
                    records: vec![],
                    create: false,
                }))
            }
            _ => Err(Error::Malformed),
        }
    }
    pub fn finish_unlock(&mut self, record: &Record) -> Result<()> {
        if self.status.state != State::Unlocking {
            return Err(Error::Cancelled);
        }
        let plain =
            self.keys
                .as_ref()
                .ok_or(Error::Locked)?
                .open(SETUP_ACCOUNT, SETUP_KEY, record)?;
        let setup: Setup = serde_json::from_slice(&plain).map_err(|_| Error::Malformed)?;
        setup.validate()?;
        self.status.setup = Some(setup);
        self.status.state = State::Unlocked;
        Ok(())
    }
    pub fn committed(&mut self) -> Result<()> {
        self.status = self.pending.take().ok_or(Error::Conflict)?;
        if self.status.state == State::Locked {
            self.keys = None;
            self.identity.invalidate();
            self.identity_account = None;
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityPrepared {
    pub output: crate::identity::Output,
    pub mutation: Option<Mutation>,
}
impl Session {
    pub fn identity_binding(&self) -> Result<crate::identity::Binding> {
        if self.status.state != State::Unlocked {
            return Err(Error::Locked);
        }
        Ok(crate::identity::Binding {
            vault_id: self.status.vault_id.clone().ok_or(Error::Locked)?,
            token: self.status.token.clone(),
            generation: self.status.generation,
            account: self.identity.account().map(str::to_owned),
            signer_generation: self.identity.generation,
            consent_revision: self.identity.consent_revision,
        })
    }
    pub fn identity_index(&self) -> Result<String> {
        self.keys
            .as_ref()
            .ok_or(Error::Locked)?
            .lookup(SETUP_ACCOUNT, "identity-index-v1")
    }
    pub fn identity_pointer(&mut self, record: Option<&Record>) -> Result<Option<String>> {
        let keys = self.keys.as_ref().ok_or(Error::Locked)?;
        self.identity_account = match record {
            Some(r) => serde_json::from_slice(&keys.open(SETUP_ACCOUNT, "identity-index-v1", r)?)
                .map_err(|_| Error::Malformed)?,
            None => None,
        };
        self.identity_account
            .as_ref()
            .map(|a| keys.lookup(a, crate::identity::RECORD_NAME))
            .transpose()
    }
    pub fn identity_restore(&mut self, record: &Record) -> Result<()> {
        let account = self.identity_account.as_ref().ok_or(Error::Malformed)?;
        let plain = self.keys.as_ref().ok_or(Error::Locked)?.open(
            account,
            crate::identity::RECORD_NAME,
            record,
        )?;
        let saved: crate::identity::Remembered =
            serde_json::from_slice(&plain).map_err(|_| Error::Malformed)?;
        if &saved.account != account {
            return Err(Error::Authentication);
        }
        self.identity.restore(saved)
    }
    pub fn identity_prepare(
        &mut self,
        request: crate::identity::Request,
        revision: u32,
        now: u64,
        entropy: &mut dyn Entropy,
    ) -> Result<IdentityPrepared> {
        use crate::identity::{Action, Output};
        let binding = self.identity_binding()?;
        if request.request_id.is_empty() || request.request_id.len() > 64 {
            return Err(Error::Malformed);
        }
        if request.binding.vault_id != binding.vault_id
            || request.binding.token != binding.token
            || request.binding.generation != binding.generation
            || (!matches!(request.action, Action::Status) && request.binding != binding)
        {
            return Err(Error::Cancelled);
        }
        if self.status.revision != revision {
            return Err(Error::Conflict);
        }
        let previous = self.identity_account.clone();
        let persist = self
            .identity
            .handle(request.action, &binding, now, entropy)?;
        let mutation = if persist {
            let next = revision.checked_add(1).ok_or(Error::Limit)?;
            let remembered = self.identity.remembered()?;
            let account = remembered.as_ref().map(|r| r.account.clone());
            let keys = self.keys.as_ref().ok_or(Error::Locked)?;
            let mut records = vec![keys.seal(
                SETUP_ACCOUNT,
                "identity-index-v1",
                next,
                &serde_json::to_vec(&account).map_err(|_| Error::Malformed)?,
                entropy,
            )?];
            if let Some(saved) = remembered {
                let plain =
                    Zeroizing::new(serde_json::to_vec(&saved).map_err(|_| Error::Malformed)?);
                records.push(keys.seal(
                    &saved.account,
                    crate::identity::RECORD_NAME,
                    next,
                    &plain,
                    entropy,
                )?);
            }
            if let Some(old) = previous.filter(|p| Some(p) != account.as_ref()) {
                records.push(keys.seal(
                    &old,
                    crate::identity::RECORD_NAME,
                    next,
                    b"null",
                    entropy,
                )?);
            }
            self.identity_account = account;
            Some(Mutation {
                expected_revision: revision,
                header: None,
                records,
                create: false,
            })
        } else {
            None
        };
        Ok(IdentityPrepared {
            output: Output {
                request_id: request.request_id,
                binding: self.identity_binding()?,
                view: self.identity.view(),
                effect: self.identity.pending(),
            },
            mutation,
        })
    }
    pub fn identity_committed(&mut self, revision: u32) {
        self.status.revision = revision;
    }
    pub fn identity_auth(
        &mut self,
        relay: &str,
        challenge: &str,
        connection: &str,
        now: u64,
        entropy: &mut dyn Entropy,
    ) -> Result<crate::identity::Effect> {
        let binding = self.identity_binding()?;
        self.identity
            .begin_auth((relay, challenge, connection), &binding, now, entropy)?;
        self.identity.pending().ok_or(Error::Cancelled)
    }
    pub fn identity_take_auth(&mut self) -> Result<Option<crate::identity::VerifiedAuth>> {
        self.identity_binding()?;
        Ok(self.identity.take_auth())
    }
    pub fn identity_cancel_auth(&mut self) {
        self.identity.cancel_auth();
    }
    pub fn identity_effect(
        &self,
        binding: &crate::identity::Binding,
        id: &str,
        now: u64,
    ) -> Result<crate::identity::Effect> {
        if &self.identity_binding()? != binding {
            return Err(Error::Cancelled);
        }
        self.identity
            .pending()
            .filter(|e| e.id == id && e.deadline >= now)
            .ok_or(Error::Cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct TestEntropy(u8);
    impl Entropy for TestEntropy {
        fn fill(&mut self, bytes: &mut [u8]) -> Result<()> {
            for b in bytes {
                self.0 = self.0.wrapping_add(1);
                *b = self.0;
            }
            Ok(())
        }
    }
    #[test]
    fn production_context_tampering_and_password_rewrap() {
        let mut random = TestEntropy(0);
        let keys = Keys::create("test-only password", &mut random).unwrap();
        let record = keys
            .seal(
                "account-a",
                "message",
                1,
                b"PRIVATE TEST CONTENT",
                &mut random,
            )
            .unwrap();
        assert_eq!(
            &*keys.open("account-a", "message", &record).unwrap(),
            b"PRIVATE TEST CONTENT"
        );
        for (account, name) in [("account-b", "message"), ("account-a", "other")] {
            assert_eq!(
                keys.open(account, name, &record).unwrap_err(),
                Error::Authentication
            );
        }
        for index in [0, 24, record.bytes.len() - 1] {
            let mut modified = record.clone();
            modified.bytes[index] ^= 1;
            assert_eq!(
                keys.open("account-a", "message", &modified).unwrap_err(),
                Error::Authentication
            );
        }
        let mut modified = record.clone();
        modified.revision += 1;
        assert_eq!(
            keys.open("account-a", "message", &modified).unwrap_err(),
            Error::Authentication
        );
        let mut excessive = keys.header.clone();
        excessive.memory_kib = u32::MAX;
        assert!(matches!(
            Keys::unlock(excessive, "test-only password"),
            Err(Error::Unsupported)
        ));
        assert!(matches!(
            Keys::unlock(keys.header.clone(), "incorrect password"),
            Err(Error::Authentication)
        ));
        let replacement = keys
            .rewrap("test-only password", "new test password", &mut random)
            .unwrap();
        assert_ne!(keys.header.salt, replacement.salt);
        assert_ne!(keys.header.nonce, replacement.nonce);
        let reopened = Keys::unlock(replacement.clone(), "new test password").unwrap();
        assert_eq!(
            &*reopened.open("account-a", "message", &record).unwrap(),
            b"PRIVATE TEST CONTENT"
        );
        assert!(matches!(
            Keys::unlock(replacement, "test-only password"),
            Err(Error::Authentication)
        ));
        for field in 0..3 {
            let mut header = keys.header.clone();
            match field {
                0 => header.salt[0] ^= 1,
                1 => header.nonce[0] ^= 1,
                _ => header.wrapped[0] ^= 1,
            }
            assert!(matches!(
                Keys::unlock(header, "test-only password"),
                Err(Error::Authentication)
            ));
        }
        let mut tampered = keys.header.clone();
        tampered.vault_id = "ff".repeat(16);
        assert!(matches!(
            Keys::unlock(tampered, "test-only password"),
            Err(Error::Authentication)
        ));
    }
    #[test]
    fn policy_and_randomness_fail_closed() {
        struct Failed;
        impl Entropy for Failed {
            fn fill(&mut self, _: &mut [u8]) -> Result<()> {
                Err(Error::Randomness)
            }
        }
        assert!(matches!(
            Keys::create("test-only password", &mut Failed),
            Err(Error::Randomness)
        ));
        assert_eq!(password("short", true), Err(Error::PasswordPolicy));
        assert!(password("  preserved spaces  ", true).is_ok());
        assert_eq!(password(&"x".repeat(1025), false), Err(Error::Limit));
        assert!(Header::parse(&vec![0; 4097]).is_err());
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    struct PublicEntropy(u8);
    impl Entropy for PublicEntropy {
        fn fill(&mut self, bytes: &mut [u8]) -> Result<()> {
            for b in bytes {
                self.0 = self.0.wrapping_add(1);
                *b = self.0;
            }
            Ok(())
        }
    }
    fn request(status: &Status, operation: Operation) -> Request {
        Request {
            version: 1,
            request_id: "lifecycle".into(),
            token: status.token.clone(),
            generation: status.generation,
            vault_id: status.vault_id.clone(),
            operation,
        }
    }
    #[test]
    fn lock_invalidates_pending_unlock_write_and_password_change() {
        let mut entropy = PublicEntropy(0);
        let mut session = Session::new(&mut entropy).unwrap();
        let empty = Snapshot {
            header: None,
            revision: 0,
            setup: None,
        };
        let created = session
            .prepare(
                &request(
                    &session.status(),
                    Operation::Create {
                        password: "lifecycle fixture password".into(),
                        confirmation: "lifecycle fixture password".into(),
                    },
                ),
                &empty,
                &mut entropy,
            )
            .unwrap()
            .unwrap();
        session.committed().unwrap();
        let snapshot = Snapshot {
            header: created.header.clone(),
            revision: 1,
            setup: None,
        };
        let setup = created.records[0].clone();
        let save = request(
            &session.status(),
            Operation::SaveSetup {
                setup: Default::default(),
                revision: 1,
            },
        );
        session.prepare(&save, &snapshot, &mut entropy).unwrap();
        session.lock();
        assert_eq!(session.committed(), Err(Error::Conflict));
        assert_eq!(session.lookup_setup(), Err(Error::Locked));
        session
            .prepare(
                &request(
                    &session.status(),
                    Operation::Unlock {
                        password: "lifecycle fixture password".into(),
                    },
                ),
                &snapshot,
                &mut entropy,
            )
            .unwrap();
        session.lock();
        assert_eq!(session.finish_unlock(&setup), Err(Error::Cancelled));
        session
            .prepare(
                &request(
                    &session.status(),
                    Operation::Unlock {
                        password: "lifecycle fixture password".into(),
                    },
                ),
                &snapshot,
                &mut entropy,
            )
            .unwrap();
        session.finish_unlock(&setup).unwrap();
        session
            .prepare(
                &request(
                    &session.status(),
                    Operation::ChangePassword {
                        current: "lifecycle fixture password".into(),
                        password: "replacement fixture password".into(),
                        confirmation: "replacement fixture password".into(),
                    },
                ),
                &snapshot,
                &mut entropy,
            )
            .unwrap();
        session.lock();
        assert_eq!(session.committed(), Err(Error::Conflict));
        assert!(session.status().setup.is_none());
        assert_eq!(session.status().state, State::Locked);
    }
}

impl Session {
    pub fn backup_validate(&self, binding: &crate::backup::Binding) -> Result<()> {
        if self.status.state != State::Unlocked {
            return Err(Error::Locked);
        }
        if self.status.vault_id.as_ref() != Some(&binding.vault_id)
            || self.status.token != binding.token
            || self.status.generation != binding.generation
        {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
    pub fn backup_account(&self) -> Result<String> {
        let view = self.identity.view();
        if view.state != "connected" {
            return Err(Error::Authentication);
        }
        view.account.ok_or(Error::Authentication)
    }
    pub fn backup_lookup(&self) -> Result<String> {
        self.keys
            .as_ref()
            .ok_or(Error::Locked)?
            .lookup(SETUP_ACCOUNT, crate::backup::RECORD)
    }
    pub fn backup_open(&self, record: &Record) -> Result<crate::backup::Backup> {
        let bytes = self.keys.as_ref().ok_or(Error::Locked)?.open(
            SETUP_ACCOUNT,
            crate::backup::RECORD,
            record,
        )?;
        crate::backup::Backup::parse(&bytes)
    }
    pub fn backup_seal(
        &self,
        data: &crate::backup::Backup,
        revision: u32,
        entropy: &mut dyn Entropy,
    ) -> Result<Mutation> {
        let bytes = Zeroizing::new(data.bytes()?);
        let record = self.keys.as_ref().ok_or(Error::Locked)?.seal(
            SETUP_ACCOUNT,
            crate::backup::RECORD,
            revision.checked_add(1).ok_or(Error::Limit)?,
            &bytes,
            entropy,
        )?;
        Ok(Mutation {
            expected_revision: revision,
            header: None,
            records: vec![record],
            create: false,
        })
    }
}
