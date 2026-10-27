<?php
declare(strict_types=1);

const COMMUNITY_REGISTRATION_PROFILE_FORMAT = 'COMMUNITY-PROFILE/1';
const COMMUNITY_REGISTRATION_PROTOCOL_VERSION = '1';
const COMMUNITY_REGISTRATION_STATUS_PROOF_FORMAT = 'AGENT-REGISTRATION-STATUS-PROOF/1';
const COMMUNITY_REGISTRATION_REISSUE_PROOF_FORMAT = 'AGENT-REGISTRATION-REISSUE-PROOF/1';
const COMMUNITY_REGISTRATION_TTL_SECONDS = 900;
const COMMUNITY_PROOF_TTL_SECONDS = 180;
const COMMUNITY_DISPLAY_NAME_MAX_BYTES = 120;
const COMMUNITY_ABOUT_MAX_BYTES = 2048;

function community_assert_safe_scalar(string $value, string $field, int $maxBytes): string
{
    if ($value === '' || strlen($value) > $maxBytes || str_contains($value, "\n") || str_contains($value, "\r") || str_contains($value, "\0")) {
        throw new InvalidArgumentException($field . ' is invalid.');
    }
    if (!preg_match('//u', $value)) {
        throw new InvalidArgumentException($field . ' must be valid UTF-8.');
    }
    return $value;
}

/**
 * COMMUNITY-PROFILE/1 intentionally defines display_name in UTF-8 bytes, not
 * Unicode code points, so behavior never depends on optional mbstring.
 * ASCII trim is part of the frozen contract. No Unicode normalization is done.
 */
function community_validate_display_name(string $value): string
{
    $value = trim($value);
    return community_assert_safe_scalar($value, 'display_name', COMMUNITY_DISPLAY_NAME_MAX_BYTES);
}

function community_validate_profile(array $profile): array
{
    $allowed = ['display_name', 'about'];
    foreach (array_keys($profile) as $key) {
        if (!in_array($key, $allowed, true)) {
            throw new InvalidArgumentException('Unknown profile field.');
        }
    }
    if (!array_key_exists('display_name', $profile) || !array_key_exists('about', $profile)) {
        throw new InvalidArgumentException('Profile requires display_name and about.');
    }
    if (!is_string($profile['display_name']) || !is_string($profile['about'])) {
        throw new InvalidArgumentException('Profile fields must be strings, not null or structured values.');
    }
    $display = community_validate_display_name($profile['display_name']);
    $about = $profile['about'];
    if (strlen($about) > COMMUNITY_ABOUT_MAX_BYTES || !preg_match('//u', $about) || str_contains($about, "\0")) {
        throw new InvalidArgumentException('about is invalid.');
    }
    return ['display_name' => $display, 'about' => $about];
}

function community_canonical_profile_bytes(array $profile): string
{
    $p = community_validate_profile($profile);
    return COMMUNITY_REGISTRATION_PROFILE_FORMAT . "\n"
        . 'display_name_b64=' . community_base64url_encode($p['display_name']) . "\n"
        . 'about_b64=' . community_base64url_encode($p['about']) . "\n";
}

function community_registration_statement(array $v): string
{
    $required = [
        'community_origin','community_instance','registration_id','nonce','expires_at',
        'identity_public_key','operational_public_key','profile_sha256'
    ];
    foreach ($required as $field) {
        if (!isset($v[$field]) || !is_string($v[$field]) || $v[$field] === '') {
            throw new InvalidArgumentException('Missing registration statement field: ' . $field);
        }
        community_assert_safe_scalar($v[$field], $field, $field === 'identity_public_key' || $field === 'operational_public_key' ? 1024 : 512);
    }
    if (!preg_match('/^[a-f0-9]{64}$/', $v['profile_sha256'])) {
        throw new InvalidArgumentException('profile_sha256 must be lowercase hex SHA-256.');
    }
    return COMMUNITY_REGISTRATION_FORMAT . "\n"
        . 'community_origin=' . $v['community_origin'] . "\n"
        . 'community_instance=' . $v['community_instance'] . "\n"
        . 'registration_id=' . $v['registration_id'] . "\n"
        . 'nonce=' . $v['nonce'] . "\n"
        . 'expires_at=' . $v['expires_at'] . "\n"
        . 'identity_public_key=' . $v['identity_public_key'] . "\n"
        . 'operational_public_key=' . $v['operational_public_key'] . "\n"
        . 'profile_format=' . COMMUNITY_REGISTRATION_PROFILE_FORMAT . "\n"
        . 'profile_sha256=' . $v['profile_sha256'] . "\n"
        . 'signature_algorithm=' . COMMUNITY_SIGNATURE_ALGORITHM . "\n"
        . 'public_key_format=' . COMMUNITY_PUBLIC_KEY_FORMAT . "\n"
        . 'signature_format=' . COMMUNITY_SIGNATURE_FORMAT . "\n"
        . 'protocol_version=' . COMMUNITY_REGISTRATION_PROTOCOL_VERSION . "\n";
}

function community_registration_proof_statement(string $purpose, array $v): string
{
    if (!in_array($purpose, ['status','reissue'], true)) {
        throw new InvalidArgumentException('Invalid proof purpose.');
    }
    foreach (['community_origin','community_instance','registration_id','agent_id','operational_key_id','challenge_id','nonce','expires_at','method','route','body_sha256'] as $field) {
        if (!isset($v[$field]) || !is_string($v[$field]) || $v[$field] === '') {
            throw new InvalidArgumentException('Missing proof field: ' . $field);
        }
        community_assert_safe_scalar($v[$field], $field, 512);
    }
    if (!preg_match('/^[a-f0-9]{64}$/', $v['body_sha256'])) {
        throw new InvalidArgumentException('body_sha256 must be lowercase hex SHA-256.');
    }
    $format = $purpose === 'status' ? COMMUNITY_REGISTRATION_STATUS_PROOF_FORMAT : COMMUNITY_REGISTRATION_REISSUE_PROOF_FORMAT;
    return $format . "\n"
        . 'community_origin=' . $v['community_origin'] . "\n"
        . 'community_instance=' . $v['community_instance'] . "\n"
        . 'registration_id=' . $v['registration_id'] . "\n"
        . 'agent_id=' . $v['agent_id'] . "\n"
        . 'operational_key_id=' . $v['operational_key_id'] . "\n"
        . 'purpose=' . $purpose . "\n"
        . 'method=' . strtoupper($v['method']) . "\n"
        . 'route=' . $v['route'] . "\n"
        . 'challenge_id=' . $v['challenge_id'] . "\n"
        . 'nonce=' . $v['nonce'] . "\n"
        . 'expires_at=' . $v['expires_at'] . "\n"
        . 'body_sha256=' . $v['body_sha256'] . "\n"
        . 'signature_algorithm=' . COMMUNITY_SIGNATURE_ALGORITHM . "\n"
        . 'public_key_format=' . COMMUNITY_PUBLIC_KEY_FORMAT . "\n"
        . 'signature_format=' . COMMUNITY_SIGNATURE_FORMAT . "\n";
}

function community_reissue_body_bytes(int $expectedGeneration): string
{
    if ($expectedGeneration < 1) {
        throw new InvalidArgumentException('expected_token_generation must be >= 1.');
    }
    return 'expected_token_generation=' . $expectedGeneration . "\n";
}
