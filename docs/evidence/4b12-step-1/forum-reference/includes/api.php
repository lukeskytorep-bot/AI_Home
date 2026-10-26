<?php
declare(strict_types=1);

function community_json_response(int $status, array $body): never
{
    http_response_code($status);
    header('Content-Type: application/json; charset=utf-8');
    header('Cache-Control: no-store');
    header('X-Content-Type-Options: nosniff');
    echo json_encode($body, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_PRETTY_PRINT);
    exit;
}

function community_require_method(string $expected): void
{
    $actual = strtoupper((string)($_SERVER['REQUEST_METHOD'] ?? 'GET'));
    if ($actual !== strtoupper($expected)) {
        header('Allow: ' . strtoupper($expected));
        throw new CommunityHttpException(405, 'method_not_allowed', 'Method not allowed.');
    }
}

function community_health_http_status(array $health): int
{
    return !empty($health['ready']) ? 200 : 503;
}

function community_read_json_body(int $maxBytes): array
{
    $length = isset($_SERVER['CONTENT_LENGTH']) ? (int)$_SERVER['CONTENT_LENGTH'] : null;
    if ($length !== null && $length > $maxBytes) {
        throw new CommunityHttpException(413, 'payload_too_large', 'Request body is too large.');
    }
    $raw = file_get_contents('php://input', false, null, 0, $maxBytes + 1);
    if ($raw === false) {
        throw new CommunityHttpException(400, 'invalid_request', 'Request body could not be read.');
    }
    if (strlen($raw) > $maxBytes) {
        throw new CommunityHttpException(413, 'payload_too_large', 'Request body is too large.');
    }
    try {
        community_json_reject_duplicate_keys($raw);
        $value = json_decode($raw, true, 32, JSON_THROW_ON_ERROR);
    } catch (CommunityHttpException $e) {
        throw $e;
    } catch (Throwable $e) {
        throw new CommunityHttpException(400, 'invalid_json', 'Request body must be valid JSON.');
    }
    if (!is_array($value) || array_is_list($value)) {
        throw new CommunityHttpException(400, 'invalid_request', 'Request body must be a JSON object.');
    }
    return $value;
}

function community_client_ip_subject(): string
{
    // Deliberately ignore X-Forwarded-For unless a trusted-proxy policy is added later.
    return hash('sha256', (string)($_SERVER['REMOTE_ADDR'] ?? 'unknown'));
}


/** Small JSON lexical parser used only to reject duplicate object property names
 * before json_decode collapses them. It does not replace json_decode for values. */
function community_json_reject_duplicate_keys(string $json, int $maxDepth = 32): void
{
    $i = 0; $n = strlen($json);
    $ws = function () use (&$i, $n, $json): void {
        while ($i < $n && str_contains(" \t\r\n", $json[$i])) { $i++; }
    };
    $parseString = function () use (&$i, $n, $json): string {
        if ($i >= $n || $json[$i] !== '"') { throw new RuntimeException('json string expected'); }
        $start = $i++;
        while ($i < $n) {
            $c = $json[$i++];
            if ($c === '\\') {
                if ($i >= $n) { throw new RuntimeException('bad escape'); }
                if ($json[$i] === 'u') { $i += 5; } else { $i++; }
                continue;
            }
            if ($c === '"') {
                $raw = substr($json, $start, $i - $start);
                $v = json_decode($raw, true, 2, JSON_THROW_ON_ERROR);
                if (!is_string($v)) { throw new RuntimeException('bad string'); }
                return $v;
            }
        }
        throw new RuntimeException('unterminated string');
    };
    $parseValue = null;
    $parseValue = function (int $depth = 0) use (&$parseValue, &$parseString, &$i, $n, $json, $ws, $maxDepth): void {
        $ws();
        if ($i >= $n) { throw new RuntimeException('value expected'); }
        if ($json[$i] === '{') {
            if ($depth >= $maxDepth) { throw new CommunityHttpException(400, 'json_too_deep', 'JSON nesting is too deep.'); }
            $i++; $keys = []; $ws();
            if ($i < $n && $json[$i] === '}') { $i++; return; }
            while (true) {
                $ws(); $k = $parseString();
                if (isset($keys[$k])) { throw new CommunityHttpException(400, 'duplicate_json_key', 'Duplicate JSON object property is not allowed.'); }
                $keys[$k] = true; $ws();
                if ($i >= $n || $json[$i] !== ':') { throw new RuntimeException('colon expected'); }
                $i++; $parseValue($depth + 1); $ws();
                if ($i < $n && $json[$i] === ',') { $i++; continue; }
                if ($i < $n && $json[$i] === '}') { $i++; return; }
                throw new RuntimeException('object terminator expected');
            }
        }
        if ($json[$i] === '[') {
            if ($depth >= $maxDepth) { throw new CommunityHttpException(400, 'json_too_deep', 'JSON nesting is too deep.'); }
            $i++; $ws();
            if ($i < $n && $json[$i] === ']') { $i++; return; }
            while (true) {
                $parseValue($depth + 1); $ws();
                if ($i < $n && $json[$i] === ',') { $i++; continue; }
                if ($i < $n && $json[$i] === ']') { $i++; return; }
                throw new RuntimeException('array terminator expected');
            }
        }
        if ($json[$i] === '"') { $parseString(); return; }
        if (preg_match('/\G(?:true|false|null|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?)/A', $json, $m, 0, $i)) {
            $i += strlen($m[0]); return;
        }
        throw new RuntimeException('invalid json token');
    };
    $parseValue(0); $ws();
    if ($i !== $n) { throw new RuntimeException('trailing json'); }
}

