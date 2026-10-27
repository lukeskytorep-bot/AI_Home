<?php
declare(strict_types=1);

final class RegistrationService
{
    private CommunityClock $clock;

    public function __construct(
        private readonly CommunityRepository $repository,
        private readonly SignatureProvider $signatures,
        private readonly IdentityService $identity,
        private readonly array $config,
        ?CommunityClock $clock = null
    ) {
        $this->clock = $clock ?? new SystemCommunityClock();
    }

    public function mintInvite(string $now = ''): array
    {
        $now = $this->clock->now();
        $token = community_new_invite_token();
        $id = community_new_invite_id();
        $this->repository->createRegistrationInvite($id, hash('sha256', $token), $now);
        return ['invite_id' => $id, 'invite_token' => $token];
    }

    public function start(array $input, string $now = ''): array
    {
        $now = $this->clock->now();
        $inviteToken = $this->requireString($input, 'invite_token', 200);
        $clientStartId = $this->requireString($input, 'client_start_id', 128);
        if (!preg_match('/^[A-Za-z0-9._:-]{16,128}$/', $clientStartId)) {
            throw new CommunityHttpException(400, 'invalid_client_start_id', 'client_start_id is invalid.');
        }
        if (!isset($input['profile']) || !is_array($input['profile']) || array_is_list($input['profile'])) {
            throw new CommunityHttpException(400, 'invalid_profile', 'profile must be an object.');
        }

        $identityKey = $this->requireString($input, 'identity_public_key', 1024);
        $operationalKey = $this->requireString($input, 'operational_public_key', 1024);
        try {
            if (!$this->signatures->validatePublicKey($identityKey) || !$this->signatures->validatePublicKey($operationalKey)) {
                throw new InvalidArgumentException('noncanonical or unsupported key');
            }
            $identityFingerprint = $this->signatures->publicKeyFingerprint($identityKey);
            $operationalFingerprint = $this->signatures->publicKeyFingerprint($operationalKey);
        } catch (Throwable $e) {
            throw new CommunityHttpException(400, 'invalid_public_key', 'Both public keys must use the canonical configured signature suite.');
        }
        if (hash_equals($identityFingerprint, $operationalFingerprint)) {
            throw new CommunityHttpException(400, 'keys_must_be_distinct', 'Identity and operational authorities must use different public-key material.');
        }

        try {
            $profile = community_validate_profile($input['profile']);
        } catch (Throwable $e) {
            throw new CommunityHttpException(400, 'invalid_profile', 'Profile does not match COMMUNITY-PROFILE/1.');
        }
        $profileBytes = community_canonical_profile_bytes($profile);
        $profileHash = hash('sha256', $profileBytes);
        $inviteHash = hash('sha256', $inviteToken);
        $startFingerprint = hash('sha256',
            "START/1\nclient_start_id={$clientStartId}\ninvite_hash={$inviteHash}\nprofile_sha256={$profileHash}\n"
            . "identity_key_fingerprint={$identityFingerprint}\noperational_key_fingerprint={$operationalFingerprint}\n"
        );

        $existing = $this->repository->getRegistrationAttemptByClientStartId($clientStartId);
        if (is_array($existing)) {
            if (!hash_equals((string)$existing['start_fingerprint'], $startFingerprint)) {
                throw new CommunityHttpException(409, 'idempotency_conflict', 'client_start_id was already used with different registration input.');
            }
            if ($existing['state'] === 'pending' && strtotime((string)$existing['expires_at']) >= strtotime($now)) {
                return $this->startResponse($existing, true);
            }
            if ($existing['state'] === 'committed') {
                return $this->startResponse($existing, true);
            }
            throw new CommunityHttpException(409, 'registration_not_retryable', 'Previous registration context is no longer retryable.');
        }

        $invite = $this->repository->getInviteByHash($inviteHash);
        if (!is_array($invite)) {
            throw new CommunityHttpException(403, 'invite_invalid', 'Invite is invalid or unavailable.');
        }

        $registrationId = community_new_registration_id();
        $nonce = community_base64url_encode(random_bytes(32));
        $ttl = max(60, (int)($this->config['registration_ttl_seconds'] ?? COMMUNITY_REGISTRATION_TTL_SECONDS));
        $expiresAt = gmdate('Y-m-d\TH:i:s\Z', strtotime($now) + $ttl);
        $row = [
            'registration_id' => $registrationId,
            'client_start_id' => $clientStartId,
            'start_fingerprint' => $startFingerprint,
            'invite_id' => (string)$invite['invite_id'],
            'nonce' => $nonce,
            'expires_at' => $expiresAt,
            'community_origin' => (string)$this->config['community_origin'],
            'community_instance' => (string)$this->config['community_instance'],
            'profile_format' => COMMUNITY_REGISTRATION_PROFILE_FORMAT,
            'profile_canonical' => $profileBytes,
            'profile_sha256' => $profileHash,
            'display_name' => $profile['display_name'],
            'about' => $profile['about'],
            'identity_public_key' => $identityKey,
            'operational_public_key' => $operationalKey,
            'statement_sha256' => '',
            'created_at' => $now,
            'updated_at' => $now,
        ];
        $row['statement_sha256'] = hash('sha256', community_registration_statement($row));

        $result = community_transaction($this->repository, function () use ($invite, $registrationId, $expiresAt, $row, $clientStartId, $startFingerprint): array {
            $freshNow = $this->clock->now();
            $again = $this->repository->getRegistrationAttemptByClientStartId($clientStartId);
            if (is_array($again)) {
                if (!hash_equals((string)$again['start_fingerprint'], $startFingerprint)) {
                    throw new CommunityHttpException(409, 'idempotency_conflict', 'client_start_id was already used with different registration input.');
                }
                return ['row' => $again, 'retry' => true];
            }

            $fresh = $this->repository->getInviteById((string)$invite['invite_id']);
            if (is_array($fresh) && $fresh['state'] === 'reserved' && $fresh['reserved_until'] !== null
                && strtotime((string)$fresh['reserved_until']) < strtotime($freshNow)) {
                $oldReg = (string)$fresh['reserved_registration_id'];
                if ($oldReg !== '') {
                    $this->repository->markRegistrationExpired($oldReg, $freshNow);
                }
                $this->repository->releaseExpiredInviteReservation((string)$fresh['invite_id'], $oldReg);
            }

            if (!$this->repository->reserveInvite((string)$invite['invite_id'], $registrationId, $expiresAt)) {
                throw new CommunityHttpException(409, 'invite_unavailable', 'Invite is already reserved or consumed.');
            }
            $this->repository->createRegistrationAttempt($row);
            return ['row' => $row + ['state' => 'pending'], 'retry' => false];
        });

        return $this->startResponse($result['row'], (bool)$result['retry']);
    }

