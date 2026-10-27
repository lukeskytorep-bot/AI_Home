<?php
declare(strict_types=1);

const COMMUNITY_AGENT_SCHEMA = 'community-agent/1';
const COMMUNITY_KEY_HISTORY_SCHEMA = 'community-key-history/1';
const COMMUNITY_API_TOKEN_SCHEMA = 'community-api-token/1';

// Domain signed-format identifiers are reserved now so stored records can carry
// explicit versions. Their exact canonical byte serialization is NOT frozen in
// 4A-2. AGENT-REGISTRATION/1 is frozen in 4A-3 before any registration endpoint
// uses it; content/recovery formats are frozen in their own implementation phases.
const COMMUNITY_REGISTRATION_FORMAT = 'AGENT-REGISTRATION/1';
const COMMUNITY_CONTENT_REVISION_FORMAT = 'COMMUNITY-CONTENT-REVISION/1';
const COMMUNITY_IDENTITY_RECOVERY_FORMAT = 'IDENTITY-RECOVERY/1';
const COMMUNITY_REGISTRATION_SERIALIZATION_STATUS = 'frozen-in-4A-3';
const COMMUNITY_CONTENT_SERIALIZATION_STATUS = 'frozen-in-4A-5';
const COMMUNITY_RECOVERY_SERIALIZATION_STATUS = 'frozen-in-4A-8A';
const COMMUNITY_RECOVERY_PROOF_FORMAT = 'IDENTITY-RECOVERY-PROOF/1';
const COMMUNITY_RECOVERY_STATE_SCHEMA = 'community-recovery-state/1';
const COMMUNITY_TRUSTED_DEVICE_SCHEMA = 'community-trusted-device/1';

// 4A-2 frozen test-host cryptographic suite. This identifier is persisted with
// keys and signed objects so no future adapter can silently reinterpret bytes.
const COMMUNITY_SIGNATURE_ALGORITHM = 'openssl-ecdsa-p256-sha256-der/1';
const COMMUNITY_PUBLIC_KEY_FORMAT = 'spki-der-base64url/1';
const COMMUNITY_SIGNATURE_FORMAT = 'asn1-der-base64url/1';

// 4A-6 semantic retry fingerprint. The raw Idempotency-Key is never stored.
const COMMUNITY_IDEMPOTENCY_FINGERPRINT_FORMAT = 'COMMUNITY-IDEMPOTENCY-FINGERPRINT/1';
