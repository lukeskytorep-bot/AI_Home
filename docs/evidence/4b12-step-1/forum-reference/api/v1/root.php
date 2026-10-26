<?php
declare(strict_types=1);

// Method gating occurs before storage bootstrap so unsupported methods return
// a stable 405 even when the service is not yet installed.
$root = dirname(__DIR__, 2);
require_once $root . '/includes/request-context.php';
require_once $root . '/includes/errors.php';
require_once $root . '/includes/api.php';
$preflightRequestId = community_request_id();
ini_set('display_errors', '0');
error_reporting(E_ALL);
set_exception_handler(function (Throwable $e) use ($preflightRequestId): void {
    $payload = community_safe_error_payload($e, $preflightRequestId);
    community_json_response($payload['status'], $payload['body']);
});
community_require_method('GET');

$ctx = require $root . '/includes/bootstrap.php';

/** @var CommunityRepository $repository */
$repository = $ctx['repository'];
$health = $repository->healthCheck();
$status = community_health_http_status($health);

if ($status !== 200) {
    community_json_response($status, [
        'ok' => false,
        'error' => 'service_not_ready',
        'message' => 'Community service is not ready.',
        'request_id' => $ctx['request_id'],
    ]);
}

community_json_response(200, [
    'ok' => true,
    'api_version' => $ctx['config']['api_version'],
    'server_version' => $ctx['config']['server_version'],
    'request_id' => $ctx['request_id'],
    'storage' => [
        'backend' => 'sqlite3',
        'schema_version' => $health['schema_version'],
        'expected_schema_version' => $health['expected_schema_version'],
        'migration_state' => $health['migration_state'],
    ],
    'capabilities' => [
        'health' => true,
        'identity_contracts' => true,
        'registration' => true,
        'profiles' => true,
        'admin_control_plane' => true,
        'forum_core' => true,
        'forum_public_write_api' => true,
        'forum_edit_endpoint' => '/api/v1/content/{id}',
        'event_storage' => true,
        'events' => true,
        'recovery_authority' => true,
        'recovery_format' => COMMUNITY_IDENTITY_RECOVERY_FORMAT,
        'restore_quarantine_recovery' => true,
        'restore_quarantine_format' => COMMUNITY_RESTORE_QUARANTINE_FORMAT,
        'trusted_device_recovery' => true,
        'change_feed_format' => 'community-change-feed/1',
        'change_feed_cursor' => 'cf1.<feed_epoch>.<sequence>',
        'messenger' => false,
        'a2a' => false,
    ],
]);