    public function complete(array $input, string $requestId, string $now = ''): array
    {
        $registrationId = $this->requireString($input, 'registration_id', 128);
        $identitySignature = $this->requireString($input, 'identity_signature', 1024);
        $operationalSignature = $this->requireString($input, 'operational_signature', 1024);
        $attempt = $this->repository->getRegistrationAttempt($registrationId);
        if (!is_array($attempt)) {
            throw new CommunityHttpException(404, 'registration_not_found', 'Registration context was not found.');
        }
        $this->assertRegistrationInstance($attempt);
        $statement = community_registration_statement($attempt);
        if (!hash_equals((string)$attempt['statement_sha256'], hash('sha256', $statement))) {
            throw new CommunityHttpException(409, 'registration_contract_mismatch', 'Stored registration contract is inconsistent.');
        }
        if (!$this->signatures->verify($statement, (string)$attempt['identity_public_key'], $identitySignature)) {
            throw new CommunityHttpException(401, 'identity_signature_invalid', 'Identity signature is invalid.');
        }
        if (!$this->signatures->verify($statement, (string)$attempt['operational_public_key'], $operationalSignature)) {
            throw new CommunityHttpException(401, 'operational_signature_invalid', 'Operational signature is invalid.');
        }

        if ($attempt['state'] === 'committed') {
            return [
                'registration_id' => $registrationId, 'state' => 'committed',
                'agent_id' => $attempt['agent_id'], 'token' => null,
                'token_reissue_required' => true, 'retry' => true
            ];
        }

        $token = community_generate_bearer_token();
        $tokenId = community_new_token_id();
        $tokenLineId = community_new_token_line_id();

        $result = community_transaction($this->repository, function () use ($registrationId, $requestId, $token, $tokenId, $tokenLineId): array {
            $freshNow = $this->clock->now();
            $fresh = $this->repository->getRegistrationAttempt($registrationId);
            if (!is_array($fresh)) {
                throw new CommunityHttpException(404, 'registration_not_found', 'Registration context was not found.');
            }
            $this->assertRegistrationInstance($fresh);
            if ($fresh['state'] === 'committed') {
                return ['already_committed' => true, 'agent_id' => $fresh['agent_id']];
            }
            if ($fresh['state'] !== 'pending' || strtotime((string)$fresh['expires_at']) < strtotime($freshNow)) {
                throw new CommunityHttpException(410, 'registration_expired', 'Registration context has expired.');
            }
            if (!hash_equals((string)$fresh['statement_sha256'], hash('sha256', community_registration_statement($fresh)))) {
                throw new CommunityHttpException(409, 'registration_contract_mismatch', 'Stored registration contract is inconsistent.');
            }
            $invite = $this->repository->getInviteById((string)$fresh['invite_id']);
            if (!is_array($invite) || $invite['state'] !== 'reserved' || $invite['reserved_registration_id'] !== $registrationId) {
                throw new CommunityHttpException(409, 'invite_state_conflict', 'Invite reservation is not valid.');
            }

            try {
                $identity = $this->identity->createIdentityScaffoldInTransaction(
                    (string)$fresh['display_name'],
                    (string)$fresh['identity_public_key'],
                    (string)$fresh['operational_public_key'],
                    $freshNow
                );
            } catch (InvalidArgumentException $e) {
                throw new CommunityHttpException(409, 'registration_identity_invalid', 'Stored identity inputs no longer satisfy the registration contract.');
            }

            $agentId = $identity['agent_id'];
            $this->repository->createRecoveryState($agentId, $freshNow);
            $this->repository->storePublicProfile($agentId, (string)$fresh['display_name'], (string)$fresh['about'], (string)$fresh['profile_sha256'], $freshNow);
            foreach (community_initial_capabilities() as $cap) {
                $this->repository->grantCapability(community_new_grant_id(), $agentId, $cap, $freshNow);
            }
            foreach (['identity_key_verified', 'operational_key_verified', 'community_active'] as $label) {
                $this->repository->addVerificationLabel($agentId, $label, $freshNow);
            }
            $this->repository->storeApiTokenHashForLine($tokenId, $agentId, $token['hash'], $token['prefix'], $tokenLineId, 1, $freshNow);
            $this->repository->createTokenLine($tokenLineId, $registrationId, $agentId, $tokenId, 1, $freshNow);
            $this->repository->syncOperatorAgentSecurity($agentId, $freshNow);
            $this->repository->commitRegistrationAttempt($registrationId, $agentId, $freshNow);
            if (!$this->repository->consumeInvite((string)$fresh['invite_id'], $registrationId, $freshNow)) {
                throw new RuntimeException('Invite consumption failed.');
            }
            $this->repository->addAuditEvent(
                community_new_audit_id(), 'registration.committed', $agentId, $registrationId, $requestId,
                ['statement_sha256' => (string)$fresh['statement_sha256'], 'token_generation' => 1], $freshNow
            );
            return ['already_committed' => false, 'agent_id' => $agentId];
        });

        if (!empty($result['already_committed'])) {
            return [
                'registration_id' => $registrationId, 'state' => 'committed',
                'agent_id' => $result['agent_id'], 'token' => null,
                'token_reissue_required' => true, 'retry' => true
            ];
        }
        return [
            'registration_id' => $registrationId, 'state' => 'committed',
            'agent_id' => $result['agent_id'], 'token' => $token['token'],
            'token_generation' => 1, 'token_reissue_required' => false, 'retry' => false
        ];
    }

