<?php
declare(strict_types=1);

final class OpenSSLP256SignatureProvider implements SignatureProvider
{
    private const MAX_PUBLIC_KEY_DER_BYTES = 512;
    private const MAX_SIGNATURE_DER_BYTES = 256;

    public function algorithmId(): string { return COMMUNITY_SIGNATURE_ALGORITHM; }
    public function publicKeyFormat(): string { return COMMUNITY_PUBLIC_KEY_FORMAT; }
    public function signatureFormat(): string { return COMMUNITY_SIGNATURE_FORMAT; }

    public function validatePublicKey(string $publicKeyEncoded): bool
    {
        try {
            $this->canonicalDetails($publicKeyEncoded);
            return true;
        } catch (Throwable $e) {
            return false;
        }
    }

    public function publicKeyFingerprint(string $publicKeyEncoded): string
    {
        $details = $this->canonicalDetails($publicKeyEncoded);
        $ec = $details['ec'] ?? null;
        if (!is_array($ec) || !isset($ec['x'], $ec['y']) || !is_string($ec['x']) || !is_string($ec['y'])) {
            throw new InvalidArgumentException('OpenSSL did not expose canonical EC coordinates.');
        }
        if (strlen($ec['x']) !== 32 || strlen($ec['y']) !== 32) {
            throw new InvalidArgumentException('Unexpected P-256 coordinate length.');
        }
        return hash('sha256', "P256-POINT/1\0" . $ec['x'] . $ec['y']);
    }

    public function verify(string $messageBytes, string $publicKeyEncoded, string $signatureEncoded): bool
    {
        if (!extension_loaded('openssl')) {
            throw new RuntimeException('OpenSSL extension is not available.');
        }
        try {
            $details = $this->canonicalDetails($publicKeyEncoded);
            $signatureDer = community_base64url_decode($signatureEncoded, self::MAX_SIGNATURE_DER_BYTES);
            $publicKey = openssl_pkey_get_public((string)$details['key']);
            if ($publicKey === false) {
                return false;
            }
            return openssl_verify($messageBytes, $signatureDer, $publicKey, OPENSSL_ALGO_SHA256) === 1;
        } catch (Throwable $e) {
            return false;
        }
    }

    private function canonicalDetails(string $publicKeyEncoded): array
    {
        if (!extension_loaded('openssl')) {
            throw new RuntimeException('OpenSSL extension is not available.');
        }
        $inputDer = community_base64url_decode($publicKeyEncoded, self::MAX_PUBLIC_KEY_DER_BYTES);
        $publicKey = openssl_pkey_get_public($this->pemFromSpkiDer($inputDer));
        if ($publicKey === false) {
            throw new InvalidArgumentException('Invalid public key.');
        }
        $details = openssl_pkey_get_details($publicKey);
        if (!is_array($details) || ($details['type'] ?? null) !== OPENSSL_KEYTYPE_EC) {
            throw new InvalidArgumentException('Public key is not EC.');
        }
        if ((string)($details['ec']['curve_name'] ?? '') !== 'prime256v1') {
            throw new InvalidArgumentException('Public key is not P-256.');
        }

        // Re-encoding catches data that OpenSSL tolerated but did not consume,
        // such as trailing DER bytes. Some valid SPKI encodings may differ in EC
        // point representation; role separation therefore uses x/y fingerprinting
        // rather than comparing SPKI text or bytes.
        $canonicalDer = $this->spkiDerFromPem((string)($details['key'] ?? ''));
        if (!hash_equals($canonicalDer, $inputDer)) {
            throw new InvalidArgumentException('Public key DER contains an unsupported alias or trailing data.');
        }
        if (!hash_equals(community_base64url_encode($canonicalDer), $publicKeyEncoded)) {
            throw new InvalidArgumentException('Public key base64url is not canonical.');
        }
        return $details;
    }

    private function pemFromSpkiDer(string $der): string
    {
        return "-----BEGIN PUBLIC KEY-----\n"
            . chunk_split(base64_encode($der), 64, "\n")
            . "-----END PUBLIC KEY-----\n";
    }

    private function spkiDerFromPem(string $pem): string
    {
        if (!preg_match('/-----BEGIN PUBLIC KEY-----\\s*(.*?)\\s*-----END PUBLIC KEY-----/s', $pem, $m)) {
            throw new InvalidArgumentException('Canonical SPKI PEM is unavailable.');
        }
        $body = preg_replace('/\\s+/', '', $m[1]);
        $der = is_string($body) ? base64_decode($body, true) : false;
        if (!is_string($der)) {
            throw new InvalidArgumentException('Canonical SPKI DER could not be decoded.');
        }
        return $der;
    }
}
