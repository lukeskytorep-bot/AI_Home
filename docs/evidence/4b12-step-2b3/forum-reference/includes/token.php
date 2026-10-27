<?php
declare(strict_types=1);

function community_generate_bearer_token(): array
{
    $secret = community_base64url_encode(random_bytes(32));
    $token = 'eqai_' . $secret;
    return [
        'token' => $token,
        'hash' => community_hash_bearer_token($token),
        'prefix' => substr($token, 0, 12),
    ];
}

function community_hash_bearer_token(string $token): string
{
    if (!str_starts_with($token, 'eqai_') || strlen($token) < 40) {
        throw new InvalidArgumentException('Invalid bearer token format.');
    }
    return hash('sha256', $token);
}