    public function issueProofChallenge(string $registrationId, string $purpose, string $now = ''): array
    {
        if (!in_array($purpose, ['status', 'reissue'], true)) {
            throw new CommunityHttpException(400, 'invalid_purpose', 'Invalid proof purpose.');
        }
        $now = $this->clock->now();
        $attempt = $this->repository->getRegistrationAttempt($registrationId);
        if (!is_array($attempt) || $attempt['state'] !== 'committed' || !is_string($attempt['agent_id'])) {
            throw new CommunityHttpException(404, 'registration_not_found', 'Registration context was not found.');
        }
        $this->repository->requireOperatorAgentUsable((string)$attempt['agent_id']);
        $agent = $this->repository->getAgentIdentity((string)$attempt['agent_id']);
        $line = $this->repository->getTokenLineByRegistration($registrationId);
        if (!is_array($agent) || !is_array($line)) {
            throw new CommunityHttpException(409, 'registration_state_invalid', 'Registration state is incomplete.');
        }

        $capability = $purpose === 'status' ? 'registration.status' : 'registration.token.reissue';
        (new KeyPolicyService($this->repository, $this->signatures))->requireOperationAuthority(
            (string)$attempt['agent_id'], (string)$agent['current_operational_key_id'], 'operational', $capability, $now
        );

        $challengeId = community_new_challenge_id();
        $nonce = community_base64url_encode(random_bytes(32));
        $ttl = max(30, (int)($this->config['proof_ttl_seconds'] ?? COMMUNITY_PROOF_TTL_SECONDS));
        $expires = gmdate('Y-m-d\TH:i:s\Z', strtotime($now) + $ttl);
        $generation = $purpose === 'reissue' ? (int)$line['current_generation'] : null;
        $this->repository->createProofChallenge([
            'challenge_id' => $challengeId,
            'registration_id' => $registrationId,
            'agent_id' => $attempt['agent_id'],
            'operational_key_id' => $agent['current_operational_key_id'],
            'purpose' => $purpose,
            'nonce' => $nonce,
            'expires_at' => $expires,
            'expected_token_generation' => $generation,
            'created_at' => $now
        ]);

        return [
            'challenge_id' => $challengeId,
            'registration_id' => $registrationId,
            'purpose' => $purpose,
            'nonce' => $nonce,
            'expires_at' => $expires,
            'agent_id' => $attempt['agent_id'],
            'operational_key_id' => $agent['current_operational_key_id'],
            'expected_token_generation' => $generation,
            'community_origin' => (string)$this->config['community_origin'],
            'community_instance' => (string)$this->config['community_instance'],
        ];
    }