function community_require_header(string $serverKey, int $maxBytes, string $errorCode = 'invalid_request'): string
{
    $value = $_SERVER[$serverKey] ?? null;
    if (!is_string($value) || $value === '' || strlen($value) > $maxBytes || str_contains($value, "\r") || str_contains($value, "\n")) {
        throw new CommunityHttpException(400, $errorCode, 'Required request header is missing or invalid.');
    }
    return $value;
}

/**
 * Apply three independent registration limits: whole community instance, source
 * IP, and registration/client context independent of IP.
 */
function community_registration_rate_limit(
    CommunityRepository $repository,
    string $operation,
    string $communityInstance,
    string $context,
    int $ipLimit,
    int $contextLimit,
    int $instanceLimit,
    int $windowSeconds,
    ?string $ipSubject = null,
    ?int $nowEpoch = null
): bool {
    $ipSubject ??= community_client_ip_subject();
    $nowEpoch ??= time();
    $instanceSubject = hash('sha256', 'instance|' . $communityInstance);
    $contextSubject = hash('sha256', 'context|' . $communityInstance . '|' . $context);
    return community_transaction($repository, function () use ($repository, $operation, $ipSubject, $instanceSubject, $contextSubject, $ipLimit, $contextLimit, $instanceLimit, $windowSeconds, $nowEpoch): bool {
        $a = $repository->hitRateLimit($operation . '.instance', $instanceSubject, $instanceLimit, $windowSeconds, $nowEpoch);
        $b = $repository->hitRateLimit($operation . '.ip', $ipSubject, $ipLimit, $windowSeconds, $nowEpoch);
        $c = $repository->hitRateLimit($operation . '.context', $contextSubject, $contextLimit, $windowSeconds, $nowEpoch);
        return $a && $b && $c;
    });
}


function community_agent_rate_limit(
    CommunityRepository $repository,
    string $operation,
    string $communityInstance,
    string $context,
    int $ipLimit,
    int $contextLimit,
    int $instanceLimit,
    int $windowSeconds = 60
): void {
    if(!community_registration_rate_limit($repository,$operation,$communityInstance,$context,$ipLimit,$contextLimit,$instanceLimit,$windowSeconds)) {
        throw new CommunityHttpException(429,'rate_limited','Too many requests.');
    }
}


function community_require_json_content_type(): void
{
    $value=(string)($_SERVER['CONTENT_TYPE']??'');
    if($value===''||!preg_match('#^application/json(?:\s*;\s*charset=[A-Za-z0-9._-]+)?$#i',trim($value))) {
        throw new CommunityHttpException(415,'unsupported_media_type','Content-Type must be application/json.');
    }
}

function community_parse_page_limit(int $defaultLimit=25,int $maxLimit=50): array
{
    $pageRaw=$_GET['page']??'1'; $limitRaw=$_GET['limit']??(string)$defaultLimit;
    if(!is_string($pageRaw)||!ctype_digit($pageRaw)||!is_string($limitRaw)||!ctype_digit($limitRaw)) throw new CommunityHttpException(400,'invalid_request','Pagination parameters are invalid.');
    $page=(int)$pageRaw; $limit=(int)$limitRaw;
    if($page<1||$page>10000||$limit<1||$limit>$maxLimit) throw new CommunityHttpException(400,'invalid_request','Pagination parameters are outside allowed bounds.');
    return [$page,$limit,($page-1)*$limit];
}
