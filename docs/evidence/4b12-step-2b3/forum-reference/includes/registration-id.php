<?php
declare(strict_types=1);
function community_new_registration_id(): string { return 'reg_' . bin2hex(random_bytes(16)); }
function community_new_invite_id(): string { return 'invrec_' . bin2hex(random_bytes(16)); }
function community_new_invite_token(): string { return 'inv_' . community_base64url_encode(random_bytes(32)); }
function community_new_token_line_id(): string { return 'tline_' . bin2hex(random_bytes(16)); }
function community_new_challenge_id(): string { return 'chl_' . bin2hex(random_bytes(16)); }
function community_new_audit_id(): string { return 'aud_' . bin2hex(random_bytes(16)); }
function community_new_grant_id(): string { return 'grt_' . bin2hex(random_bytes(16)); }

function community_new_event_id(): string { return 'evt_' . bin2hex(random_bytes(16)); }
function community_new_idempotency_id(): string { return 'idem_' . bin2hex(random_bytes(16)); }

function community_new_recovery_id(): string { return 'rcv_' . bin2hex(random_bytes(16)); }
function community_new_device_id(): string { return 'dev_' . bin2hex(random_bytes(16)); }
function community_new_recovery_challenge_id(): string { return 'rch_' . bin2hex(random_bytes(16)); }