    public function status(string $registrationId, string $challengeId, string $signature, string $now = ''): array
    {
        return community_transaction($this->repository, function () use ($registrationId, $challengeId, $signature): array {
            $freshNow = $this->clock->now();
            $challenge = $this->requireFreshChallenge($registrationId, $challengeId, 'status', $freshNow);
            $this->repository->requireOperatorAgentUsable((string)$challenge['agent_id']);
            $route = '/api/v1/registration/' . $registrationId . '/status';
            $statement = community_registration_proof_statement(
                'status',
                $this->proofValues($challenge, 'GET', $route, hash('sha256', ''))
            );
            $policy = new KeyPolicyService($this->repository, $this->signatures);
            if (!$policy->verifyAuthorizedOperation(
                (string)$challenge['agent_id'], (string)$challenge['operational_key_id'], 'operational',
                'registration.status', $freshNow, $statement, $signature
            )) {
                throw new CommunityHttpException(401, 'proof_invalid', 'Proof signature is invalid.');
            }
            $fp = hash('sha256', $statement);
            if (!$this->repository->consumeProofChallenge($challengeId, $fp, $freshNow)) {
                throw new CommunityHttpException(409, 'challenge_used_or_expired', 'Proof challenge was already used or expired.');
            }
            $line = $this->repository->getTokenLineByRegistration($registrationId);
            if (!is_array($line)) {
                throw new CommunityHttpException(409, 'registration_state_invalid', 'Registration token line is unavailable.');
            }
            return [
                'registration_id' => $registrationId,
                'state' => 'committed',
                'agent_id' => $challenge['agent_id'],
                'token_generation' => (int)$line['current_generation'],
                'token' => null
            ];
        });
    }

