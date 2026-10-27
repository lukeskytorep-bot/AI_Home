use super::*;
/// Local validated protocol value, not a generic signing request.
pub(super) struct Proof {
    pub(super) scope_binding: String,
    pub(super) agent_id: String,
    pub(super) operational_key_id: String,
    pub(super) challenge_id: String,
    pub(super) generation: Option<i64>,
    pub(super) expires: DateTime<Utc>,
    statement: String,
}
impl Proof {
    pub(super) fn validate(
        scope: &CommunityScope,
        id: &str,
        purpose: &str,
        response: &Value,
        agent: Option<&str>,
        key: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<Self, IdentityError> {
        if !prefixed(id, "reg_")
            || !["status", "reissue"].contains(&purpose)
            || response["ok"] != true
            || !response["request_id"]
                .as_str()
                .is_some_and(|s| scalar(s, 256))
        {
            return Err(IdentityError::Invalid);
        }
        let c = &response["challenge"];
        if c["community_origin"] != scope.origin
            || c["community_instance"] != scope.instance
            || c["registration_id"] != id
            || c["purpose"] != purpose
        {
            return Err(IdentityError::ScopeMismatch);
        }
        let a = field(c, "agent_id", 128)?;
        let k = field(c, "operational_key_id", 128)?;
        let ch = field(c, "challenge_id", 128)?;
        if !prefixed(&a, "agt_") || !prefixed(&k, "key_") || !prefixed(&ch, "chl_") {
            return Err(IdentityError::Invalid);
        }
        if agent.is_some_and(|v| v != a) || key.is_some_and(|v| v != k) {
            return Err(IdentityError::ScopeMismatch);
        }
        let nonce = field(c, "nonce", 128)?;
        if crypto::decode(&nonce, 32)?.len() != 32 {
            return Err(IdentityError::Invalid);
        }
        let expiry = field(c, "expires_at", 64)?;
        let expires = DateTime::parse_from_rfc3339(&expiry)
            .map_err(|_| IdentityError::Invalid)?
            .with_timezone(&Utc);
        if expires.to_rfc3339_opts(SecondsFormat::Secs, true) != expiry
            || expires > now + chrono::Duration::hours(24)
        {
            return Err(IdentityError::Invalid);
        }
        if expires < now {
            return Err(IdentityError::Expired);
        }
        let generation = if purpose == "status" {
            if c.get("expected_token_generation") != Some(&Value::Null) {
                return Err(IdentityError::Invalid);
            }
            None
        } else {
            Some(
                c["expected_token_generation"]
                    .as_i64()
                    .filter(|v| *v >= 1 && *v < i64::MAX)
                    .ok_or(IdentityError::Invalid)?,
            )
        };
        let (format, method, route, body) = if let Some(g) = generation {
            (
                "AGENT-REGISTRATION-REISSUE-PROOF/1",
                "POST",
                format!("/api/v1/registration/{id}/token/reissue"),
                format!("expected_token_generation={g}\n"),
            )
        } else {
            (
                "AGENT-REGISTRATION-STATUS-PROOF/1",
                "GET",
                format!("/api/v1/registration/{id}/status"),
                String::new(),
            )
        };
        let statement=format!("{format}\ncommunity_origin={}\ncommunity_instance={}\nregistration_id={id}\nagent_id={a}\noperational_key_id={k}\npurpose={purpose}\nmethod={method}\nroute={route}\nchallenge_id={ch}\nnonce={nonce}\nexpires_at={expiry}\nbody_sha256={}\nsignature_algorithm={ALGORITHM}\npublic_key_format={PUBLIC_FORMAT}\nsignature_format={SIGNATURE_FORMAT}\n",scope.origin,scope.instance,crypto::digest(body.as_bytes()));
        Ok(Self {
            scope_binding: scope.binding(),
            agent_id: a,
            operational_key_id: k,
            challenge_id: ch,
            generation,
            expires,
            statement,
        })
    }
    pub(super) fn bytes(&self) -> &[u8] {
        self.statement.as_bytes()
    }
}