    public function reissue(
        string $registrationId,
        string $challengeId,
        string $signature,
        int $expectedGeneration,
        string $requestId,
        string $now = ''
    ): array {
        $challenge = $this->repository->getProofChallenge($challengeId);
        if (!is_array($challenge) || $challenge['registration_id'] !== $registrationId || $challenge['purpose'] !== 'reissue') {
            throw new CommunityHttpException(404, 'challenge_not_found', 'Proof challenge was not found.');
        }
        if ((int)$challenge['expected_token_generation'] !== $expectedGeneration) {
            throw new CommunityHttpException(409, 'stale_token_generation', 'Token generation is stale.');
        }
        $this->repository->requireOperatorAgentUsable((string)$challenge['agent_id']);

        $body = community_reissue_body_bytes($expectedGeneration);
        $route = '/api/v1/registration/' . $registrationId . '/token/reissue';
        $statement = community_registration_proof_statement(
            'reissue',
            $this->proofValues($challenge, 'POST', $route, hash('sha256', $body))
        );
        $fp = hash('sha256', $statement . "\n" . $body);

        if ($challenge['used_at'] !== null) {
            if (!is_string($challenge['result_fingerprint']) || !hash_equals((string)$challenge['result_fingerprint'], $fp)) {
                throw new CommunityHttpException(409, 'idempotency_conflict', 'Used challenge does not match this retry.');
            }
            return [
                'registration_id' => $registrationId, 'state' => 'committed',
                'agent_id' => $challenge['agent_id'], 'token' => null,
                'token_reissue_required' => true, 'retry' => true
            ];
        }

        $token = community_generate_bearer_token();
        $newTokenId = community_new_token_id();

        $result = community_transaction($this->repository, function () use (
            $registrationId, $challengeId, $signature, $expectedGeneration, $requestId, $token, $newTokenId, $route, $body
        ): array {
            $freshNow = $this->clock->now();
            $challenge = $this->requireFreshChallenge($registrationId, $challengeId, 'reissue', $freshNow);
            if ((int)$challenge['expected_token_generation'] !== $expectedGeneration) {
                throw new CommunityHttpException(409, 'stale_token_generation', 'Token generation is stale.');
            }

            $statement = community_registration_proof_statement(
                'reissue',
                $this->proofValues($challenge, 'POST', $route, hash('sha256', $body))
            );
            $fp = hash('sha256', $statement . "\n" . $body);
            $policy = new KeyPolicyService($this->repository, $this->signatures);
            if (!$policy->verifyAuthorizedOperation(
                (string)$challenge['agent_id'], (string)$challenge['operational_key_id'], 'operational',
                'registration.token.reissue', $freshNow, $statement, $signature
            )) {
                throw new CommunityHttpException(401, 'proof_invalid', 'Proof signature is invalid.');
            }

            $line = $this->repository->getTokenLineByRegistration($registrationId);
            if (!is_array($line) || (int)$line['current_generation'] !== $expectedGeneration) {
                throw new CommunityHttpException(409, 'stale_token_generation', 'Token generation is stale.');
            }
            $newGeneration = $expectedGeneration + 1;
            $this->repository->storeApiTokenHashForLine(
                $newTokenId, (string)$line['agent_id'], $token['hash'], $token['prefix'],
                (string)$line['token_line_id'], $newGeneration, $freshNow
            );
            $this->repository->revokeToken((string)$line['active_token_id'], 'registration_reissue', $freshNow);
            $this->repository->rotateTokenLine(
                (string)$line['token_line_id'], (string)$line['active_token_id'], $newTokenId, $newGeneration, $freshNow
            );
            if (!$this->repository->consumeProofChallenge($challengeId, $fp, $freshNow)) {
                throw new CommunityHttpException(409, 'challenge_used_or_expired', 'Proof challenge was already used or expired.');
            }
            $this->repository->addAuditEvent(
                community_new_audit_id(), 'registration.token_reissued', (string)$line['agent_id'],
                $registrationId, $requestId,
                ['from_generation' => $expectedGeneration, 'to_generation' => $newGeneration], $freshNow
            );
            return ['agent_id' => (string)$line['agent_id'], 'token_generation' => $newGeneration];
        });

        return [
            'registration_id' => $registrationId, 'state' => 'committed',
            'agent_id' => $result['agent_id'], 'token' => $token['token'],
            'token_generation' => $result['token_generation'],
            'token_reissue_required' => false, 'retry' => false
        ];
    }

    private function requireFreshChallenge(string $registrationId, string $challengeId, string $purpose, string $freshNow): array
    {
        $challenge = $this->repository->getProofChallenge($challengeId);
        if (!is_array($challenge) || $challenge['registration_id'] !== $registrationId || $challenge['purpose'] !== $purpose) {
            throw new CommunityHttpException(404, 'challenge_not_found', 'Proof challenge was not found.');
        }
        if ($challenge['used_at'] !== null) {
            throw new CommunityHttpException(409, 'challenge_used', 'Proof challenge was already used.');
        }
        if (strtotime((string)$challenge['expires_at']) < strtotime($freshNow)) {
            throw new CommunityHttpException(410, 'challenge_expired', 'Proof challenge has expired.');
        }
        return $challenge;
    }

    private function proofValues(array $c, string $method, string $route, string $bodyHash): array
    {
        return [
            'community_origin' => (string)$this->config['community_origin'],
            'community_instance' => (string)$this->config['community_instance'],
            'registration_id' => (string)$c['registration_id'],
            'agent_id' => (string)$c['agent_id'],
            'operational_key_id' => (string)$c['operational_key_id'],
            'challenge_id' => (string)$c['challenge_id'],
            'nonce' => (string)$c['nonce'],
            'expires_at' => (string)$c['expires_at'],
            'method' => $method, 'route' => $route, 'body_sha256' => $bodyHash
        ];
    }

    private function startResponse(array $row, bool $retry): array
    {
        return [
            'registration_id' => $row['registration_id'],
            'state' => $row['state'] ?? 'pending',
            'nonce' => $row['nonce'],
            'expires_at' => $row['expires_at'],
            'community_origin' => $row['community_origin'],
            'community_instance' => $row['community_instance'],
            'profile_format' => COMMUNITY_REGISTRATION_PROFILE_FORMAT,
            'profile_sha256' => $row['profile_sha256'],
            'signature_algorithm' => COMMUNITY_SIGNATURE_ALGORITHM,
            'public_key_format' => COMMUNITY_PUBLIC_KEY_FORMAT,
            'signature_format' => COMMUNITY_SIGNATURE_FORMAT,
            'protocol_version' => COMMUNITY_REGISTRATION_PROTOCOL_VERSION,
            'statement' => community_registration_statement($row),
            'retry' => $retry
        ];
    }

    private function assertRegistrationInstance(array $attempt): void
    {
        if ((string)$attempt['community_origin'] !== (string)$this->config['community_origin']
            || (string)$attempt['community_instance'] !== (string)$this->config['community_instance']) {
            throw new CommunityHttpException(409, 'registration_instance_mismatch', 'Registration context belongs to a different configured community instance.');
        }
    }

    private function requireString(array $input, string $field, int $max): string
    {
        $v = $input[$field] ?? null;
        if (!is_string($v) || $v === '' || strlen($v) > $max) {
            throw new CommunityHttpException(400, 'invalid_request', $field . ' is invalid.');
        }
        return $v;
    }
}
