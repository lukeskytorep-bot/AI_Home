<?php
declare(strict_types=1);

final class NativeSQLite3Repository implements CommunityRepository
{
    private ?SQLite3 $db = null;
    private bool $inTransaction = false;
    private bool $operatorAttached = false;

    public function __construct(
        private readonly string $databasePath,
        private readonly string $migrationsDir,
        private readonly int $busyTimeoutMs = 5000,
        private readonly ?string $operatorStatePath = null
    ) {}

    public function openExisting(): void
    {
        $this->assertSQLiteAvailable();
        if (!is_file($this->databasePath)) {
            throw new RuntimeException('Community database does not exist. Run the maintenance installer.');
        }
        $this->open(SQLITE3_OPEN_READWRITE);
    }

    public function openForMaintenance(): void
    {
        $this->assertSQLiteAvailable();
        $directory = dirname($this->databasePath);
        if (!is_dir($directory) && !@mkdir($directory, 0700, true) && !is_dir($directory)) {
            throw new RuntimeException('Community data directory could not be created.');
        }
        if (!is_writable($directory)) {
            throw new RuntimeException('Community data directory is not writable.');
        }
        $this->open(SQLITE3_OPEN_READWRITE | SQLITE3_OPEN_CREATE);
    }

    private function open(int $flags): void
    {
        if ($this->db instanceof SQLite3) {
            return;
        }
        $this->db = new SQLite3($this->databasePath, $flags);
        $this->db->enableExceptions(true);
        $this->db->busyTimeout($this->busyTimeoutMs);
        $this->db->exec('PRAGMA foreign_keys = ON');
        $this->db->exec('PRAGMA synchronous = FULL');
        if($this->operatorStatePath!==null) {
            $this->attachOperatorState();
        }
    }

    public function migrate(): array
    {
        $db = $this->requireDb();
        $migrations = $this->migrationFiles();
        $appliedNow = [];

        foreach ($migrations as $migration) {
            // BEGIN IMMEDIATE is intentionally acquired before checking whether the
            // migration is already applied. Competing migration runners serialize
            // here, so check + apply + ledger write are one critical section.
            $this->beginTransaction();
            try {
                $this->ensureMigrationLedger();
                $row = $this->migrationLedgerRow($migration['version']);
                if (is_array($row)) {
                    if (!hash_equals((string)$row['checksum'], $migration['checksum'])) {
                        throw new RuntimeException(
                            'Applied migration checksum mismatch for version ' . $migration['version'] . '.'
                        );
                    }
                    $this->commit();
                    continue;
                }

                if (!$db->exec($migration['sql'])) {
                    throw new RuntimeException('Migration execution failed.');
                }

                $insert = $db->prepare(
                    'INSERT INTO schema_migrations(version, name, checksum, applied_at) ' .
                    'VALUES (:version, :name, :checksum, :applied_at)'
                );
                $insert->bindValue(':version', $migration['version'], SQLITE3_INTEGER);
                $insert->bindValue(':name', $migration['name'], SQLITE3_TEXT);
                $insert->bindValue(':checksum', $migration['checksum'], SQLITE3_TEXT);
                $insert->bindValue(':applied_at', gmdate('c'), SQLITE3_TEXT);
                $result = $insert->execute();
                if (!$result) {
                    throw new RuntimeException('Migration ledger write failed.');
                }

                $this->commit();
                $appliedNow[] = $migration['version'];
            } catch (Throwable $e) {
                $this->rollback();
                throw $e;
            }
        }

        return $appliedNow;
    }

    public function getSchemaVersion(): int
    {
        $db = $this->requireDb();
        if (!$this->tableExists('schema_migrations')) {
            return 0;
        }
        return (int)$db->querySingle('SELECT COALESCE(MAX(version), 0) FROM schema_migrations');
    }

    public function getExpectedSchemaVersion(): int
    {
        $files = $this->migrationFiles();
        if ($files === []) {
            return 0;
        }
        return max(array_column($files, 'version'));
    }

    public function readinessCheck(): array
    {
        $db = $this->requireDb();
        $open = ((int)$db->querySingle('SELECT 1') === 1);
        $expected = $this->getExpectedSchemaVersion();
        $current = $this->getSchemaVersion();
        $migrationState = 'current';

        if (!$this->tableExists('schema_migrations')) {
            $migrationState = 'missing_ledger';
        } elseif ($current !== $expected) {
            $migrationState = 'outdated';
        } else {
            foreach ($this->migrationFiles() as $migration) {
                $row = $this->migrationLedgerRow($migration['version']);
                if (!is_array($row)) {
                    $migrationState = 'outdated';
                    break;
                }
                if (!hash_equals((string)$row['checksum'], $migration['checksum'])) {
                    $migrationState = 'checksum_mismatch';
                    break;
                }
            }
        }

        return [
            'ready' => $open && $migrationState === 'current',
            'open' => $open,
            'schema_version' => $current,
            'expected_schema_version' => $expected,
            'migration_state' => $migrationState,
            'busy_timeout_ms' => (int)$db->querySingle('PRAGMA busy_timeout'),
        ];
    }

    public function beginTransaction(): void
    {
        if ($this->inTransaction) {
            throw new RuntimeException('Nested transactions are not supported by the foundation repository.');
        }
        $attempts = 0;
        while (true) {
            $attempts++;
            try {
                $this->requireDb()->exec('BEGIN IMMEDIATE');
                $this->inTransaction = true;
                return;
            } catch (Throwable $e) {
                $message = strtolower($e->getMessage());
                $busy = (int)$e->getCode() === 5 || (int)$e->getCode() === 6 || str_contains($message, 'database is locked') || str_contains($message, 'database is busy');
                if (!$busy || $attempts >= 2) {
                    if ($busy) { throw new CommunityStorageBusyException('Community storage is temporarily busy.', 0, $e); }
                    throw $e;
                }
                usleep(50000);
            }
        }
    }

    public function beginReadTransaction(): void
    {
        if ($this->inTransaction) {
            throw new RuntimeException('Nested transactions are not supported by the foundation repository.');
        }
        try {
            // Deferred BEGIN establishes a short read snapshot on the first read and
            // does not acquire the writer reservation used by BEGIN IMMEDIATE.
            $this->requireDb()->exec('BEGIN');
            $this->inTransaction = true;
        } catch (Throwable $e) {
            $message = strtolower($e->getMessage());
            $busy = (int)$e->getCode() === 5 || (int)$e->getCode() === 6 || str_contains($message, 'database is locked') || str_contains($message, 'database is busy');
            if ($busy) { throw new CommunityStorageBusyException('Community storage is temporarily busy.', 0, $e); }
            throw $e;
        }
    }

    public function isInTransaction(): bool
    {
        return $this->inTransaction;
    }

    public function commit(): void
    {
        if (!$this->inTransaction) {
            throw new RuntimeException('No active transaction to commit.');
        }
        try {
            $this->requireDb()->exec('COMMIT');
            $this->inTransaction = false;
        } catch (Throwable $e) {
            $message=strtolower($e->getMessage());
            $busy=(int)$e->getCode()===5||(int)$e->getCode()===6||str_contains($message,'database is locked')||str_contains($message,'database is busy');
            if($busy) throw new CommunityStorageBusyException('Community storage is temporarily busy.',0,$e);
            throw $e;
        }
    }

    public function rollback(): void
    {
        if (!$this->inTransaction) {
            return;
        }
        $this->requireDb()->exec('ROLLBACK');
        $this->inTransaction = false;
    }

    public function healthCheck(): array
    {
        // Lightweight readiness only. Full PRAGMA integrity_check is deliberately
        // excluded from ordinary public health requests.
        return $this->readinessCheck();
    }

    public function integrityCheck(): string
    {
        return (string)$this->requireDb()->querySingle('PRAGMA integrity_check');
    }

    public function foreignKeyCheck(): array
    {
        $result = $this->requireDb()->query('PRAGMA foreign_key_check');
        $rows = [];
        while ($result && ($row = $result->fetchArray(SQLITE3_ASSOC))) {
            $rows[] = $row;
        }
        return $rows;
    }

    public function close(): void
    {
        if ($this->inTransaction) {
            $this->rollback();
        }
        if ($this->db instanceof SQLite3) {
            $this->db->close();
        }
        $this->db = null;
        $this->operatorAttached = false;
    }


    private function attachOperatorState(): void
    {
        if($this->operatorStatePath===null)return;
        if(!is_file($this->operatorStatePath))throw new CommunityHttpException(503,'operator_state_unavailable','Operator security state is unavailable.');
        $mainDir=realpath(dirname($this->databasePath));$opDir=realpath(dirname($this->operatorStatePath));
        if($mainDir===false||$opDir===false||$mainDir!==$opDir)throw new CommunityHttpException(503,'operator_state_filesystem_mismatch','Community and operator databases must share the same protected filesystem directory.');
        $path=SQLite3::escapeString($this->operatorStatePath);
        $this->requireDb()->exec("ATTACH DATABASE '".$path."' AS operator");
        $this->operatorAttached=true;
        $mainMode=strtolower((string)$this->requireDb()->querySingle('PRAGMA main.journal_mode'));
        $opMode=strtolower((string)$this->requireDb()->querySingle('PRAGMA operator.journal_mode'));
        foreach([$mainMode,$opMode] as $mode) {
            if(in_array($mode,['wal','off','memory'],true))throw new CommunityHttpException(503,'operator_atomicity_unsafe','Multi-database security transactions require rollback-journal mode.');
        }
        $this->requireDb()->exec('PRAGMA main.synchronous=FULL');
        $this->requireDb()->exec('PRAGMA operator.synchronous=FULL');
        $version=(int)$this->requireDb()->querySingle('SELECT COALESCE(MAX(version),0) FROM operator.operator_schema_migrations');
        if($version!==COMMUNITY_OPERATOR_SCHEMA_VERSION)throw new CommunityHttpException(503,'operator_state_schema_mismatch','Operator security state schema is not current.');
    }

    public function operatorStateAttached(): bool { return $this->operatorAttached; }

    public function getOperatorAgentSecurity(string $agentId): ?array
    {
        if(!$this->operatorAttached)return null;
        $s=$this->requireDb()->prepare('SELECT * FROM operator.operator_agent_security WHERE agent_id=:a');$s->bindValue(':a',$agentId,SQLITE3_TEXT);$q=$s->execute();$row=$q?$q->fetchArray(SQLITE3_ASSOC):false;return is_array($row)?$row:null;
    }

    public function requireOperatorAgentUsable(string $agentId): void
    {
        if(!$this->operatorAttached)return;
        $row=$this->getOperatorAgentSecurity($agentId);
        if(!is_array($row))throw new CommunityHttpException(503,'security_state_unavailable','Agent security barrier is unavailable.');
        if((string)$row['status']!=='active')throw new CommunityHttpException(403,'agent_not_active','Agent is not active for this operation.');
        if((string)$row['quarantine_state']!=='none')throw new CommunityHttpException(423,'restore_quarantine','Agent access is quarantined after restore and requires identity-authorized recovery.');
    }

    public function syncOperatorAgentSecurity(string $agentId, string $now): void
    {
        if(!$this->operatorAttached)return;
        $agent=$this->getAgentIdentity($agentId);$recovery=$this->getRecoveryState($agentId);
        $identity=$this->getActiveKeyByPurpose($agentId,'identity_rotation');
        if(!is_array($agent)||!is_array($recovery)||!is_array($identity))throw new RuntimeException('Cannot synchronize incomplete operator security state.');
        $oper=$this->getKeyRecord((string)$agent['current_operational_key_id']);
        if(!is_array($oper))throw new RuntimeException('Current operational key is unavailable.');
        $line=$this->getTokenLineByAgent($agentId);$tokenGen=max(1,(int)($line['current_generation']??1));
        $existing=$this->getOperatorAgentSecurity($agentId);
        if(is_array($existing)) {
            if(!hash_equals((string)$existing['trusted_identity_key_id'],(string)$identity['key_id'])||!hash_equals((string)$existing['trusted_identity_key_sha256'],hash('sha256',(string)$identity['public_key']))) {
                throw new CommunityHttpException(409,'operator_identity_conflict','Identity authority differs from the non-rollback operator state.');
            }
            $sql='UPDATE operator.operator_agent_security SET current_operational_key_id=:ok,current_operational_key_sha256=:oh,min_key_generation=MAX(min_key_generation,:kg),min_policy_generation=MAX(min_policy_generation,:pg),min_token_generation=MAX(min_token_generation,:tg),status=:status,security_version=security_version+1,updated_at=:now WHERE agent_id=:a';
        } else {
            $sql="INSERT INTO operator.operator_agent_security(agent_id,security_version,trusted_identity_key_id,trusted_identity_key_sha256,current_operational_key_id,current_operational_key_sha256,min_key_generation,min_policy_generation,min_token_generation,status,quarantine_state,updated_at) VALUES (:a,1,:ik,:ih,:ok,:oh,:kg,:pg,:tg,:status,'none',:now)";
        }
        $s=$this->requireDb()->prepare($sql);$s->bindValue(':a',$agentId,SQLITE3_TEXT);
        if(!is_array($existing)){$s->bindValue(':ik',(string)$identity['key_id'],SQLITE3_TEXT);$s->bindValue(':ih',hash('sha256',(string)$identity['public_key']),SQLITE3_TEXT);}
        $s->bindValue(':ok',(string)$oper['key_id'],SQLITE3_TEXT);$s->bindValue(':oh',hash('sha256',(string)$oper['public_key']),SQLITE3_TEXT);$s->bindValue(':kg',(int)$recovery['key_generation'],SQLITE3_INTEGER);$s->bindValue(':pg',(int)$recovery['policy_generation'],SQLITE3_INTEGER);$s->bindValue(':tg',$tokenGen,SQLITE3_INTEGER);$s->bindValue(':status',(string)$agent['status'],SQLITE3_TEXT);$s->bindValue(':now',$now,SQLITE3_TEXT);$s->execute();
    }

    public function setRecoveryGenerations(string $agentId, int $keyGeneration, int $policyGeneration, string $now): void
    {
        if($keyGeneration<1||$policyGeneration<1)throw new InvalidArgumentException('Recovery generations must be positive.');
        $s=$this->requireDb()->prepare('UPDATE agent_recovery_state SET key_generation=:kg,policy_generation=:pg,updated_at=:now WHERE agent_id=:a');$s->bindValue(':kg',$keyGeneration,SQLITE3_INTEGER);$s->bindValue(':pg',$policyGeneration,SQLITE3_INTEGER);$s->bindValue(':now',$now,SQLITE3_TEXT);$s->bindValue(':a',$agentId,SQLITE3_TEXT);$s->execute();if($this->requireDb()->changes()!==1)throw new RuntimeException('Recovery state update failed.');
    }

    public function setTokenLineGenerationFloor(string $agentId, int $minimumGeneration, string $now): void
    {
        if($minimumGeneration<1)return;
        $s=$this->requireDb()->prepare('UPDATE registration_token_lines SET current_generation=CASE WHEN current_generation<:g THEN :g ELSE current_generation END,updated_at=:now WHERE agent_id=:a');$s->bindValue(':g',$minimumGeneration,SQLITE3_INTEGER);$s->bindValue(':now',$now,SQLITE3_TEXT);$s->bindValue(':a',$agentId,SQLITE3_TEXT);$s->execute();
    }

    public function createRestoreQuarantineChallenge(array $row): void
    {
        if(!$this->operatorAttached)throw new CommunityHttpException(503,'operator_state_unavailable','Operator state is required.');
        $s=$this->requireDb()->prepare("INSERT INTO operator.operator_quarantine_challenges(challenge_id,agent_id,state,trusted_identity_key_id,expected_security_version,expected_min_key_generation,expected_min_policy_generation,expected_min_token_generation,new_operational_key_id,new_operational_public_key,nonce,expires_at,statement_canonical,statement_sha256,created_at,updated_at) VALUES (:id,:a,'pending',:ik,:sv,:kg,:pg,:tg,:nk,:pub,:nonce,:exp,:stmt,:sha,:now,:now)");
        foreach(['challenge_id'=>'id','agent_id'=>'a','trusted_identity_key_id'=>'ik','new_operational_key_id'=>'nk','new_operational_public_key'=>'pub','nonce'=>'nonce','expires_at'=>'exp','statement_canonical'=>'stmt','statement_sha256'=>'sha','created_at'=>'now'] as $k=>$p)$s->bindValue(':'.$p,(string)$row[$k],SQLITE3_TEXT);
        foreach(['expected_security_version'=>'sv','expected_min_key_generation'=>'kg','expected_min_policy_generation'=>'pg','expected_min_token_generation'=>'tg'] as $k=>$p)$s->bindValue(':'.$p,(int)$row[$k],SQLITE3_INTEGER);$s->execute();
    }

    public function getRestoreQuarantineChallenge(string $challengeId): ?array
    {
        if(!$this->operatorAttached)return null;$s=$this->requireDb()->prepare('SELECT * FROM operator.operator_quarantine_challenges WHERE challenge_id=:i');$s->bindValue(':i',$challengeId,SQLITE3_TEXT);$q=$s->execute();$row=$q?$q->fetchArray(SQLITE3_ASSOC):false;return is_array($row)?$row:null;
    }

    public function markRestoreQuarantineChallengeCommitted(string $challengeId,string $fingerprint,array $result,string $now): void
    {
        $s=$this->requireDb()->prepare("UPDATE operator.operator_quarantine_challenges SET state='committed',result_fingerprint=:f,result_json=:j,committed_at=:n,updated_at=:n WHERE challenge_id=:i AND state='pending'");$s->bindValue(':f',$fingerprint,SQLITE3_TEXT);$s->bindValue(':j',json_encode($result,JSON_UNESCAPED_SLASHES|JSON_UNESCAPED_UNICODE),SQLITE3_TEXT);$s->bindValue(':n',$now,SQLITE3_TEXT);$s->bindValue(':i',$challengeId,SQLITE3_TEXT);$s->execute();if($this->requireDb()->changes()!==1)throw new RuntimeException('Quarantine challenge changed concurrently.');
    }

    public function clearOperatorQuarantine(string $agentId,string $newOperationalKeyId,string $newOperationalPublicKey,int $newKeyGeneration,int $newPolicyGeneration,int $newTokenGeneration,string $now): void
    {
        if(!$this->operatorAttached)throw new CommunityHttpException(503,'operator_state_unavailable','Operator state is required.');
        $s=$this->requireDb()->prepare("UPDATE operator.operator_agent_security SET current_operational_key_id=:k,current_operational_key_sha256=:h,min_key_generation=MAX(min_key_generation,:kg),min_policy_generation=MAX(min_policy_generation,:pg),min_token_generation=MAX(min_token_generation,:tg),quarantine_state='none',quarantine_reason=NULL,quarantine_operation_id=NULL,quarantined_at=NULL,security_version=security_version+1,updated_at=:n WHERE agent_id=:a AND status='active'");
        $s->bindValue(':k',$newOperationalKeyId,SQLITE3_TEXT);$s->bindValue(':h',hash('sha256',$newOperationalPublicKey),SQLITE3_TEXT);$s->bindValue(':kg',$newKeyGeneration,SQLITE3_INTEGER);$s->bindValue(':pg',$newPolicyGeneration,SQLITE3_INTEGER);$s->bindValue(':tg',$newTokenGeneration,SQLITE3_INTEGER);$s->bindValue(':n',$now,SQLITE3_TEXT);$s->bindValue(':a',$agentId,SQLITE3_TEXT);$s->execute();if($this->requireDb()->changes()!==1)throw new CommunityHttpException(409,'quarantine_state_conflict','Operator quarantine state changed.');
    }

    public function setCommunityRuntimeIdentity(string $communityInstance,string $sourceManifestSha256,string $now): void
    {
        if(!preg_match('/^[A-Za-z0-9._:-]{3,128}$/D',$communityInstance)||!preg_match('/^[a-f0-9]{64}$/D',$sourceManifestSha256))throw new InvalidArgumentException('Runtime identity metadata is invalid.');
        $q=$this->requireDb()->prepare("UPDATE community_runtime_identity SET community_instance=:i,source_manifest_sha256=:s,operator_state_required=1,backup_format='community-backup/1',updated_at=:n WHERE singleton_id=1");
        $q->bindValue(':i',$communityInstance,SQLITE3_TEXT);$q->bindValue(':s',$sourceManifestSha256,SQLITE3_TEXT);$q->bindValue(':n',$now,SQLITE3_TEXT);$q->execute();if($this->requireDb()->changes()!==1)throw new RuntimeException('Community runtime identity row is unavailable.');
    }

    public function operatorHasSeenKeyFingerprint(string $agentId,string $publicKeySha256): bool
    {
        if(!$this->operatorAttached)return false;
        $q=$this->requireDb()->prepare('SELECT 1 FROM operator.operator_seen_keys WHERE agent_id=:a AND public_key_sha256=:h LIMIT 1');$q->bindValue(':a',$agentId,SQLITE3_TEXT);$q->bindValue(':h',$publicKeySha256,SQLITE3_TEXT);$r=$q->execute();return (bool)($r&&$r->fetchArray(SQLITE3_NUM));
    }

    public function getOperatorAdminSessionEpoch(): int
    {
        if(!$this->operatorAttached)return 1;$v=$this->requireDb()->querySingle("SELECT value FROM operator.operator_meta WHERE key='admin_session_epoch'");return max(1,(int)$v);
    }

    public function createAgentIdentity(string $agentId, string $displayName, string $currentOperationalKeyId, string $createdAt): void
    {
        $stmt = $this->requireDb()->prepare(
            'INSERT INTO agents(agent_id, schema_version, display_name, status, current_operational_key_id, created_at, updated_at) ' .
            'VALUES (:agent_id, :schema_version, :display_name, :status, :key_id, :created_at, :updated_at)'
        );
        $stmt->bindValue(':agent_id', $agentId, SQLITE3_TEXT);
        $stmt->bindValue(':schema_version', COMMUNITY_AGENT_SCHEMA, SQLITE3_TEXT);
        $stmt->bindValue(':display_name', $displayName, SQLITE3_TEXT);
        $stmt->bindValue(':status', 'active', SQLITE3_TEXT);
        $stmt->bindValue(':key_id', $currentOperationalKeyId, SQLITE3_TEXT);
        $stmt->bindValue(':created_at', $createdAt, SQLITE3_TEXT);
        $stmt->bindValue(':updated_at', $createdAt, SQLITE3_TEXT);
        $stmt->execute();
    }

    public function getAgentIdentity(string $agentId): ?array
    {
        $stmt = $this->requireDb()->prepare('SELECT * FROM agents WHERE agent_id = :agent_id LIMIT 1');
        $stmt->bindValue(':agent_id', $agentId, SQLITE3_TEXT);
        $result = $stmt->execute();
        $row = $result ? $result->fetchArray(SQLITE3_ASSOC) : false;
        return is_array($row) ? $row : null;
    }

    public function addKeyHistoryRecord(array $record): void
    {
        $stmt = $this->requireDb()->prepare(
            'INSERT INTO key_history(key_id, schema_version, agent_id, purpose, algorithm_id, public_key_format, public_key, valid_from, valid_until, status, status_reason, created_at) ' .
            'VALUES (:key_id, :schema_version, :agent_id, :purpose, :algorithm_id, :public_key_format, :public_key, :valid_from, :valid_until, :status, :status_reason, :created_at)'
        );
        $stmt->bindValue(':key_id', (string)$record['key_id'], SQLITE3_TEXT);
        $stmt->bindValue(':schema_version', COMMUNITY_KEY_HISTORY_SCHEMA, SQLITE3_TEXT);
        $stmt->bindValue(':agent_id', (string)$record['agent_id'], SQLITE3_TEXT);
        $stmt->bindValue(':purpose', (string)$record['purpose'], SQLITE3_TEXT);
        $stmt->bindValue(':algorithm_id', (string)$record['algorithm_id'], SQLITE3_TEXT);
        $stmt->bindValue(':public_key_format', (string)$record['public_key_format'], SQLITE3_TEXT);
        $stmt->bindValue(':public_key', (string)$record['public_key'], SQLITE3_TEXT);
        $stmt->bindValue(':valid_from', (string)$record['valid_from'], SQLITE3_TEXT);
        $stmt->bindValue(':valid_until', $record['valid_until'] ?? null, ($record['valid_until'] ?? null) === null ? SQLITE3_NULL : SQLITE3_TEXT);
        $stmt->bindValue(':status', (string)$record['status'], SQLITE3_TEXT);
        $stmt->bindValue(':status_reason', $record['status_reason'] ?? null, ($record['status_reason'] ?? null) === null ? SQLITE3_NULL : SQLITE3_TEXT);
        $stmt->bindValue(':created_at', (string)$record['valid_from'], SQLITE3_TEXT);
        $stmt->execute();
        if($this->operatorAttached){
            $x=$this->requireDb()->prepare('INSERT INTO operator.operator_seen_keys(key_id,agent_id,purpose,public_key_sha256,first_seen_at,last_state) VALUES (:k,:a,:p,:h,:t,:s) ON CONFLICT(key_id) DO UPDATE SET last_state=excluded.last_state');
            $x->bindValue(':k',(string)$record['key_id'],SQLITE3_TEXT);$x->bindValue(':a',(string)$record['agent_id'],SQLITE3_TEXT);$x->bindValue(':p',(string)$record['purpose'],SQLITE3_TEXT);$x->bindValue(':h',hash('sha256',(string)$record['public_key']),SQLITE3_TEXT);$x->bindValue(':t',(string)$record['valid_from'],SQLITE3_TEXT);$x->bindValue(':s',(string)$record['status'],SQLITE3_TEXT);$x->execute();
        }
    }

    public function getKeyHistory(string $agentId): array
    {
        $stmt = $this->requireDb()->prepare(
            'SELECT * FROM key_history WHERE agent_id = :agent_id ORDER BY valid_from ASC, key_id ASC'
        );
        $stmt->bindValue(':agent_id', $agentId, SQLITE3_TEXT);
        $result = $stmt->execute();
        $rows = [];
        while ($result && ($row = $result->fetchArray(SQLITE3_ASSOC))) {
            $rows[] = $row;
        }
        return $rows;
    }

    public function getKeyRecord(string $keyId): ?array
    {
        $stmt = $this->requireDb()->prepare('SELECT * FROM key_history WHERE key_id = :key_id LIMIT 1');
        $stmt->bindValue(':key_id', $keyId, SQLITE3_TEXT);
        $result = $stmt->execute();
        $row = $result ? $result->fetchArray(SQLITE3_ASSOC) : false;
        return is_array($row) ? $row : null;
    }

    public function setCurrentOperationalKey(string $agentId, string $keyId, string $updatedAt): void
    {
        $key = $this->getKeyRecord($keyId);
        if (!is_array($key) || $key['agent_id'] !== $agentId || $key['purpose'] !== 'operational') {
            throw new InvalidArgumentException('Current operational key must belong to the agent and have operational purpose.');
        }
        if ($key['status'] !== 'active') {
            throw new InvalidArgumentException('Current operational key must be active.');
        }
        $stmt = $this->requireDb()->prepare(
            'UPDATE agents SET current_operational_key_id = :key_id, updated_at = :updated_at WHERE agent_id = :agent_id'
        );
        $stmt->bindValue(':key_id', $keyId, SQLITE3_TEXT);
        $stmt->bindValue(':updated_at', $updatedAt, SQLITE3_TEXT);
        $stmt->bindValue(':agent_id', $agentId, SQLITE3_TEXT);
        $stmt->execute();
        if ($this->requireDb()->changes() !== 1) {
            throw new RuntimeException('Agent record was not found.');
        }
    }

    public function setKeyStatus(string $keyId, string $status, ?string $reason, ?string $validUntil): void
    {
        if (!in_array($status, ['retired', 'revoked'], true)) {
            throw new InvalidArgumentException('Keys may only transition away from active state through this operation.');
        }
        if ($validUntil === null || trim($validUntil) === '') {
            throw new InvalidArgumentException('Key retirement/revocation requires a validity end timestamp.');
        }

        $current = $this->getKeyRecord($keyId);
        if (!is_array($current)) {
            throw new RuntimeException('Key record was not found.');
        }
        $from = (string)$current['status'];
        $allowed = ($from === 'active' && in_array($status, ['retired', 'revoked'], true))
            || ($from === 'retired' && $status === 'revoked');
        if (!$allowed) {
            throw new RuntimeException('Illegal key status transition.');
        }

        // A later revocation must never extend a key's historical validity window
        // beyond an earlier retirement time.
        $effectiveValidUntil = $validUntil;
        if ($from === 'retired' && $current['valid_until'] !== null) {
            $existingUntil = strtotime((string)$current['valid_until']);
            $requestedUntil = strtotime($validUntil);
            if ($existingUntil !== false && $requestedUntil !== false && $existingUntil < $requestedUntil) {
                $effectiveValidUntil = (string)$current['valid_until'];
            }
        }

        $stmt = $this->requireDb()->prepare(
            'UPDATE key_history SET status = :status, status_reason = :reason, valid_until = :valid_until WHERE key_id = :key_id'
        );
        $stmt->bindValue(':status', $status, SQLITE3_TEXT);
        $stmt->bindValue(':reason', $reason, $reason === null ? SQLITE3_NULL : SQLITE3_TEXT);
        $stmt->bindValue(':valid_until', $effectiveValidUntil, SQLITE3_TEXT);
        $stmt->bindValue(':key_id', $keyId, SQLITE3_TEXT);
        $stmt->execute();
        if ($this->requireDb()->changes() !== 1) {
            throw new RuntimeException('Key status update failed.');
        }
        if($this->operatorAttached){
            $k=$this->getKeyRecord($keyId);
            if(is_array($k)){
                $seen=$this->requireDb()->prepare('UPDATE operator.operator_seen_keys SET last_state=:s WHERE key_id=:k');$seen->bindValue(':s',$status,SQLITE3_TEXT);$seen->bindValue(':k',$keyId,SQLITE3_TEXT);$seen->execute();
                if($status==='revoked'){$x=$this->requireDb()->prepare("INSERT INTO operator.operator_revoked_keys(key_id,agent_id,reason,revoked_at) VALUES (:k,:a,:r,:n) ON CONFLICT(key_id) DO UPDATE SET reason=excluded.reason,revoked_at=excluded.revoked_at");$x->bindValue(':k',$keyId,SQLITE3_TEXT);$x->bindValue(':a',(string)$k['agent_id'],SQLITE3_TEXT);$x->bindValue(':r',$reason??'revoked',SQLITE3_TEXT);$x->bindValue(':n',$validUntil??gmdate('c'),SQLITE3_TEXT);$x->execute();}
            }
        }
    }

    public function storeApiTokenHash(string $tokenId, string $agentId, string $tokenHash, string $tokenPrefix, string $createdAt): void
    {
        $stmt = $this->requireDb()->prepare(
            'INSERT INTO api_tokens(token_id, schema_version, agent_id, token_hash, token_prefix, created_at) ' .
            'VALUES (:token_id, :schema_version, :agent_id, :token_hash, :token_prefix, :created_at)'
        );
        $stmt->bindValue(':token_id', $tokenId, SQLITE3_TEXT);
        $stmt->bindValue(':schema_version', COMMUNITY_API_TOKEN_SCHEMA, SQLITE3_TEXT);
        $stmt->bindValue(':agent_id', $agentId, SQLITE3_TEXT);
        $stmt->bindValue(':token_hash', $tokenHash, SQLITE3_TEXT);
        $stmt->bindValue(':token_prefix', $tokenPrefix, SQLITE3_TEXT);
        $stmt->bindValue(':created_at', $createdAt, SQLITE3_TEXT);
        $stmt->execute();
    }

    public function findApiTokenByHash(string $tokenHash): ?array
    {
        $stmt = $this->requireDb()->prepare(
            'SELECT * FROM api_tokens WHERE token_hash = :token_hash AND revoked_at IS NULL LIMIT 1'
        );
        $stmt->bindValue(':token_hash', $tokenHash, SQLITE3_TEXT);
        $result = $stmt->execute();
        $row = $result ? $result->fetchArray(SQLITE3_ASSOC) : false;
        return is_array($row) ? $row : null;
    }

    public function createRegistrationInvite(string $inviteId, string $inviteHash, string $createdAt): void
    {
        $stmt = $this->requireDb()->prepare(
            'INSERT INTO registration_invites(invite_id, invite_hash, state, created_at) VALUES (:id,:hash,\'available\',:created)'
        );
        $stmt->bindValue(':id', $inviteId, SQLITE3_TEXT);
        $stmt->bindValue(':hash', $inviteHash, SQLITE3_TEXT);
        $stmt->bindValue(':created', $createdAt, SQLITE3_TEXT);
        $stmt->execute();
    }

    public function getInviteByHash(string $inviteHash): ?array
    {
        $stmt = $this->requireDb()->prepare('SELECT * FROM registration_invites WHERE invite_hash=:hash LIMIT 1');
        $stmt->bindValue(':hash', $inviteHash, SQLITE3_TEXT);
        $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false;
        return is_array($row)?$row:null;
    }

    public function getInviteById(string $inviteId): ?array
    {
        $stmt = $this->requireDb()->prepare('SELECT * FROM registration_invites WHERE invite_id=:id LIMIT 1');
        $stmt->bindValue(':id', $inviteId, SQLITE3_TEXT);
        $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false;
        return is_array($row)?$row:null;
    }

    public function reserveInvite(string $inviteId, string $registrationId, string $reservedUntil): bool
    {
        $stmt=$this->requireDb()->prepare(
            "UPDATE registration_invites SET state='reserved', reserved_registration_id=:reg, reserved_until=:until " .
            "WHERE invite_id=:id AND state='available'"
        );
        $stmt->bindValue(':reg',$registrationId,SQLITE3_TEXT);
        $stmt->bindValue(':until',$reservedUntil,SQLITE3_TEXT);
        $stmt->bindValue(':id',$inviteId,SQLITE3_TEXT);
        $stmt->execute();
        return $this->requireDb()->changes()===1;
    }

    public function releaseExpiredInviteReservation(string $inviteId, string $registrationId): void
    {
        $stmt=$this->requireDb()->prepare(
            "UPDATE registration_invites SET state='available', reserved_registration_id=NULL, reserved_until=NULL " .
            "WHERE invite_id=:id AND state='reserved' AND reserved_registration_id=:reg"
        );
        $stmt->bindValue(':id',$inviteId,SQLITE3_TEXT);
        $stmt->bindValue(':reg',$registrationId,SQLITE3_TEXT);
        $stmt->execute();
    }

    public function consumeInvite(string $inviteId, string $registrationId, string $consumedAt): bool
    {
        $stmt=$this->requireDb()->prepare(
            "UPDATE registration_invites SET state='consumed', consumed_at=:at, reserved_until=NULL " .
            "WHERE invite_id=:id AND state='reserved' AND reserved_registration_id=:reg"
        );
        $stmt->bindValue(':at',$consumedAt,SQLITE3_TEXT);
        $stmt->bindValue(':id',$inviteId,SQLITE3_TEXT);
        $stmt->bindValue(':reg',$registrationId,SQLITE3_TEXT);
        $stmt->execute();
        return $this->requireDb()->changes()===1;
    }

    public function createRegistrationAttempt(array $row): void
    {
        $stmt=$this->requireDb()->prepare(
            'INSERT INTO registration_attempts(registration_id,client_start_id,start_fingerprint,invite_id,state,nonce,expires_at,community_origin,community_instance,profile_format,profile_canonical,profile_sha256,display_name,about,identity_public_key,operational_public_key,statement_sha256,created_at,updated_at) ' .
            'VALUES (:registration_id,:client_start_id,:start_fingerprint,:invite_id,\'pending\',:nonce,:expires_at,:community_origin,:community_instance,:profile_format,:profile_canonical,:profile_sha256,:display_name,:about,:identity_public_key,:operational_public_key,:statement_sha256,:created_at,:updated_at)'
        );
        foreach (['registration_id','client_start_id','start_fingerprint','invite_id','nonce','expires_at','community_origin','community_instance','profile_format','profile_canonical','profile_sha256','display_name','about','identity_public_key','operational_public_key','statement_sha256','created_at','updated_at'] as $k) {
            $stmt->bindValue(':'.$k,(string)$row[$k],SQLITE3_TEXT);
        }
        $stmt->execute();
    }

    public function getRegistrationAttempt(string $registrationId): ?array
    {
        $stmt=$this->requireDb()->prepare('SELECT * FROM registration_attempts WHERE registration_id=:id LIMIT 1');
        $stmt->bindValue(':id',$registrationId,SQLITE3_TEXT);
        $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false;
        return is_array($row)?$row:null;
    }

    public function getRegistrationAttemptByClientStartId(string $clientStartId): ?array
    {
        $stmt=$this->requireDb()->prepare('SELECT * FROM registration_attempts WHERE client_start_id=:id LIMIT 1');
        $stmt->bindValue(':id',$clientStartId,SQLITE3_TEXT);
        $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false;
        return is_array($row)?$row:null;
    }

    public function markRegistrationExpired(string $registrationId, string $updatedAt): void
    {
        $stmt=$this->requireDb()->prepare("UPDATE registration_attempts SET state='expired', updated_at=:at WHERE registration_id=:id AND state='pending'");
        $stmt->bindValue(':at',$updatedAt,SQLITE3_TEXT); $stmt->bindValue(':id',$registrationId,SQLITE3_TEXT); $stmt->execute();
    }

    public function commitRegistrationAttempt(string $registrationId, string $agentId, string $committedAt): void
    {
        $stmt=$this->requireDb()->prepare("UPDATE registration_attempts SET state='committed', agent_id=:agent, committed_at=:at, updated_at=:at WHERE registration_id=:id AND state='pending'");
        $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT); $stmt->bindValue(':at',$committedAt,SQLITE3_TEXT); $stmt->bindValue(':id',$registrationId,SQLITE3_TEXT); $stmt->execute();
        if ($this->requireDb()->changes()!==1) throw new RuntimeException('Registration attempt could not be committed.');
    }

    public function storePublicProfile(string $agentId, string $displayName, string $about, string $profileSha256, string $now): void
    {
        $stmt=$this->requireDb()->prepare('INSERT INTO public_agent_profiles(agent_id,schema_version,display_name,about,profile_sha256,created_at,updated_at) VALUES (:agent,\'community-profile/1\',:display,:about,:hash,:now,:now)');
        $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT); $stmt->bindValue(':display',$displayName,SQLITE3_TEXT); $stmt->bindValue(':about',$about,SQLITE3_TEXT); $stmt->bindValue(':hash',$profileSha256,SQLITE3_TEXT); $stmt->bindValue(':now',$now,SQLITE3_TEXT); $stmt->execute();
    }

    public function grantCapability(string $grantId, string $agentId, string $capability, string $now): void
    {
        $stmt=$this->requireDb()->prepare("INSERT INTO capability_grants(grant_id,agent_id,capability,state,granted_at) VALUES (:id,:agent,:cap,'active',:now)");
        $stmt->bindValue(':id',$grantId,SQLITE3_TEXT); $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT); $stmt->bindValue(':cap',$capability,SQLITE3_TEXT); $stmt->bindValue(':now',$now,SQLITE3_TEXT); $stmt->execute();
    }

    public function hasActiveCapability(string $agentId, string $capability): bool
    {
        $stmt=$this->requireDb()->prepare("SELECT 1 FROM capability_grants WHERE agent_id=:agent AND capability=:cap AND state='active' LIMIT 1");
        $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT); $stmt->bindValue(':cap',$capability,SQLITE3_TEXT);
        $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_NUM):false;
        return is_array($row);
    }

    public function setCapabilityState(string $agentId, string $capability, string $state, string $now): void
    {
        if (!in_array($state,['active','revoked'],true)) throw new InvalidArgumentException('Invalid capability state.');
        $sql=$state==='active'
            ? "UPDATE capability_grants SET state='active', revoked_at=NULL WHERE agent_id=:agent AND capability=:cap"
            : "UPDATE capability_grants SET state='revoked', revoked_at=:now WHERE agent_id=:agent AND capability=:cap";
        $stmt=$this->requireDb()->prepare($sql);
        $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT); $stmt->bindValue(':cap',$capability,SQLITE3_TEXT);
        if($state==='revoked')$stmt->bindValue(':now',$now,SQLITE3_TEXT);
        $stmt->execute();
        if($this->requireDb()->changes()!==1) throw new RuntimeException('Capability grant was not found.');
        if($this->operatorAttached){
            if($state==='revoked'){$x=$this->requireDb()->prepare("INSERT INTO operator.operator_revoked_capabilities(agent_id,capability,revoked_at,reason) VALUES (:a,:c,:n,'capability_revoked') ON CONFLICT(agent_id,capability) DO UPDATE SET revoked_at=excluded.revoked_at,reason=excluded.reason");$x->bindValue(':a',$agentId,SQLITE3_TEXT);$x->bindValue(':c',$capability,SQLITE3_TEXT);$x->bindValue(':n',$now,SQLITE3_TEXT);$x->execute();}
            else{$x=$this->requireDb()->prepare('DELETE FROM operator.operator_revoked_capabilities WHERE agent_id=:a AND capability=:c');$x->bindValue(':a',$agentId,SQLITE3_TEXT);$x->bindValue(':c',$capability,SQLITE3_TEXT);$x->execute();}
        }
    }

    public function setAgentStatus(string $agentId, string $status, string $now): void
    {
        if (!in_array($status,['active','suspended','archived'],true)) throw new InvalidArgumentException('Invalid agent status.');
        $stmt=$this->requireDb()->prepare('UPDATE agents SET status=:status, updated_at=:now WHERE agent_id=:agent');
        $stmt->bindValue(':status',$status,SQLITE3_TEXT); $stmt->bindValue(':now',$now,SQLITE3_TEXT); $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT);
        $stmt->execute();
        if($this->requireDb()->changes()!==1) throw new RuntimeException('Agent record was not found.');
        if($this->operatorAttached){$x=$this->requireDb()->prepare('UPDATE operator.operator_agent_security SET status=:s,security_version=security_version+1,updated_at=:n WHERE agent_id=:a');$x->bindValue(':s',$status,SQLITE3_TEXT);$x->bindValue(':n',$now,SQLITE3_TEXT);$x->bindValue(':a',$agentId,SQLITE3_TEXT);$x->execute();if($this->requireDb()->changes()!==1)throw new CommunityHttpException(503,'security_state_unavailable','Operator agent state is unavailable.');}
    }

    public function addVerificationLabel(string $agentId, string $label, string $now): void
    {
        $stmt=$this->requireDb()->prepare('INSERT INTO agent_verifications(agent_id,label,verified_at) VALUES (:agent,:label,:now)');
        $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT); $stmt->bindValue(':label',$label,SQLITE3_TEXT); $stmt->bindValue(':now',$now,SQLITE3_TEXT); $stmt->execute();
    }

    public function storeApiTokenHashForLine(string $tokenId, string $agentId, string $tokenHash, string $tokenPrefix, string $tokenLineId, int $generation, string $createdAt): void
    {
        $stmt=$this->requireDb()->prepare('INSERT INTO api_tokens(token_id,schema_version,agent_id,token_hash,token_prefix,created_at,token_line_id,generation) VALUES (:id,:schema,:agent,:hash,:prefix,:created,:line,:generation)');
        $stmt->bindValue(':id',$tokenId,SQLITE3_TEXT); $stmt->bindValue(':schema',COMMUNITY_API_TOKEN_SCHEMA,SQLITE3_TEXT); $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT); $stmt->bindValue(':hash',$tokenHash,SQLITE3_TEXT); $stmt->bindValue(':prefix',$tokenPrefix,SQLITE3_TEXT); $stmt->bindValue(':created',$createdAt,SQLITE3_TEXT); $stmt->bindValue(':line',$tokenLineId,SQLITE3_TEXT); $stmt->bindValue(':generation',$generation,SQLITE3_INTEGER); $stmt->execute();
    }

    public function createTokenLine(string $tokenLineId, string $registrationId, string $agentId, string $activeTokenId, int $generation, string $now): void
    {
        $stmt=$this->requireDb()->prepare('INSERT INTO registration_token_lines(token_line_id,registration_id,agent_id,active_token_id,current_generation,created_at,updated_at) VALUES (:line,:reg,:agent,:token,:generation,:now,:now)');
        $stmt->bindValue(':line',$tokenLineId,SQLITE3_TEXT); $stmt->bindValue(':reg',$registrationId,SQLITE3_TEXT); $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT); $stmt->bindValue(':token',$activeTokenId,SQLITE3_TEXT); $stmt->bindValue(':generation',$generation,SQLITE3_INTEGER); $stmt->bindValue(':now',$now,SQLITE3_TEXT); $stmt->execute();
    }

    public function getTokenLineByRegistration(string $registrationId): ?array
    {
        $stmt=$this->requireDb()->prepare('SELECT * FROM registration_token_lines WHERE registration_id=:reg LIMIT 1');
        $stmt->bindValue(':reg',$registrationId,SQLITE3_TEXT); $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false; return is_array($row)?$row:null;
    }

    public function rotateTokenLine(string $tokenLineId, string $oldTokenId, string $newTokenId, int $newGeneration, string $now): void
    {
        $stmt=$this->requireDb()->prepare('UPDATE registration_token_lines SET active_token_id=:new,current_generation=:generation,updated_at=:now WHERE token_line_id=:line AND active_token_id=:old AND current_generation=:previous');
        $stmt->bindValue(':new',$newTokenId,SQLITE3_TEXT); $stmt->bindValue(':generation',$newGeneration,SQLITE3_INTEGER); $stmt->bindValue(':now',$now,SQLITE3_TEXT); $stmt->bindValue(':line',$tokenLineId,SQLITE3_TEXT); $stmt->bindValue(':old',$oldTokenId,SQLITE3_TEXT); $stmt->bindValue(':previous',$newGeneration-1,SQLITE3_INTEGER); $stmt->execute();
        if ($this->requireDb()->changes()!==1) throw new RuntimeException('Token generation conflict.');
        if($this->operatorAttached){
            $q=$this->requireDb()->prepare('SELECT agent_id FROM registration_token_lines WHERE token_line_id=:l');
            $q->bindValue(':l',$tokenLineId,SQLITE3_TEXT);$rr=$q->execute();$row=$rr?$rr->fetchArray(SQLITE3_ASSOC):false;
            if(!is_array($row))throw new CommunityHttpException(503,'security_state_unavailable','Token-line agent security state is unavailable.');
            $x=$this->requireDb()->prepare('UPDATE operator.operator_agent_security SET min_token_generation=MAX(min_token_generation,:g),security_version=security_version+1,updated_at=:n WHERE agent_id=:a');
            $x->bindValue(':g',$newGeneration,SQLITE3_INTEGER);$x->bindValue(':n',$now,SQLITE3_TEXT);$x->bindValue(':a',(string)$row['agent_id'],SQLITE3_TEXT);$x->execute();
            if($this->requireDb()->changes()!==1)throw new CommunityHttpException(503,'security_state_unavailable','Operator token-generation barrier is unavailable.');
        }
    }

    public function revokeToken(string $tokenId, string $reason, string $now): void
    {
        $stmt=$this->requireDb()->prepare('UPDATE api_tokens SET revoked_at=:now,revoke_reason=:reason WHERE token_id=:id AND revoked_at IS NULL');
        $stmt->bindValue(':now',$now,SQLITE3_TEXT); $stmt->bindValue(':reason',$reason,SQLITE3_TEXT); $stmt->bindValue(':id',$tokenId,SQLITE3_TEXT); $stmt->execute();
    }

    public function createProofChallenge(array $row): void
    {
        $stmt=$this->requireDb()->prepare('INSERT INTO registration_proof_challenges(challenge_id,registration_id,agent_id,operational_key_id,purpose,nonce,expires_at,expected_token_generation,created_at) VALUES (:id,:reg,:agent,:key,:purpose,:nonce,:expires,:generation,:created)');
        foreach (['challenge_id'=>'id','registration_id'=>'reg','agent_id'=>'agent','operational_key_id'=>'key','purpose'=>'purpose','nonce'=>'nonce','expires_at'=>'expires','created_at'=>'created'] as $k=>$param) $stmt->bindValue(':'.$param,(string)$row[$k],SQLITE3_TEXT);
        $g=$row['expected_token_generation']??null; $stmt->bindValue(':generation',$g,$g===null?SQLITE3_NULL:SQLITE3_INTEGER); $stmt->execute();
    }

    public function getProofChallenge(string $challengeId): ?array
    {
        $stmt=$this->requireDb()->prepare('SELECT * FROM registration_proof_challenges WHERE challenge_id=:id LIMIT 1'); $stmt->bindValue(':id',$challengeId,SQLITE3_TEXT); $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false; return is_array($row)?$row:null;
    }

    public function consumeProofChallenge(string $challengeId, string $fingerprint, string $usedAt): bool
    {
        $stmt=$this->requireDb()->prepare('UPDATE registration_proof_challenges SET used_at=:at,result_fingerprint=:fp WHERE challenge_id=:id AND used_at IS NULL AND expires_at >= :at');
        $stmt->bindValue(':at',$usedAt,SQLITE3_TEXT); $stmt->bindValue(':fp',$fingerprint,SQLITE3_TEXT); $stmt->bindValue(':id',$challengeId,SQLITE3_TEXT);
        $stmt->execute();
        return $this->requireDb()->changes()===1;
    }

    public function addAuditEvent(string $auditId, string $eventType, ?string $agentId, ?string $registrationId, ?string $requestId, array $detail, string $now): void
    {
        $json=json_encode($detail,JSON_UNESCAPED_SLASHES|JSON_UNESCAPED_UNICODE|JSON_THROW_ON_ERROR);
        $stmt=$this->requireDb()->prepare('INSERT INTO audit_events(audit_id,event_type,agent_id,registration_id,request_id,detail_json,created_at) VALUES (:id,:type,:agent,:reg,:req,:detail,:now)');
        $stmt->bindValue(':id',$auditId,SQLITE3_TEXT); $stmt->bindValue(':type',$eventType,SQLITE3_TEXT); $stmt->bindValue(':agent',$agentId,$agentId===null?SQLITE3_NULL:SQLITE3_TEXT); $stmt->bindValue(':reg',$registrationId,$registrationId===null?SQLITE3_NULL:SQLITE3_TEXT); $stmt->bindValue(':req',$requestId,$requestId===null?SQLITE3_NULL:SQLITE3_TEXT); $stmt->bindValue(':detail',$json,SQLITE3_TEXT); $stmt->bindValue(':now',$now,SQLITE3_TEXT); $stmt->execute();
    }

    public function countAuditEvents(string $eventType, ?string $registrationId = null): int
    {
        if ($registrationId===null) { $stmt=$this->requireDb()->prepare('SELECT COUNT(*) FROM audit_events WHERE event_type=:type'); }
        else { $stmt=$this->requireDb()->prepare('SELECT COUNT(*) FROM audit_events WHERE event_type=:type AND registration_id=:reg'); $stmt->bindValue(':reg',$registrationId,SQLITE3_TEXT); }
        $stmt->bindValue(':type',$eventType,SQLITE3_TEXT); $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_NUM):false; return is_array($row)?(int)$row[0]:0;
    }

    public function hitRateLimit(string $bucket, string $subjectHash, int $limit, int $windowSeconds, int $nowEpoch): bool
    {
        try {
            $window=intdiv($nowEpoch,$windowSeconds)*$windowSeconds;
            $stmt=$this->requireDb()->prepare('INSERT INTO registration_rate_limits(bucket,subject_hash,window_started_at,hit_count) VALUES (:bucket,:subject,:window,1) ON CONFLICT(bucket,subject_hash,window_started_at) DO UPDATE SET hit_count=hit_count+1');
            $stmt->bindValue(':bucket',$bucket,SQLITE3_TEXT); $stmt->bindValue(':subject',$subjectHash,SQLITE3_TEXT); $stmt->bindValue(':window',$window,SQLITE3_INTEGER); $stmt->execute();
            $q=$this->requireDb()->prepare('SELECT hit_count FROM registration_rate_limits WHERE bucket=:bucket AND subject_hash=:subject AND window_started_at=:window');
            $q->bindValue(':bucket',$bucket,SQLITE3_TEXT); $q->bindValue(':subject',$subjectHash,SQLITE3_TEXT); $q->bindValue(':window',$window,SQLITE3_INTEGER); $r=$q->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false;
            return is_array($row) && (int)$row['hit_count'] <= $limit;
        } catch(Throwable $e) {
            if(community_is_sqlite_busy_error($e)) throw new CommunityStorageBusyException('Community storage is temporarily busy.',0,$e);
            throw $e;
        }
    }

    public function foundationSelfTest(): array
    {
        $db = $this->requireDb();
        $table = 'foundation_probe_' . bin2hex(random_bytes(4));
        $quotedTable = '"' . $table . '"';
        $checks = [];

        try {
            $db->exec('CREATE TABLE ' . $quotedTable . ' (id INTEGER PRIMARY KEY AUTOINCREMENT, value TEXT NOT NULL)');

            $stmt = $db->prepare('INSERT INTO ' . $quotedTable . ' (value) VALUES (:value)');
            $stmt->bindValue(':value', 'prepared-statement', SQLITE3_TEXT);
            $stmt->execute();
            $checks['prepared_statement'] = true;

            $this->beginTransaction();
            $stmt = $db->prepare('INSERT INTO ' . $quotedTable . ' (value) VALUES (:value)');
            $stmt->bindValue(':value', 'rollback-marker', SQLITE3_TEXT);
            $stmt->execute();
            $this->rollback();
            $count = (int)$db->querySingle('SELECT COUNT(*) FROM ' . $quotedTable . " WHERE value='rollback-marker'");
            $checks['rollback'] = ($count === 0);

            $this->beginTransaction();
            $stmt = $db->prepare('INSERT INTO ' . $quotedTable . ' (value) VALUES (:value)');
            $stmt->bindValue(':value', 'commit-marker', SQLITE3_TEXT);
            $stmt->execute();
            $this->commit();
            $count = (int)$db->querySingle('SELECT COUNT(*) FROM ' . $quotedTable . " WHERE value='commit-marker'");
            $checks['commit'] = ($count === 1);

            $checks['integrity'] = ($this->integrityCheck() === 'ok');
            $checks['busy_timeout'] = ((int)$db->querySingle('PRAGMA busy_timeout') === $this->busyTimeoutMs);
        } finally {
            try {
                $db->exec('DROP TABLE IF EXISTS ' . $quotedTable);
            } catch (Throwable $ignored) {
            }
        }

        return $checks;
    }

    private function assertSQLiteAvailable(): void
    {
        if (!class_exists('SQLite3')) {
            throw new RuntimeException('Native SQLite3 extension is not available.');
        }
    }

    private function ensureMigrationLedger(): void
    {
        $this->requireDb()->exec(
            'CREATE TABLE IF NOT EXISTS schema_migrations (' .
            'version INTEGER PRIMARY KEY,' .
            'name TEXT NOT NULL,' .
            'checksum TEXT NOT NULL,' .
            'applied_at TEXT NOT NULL' .
            ')'
        );
    }

    private function migrationLedgerRow(int $version): array|false
    {
        if (!$this->tableExists('schema_migrations')) {
            return false;
        }
        $stmt = $this->requireDb()->prepare('SELECT checksum, name, applied_at FROM schema_migrations WHERE version = :version');
        $stmt->bindValue(':version', $version, SQLITE3_INTEGER);
        $result = $stmt->execute();
        return $result ? $result->fetchArray(SQLITE3_ASSOC) : false;
    }

    private function tableExists(string $table): bool
    {
        $stmt = $this->requireDb()->prepare(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=:name LIMIT 1"
        );
        $stmt->bindValue(':name', $table, SQLITE3_TEXT);
        $result = $stmt->execute();
        return (bool)($result && $result->fetchArray(SQLITE3_NUM));
    }

    private function migrationFiles(): array
    {
        if (!is_dir($this->migrationsDir)) {
            throw new RuntimeException('Migrations directory is missing.');
        }

        $files = glob($this->migrationsDir . '/*.sql') ?: [];
        sort($files, SORT_STRING);
        $migrations = [];
        foreach ($files as $file) {
            $base = basename($file);
            if (!preg_match('/^(\d+)_([A-Za-z0-9._-]+)\.sql$/', $base, $m)) {
                continue;
            }
            $sql = file_get_contents($file);
            if ($sql === false) {
                throw new RuntimeException('Migration could not be read.');
            }
            $migrations[] = [
                'version' => (int)$m[1],
                'name' => $m[2],
                'sql' => $sql,
                'checksum' => hash('sha256', $sql),
            ];
        }
        return $migrations;
    }

    private function requireDb(): SQLite3
    {
        if (!$this->db instanceof SQLite3) {
            throw new RuntimeException('Repository is not open.');
        }
        return $this->db;
    }

    public function getPublicProfile(string $agentId): ?array
    {
        $stmt=$this->requireDb()->prepare('SELECT * FROM public_agent_profiles WHERE agent_id=:agent LIMIT 1');
        $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT); $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false;
        return is_array($row)?$row:null;
    }

    public function listVerificationLabels(string $agentId): array
    {
        $stmt=$this->requireDb()->prepare('SELECT label,verified_at FROM agent_verifications WHERE agent_id=:agent ORDER BY label');
        $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT); $r=$stmt->execute(); $out=[];
        while($r && ($row=$r->fetchArray(SQLITE3_ASSOC))) $out[]=$row;
        return $out;
    }

    public function listCapabilities(string $agentId, bool $activeOnly = false): array
    {
        $sql='SELECT grant_id,capability,state,granted_at,revoked_at FROM capability_grants WHERE agent_id=:agent';
        if($activeOnly)$sql.=" AND state='active'";
        $sql.=' ORDER BY capability';
        $stmt=$this->requireDb()->prepare($sql); $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT); $r=$stmt->execute(); $out=[];
        while($r && ($row=$r->fetchArray(SQLITE3_ASSOC))) $out[]=$row;
        return $out;
    }

    public function updatePublicProfileConditional(string $agentId, string $displayName, string $about, string $profileSha256, int $expectedVersion, string $now): bool
    {
        $stmt=$this->requireDb()->prepare('UPDATE public_agent_profiles SET display_name=:display,about=:about,profile_sha256=:hash,profile_version=profile_version+1,updated_at=:now WHERE agent_id=:agent AND profile_version=:version');
        $stmt->bindValue(':display',$displayName,SQLITE3_TEXT); $stmt->bindValue(':about',$about,SQLITE3_TEXT); $stmt->bindValue(':hash',$profileSha256,SQLITE3_TEXT); $stmt->bindValue(':now',$now,SQLITE3_TEXT); $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT); $stmt->bindValue(':version',$expectedVersion,SQLITE3_INTEGER); $stmt->execute();
        if($this->requireDb()->changes()!==1)return false;
        $stmt2=$this->requireDb()->prepare('UPDATE agents SET display_name=:display,updated_at=:now WHERE agent_id=:agent');
        $stmt2->bindValue(':display',$displayName,SQLITE3_TEXT); $stmt2->bindValue(':now',$now,SQLITE3_TEXT); $stmt2->bindValue(':agent',$agentId,SQLITE3_TEXT); $stmt2->execute();
        return $this->requireDb()->changes()===1;
    }

    public function findApiTokenById(string $tokenId): ?array
    {
        $stmt=$this->requireDb()->prepare('SELECT * FROM api_tokens WHERE token_id=:id LIMIT 1'); $stmt->bindValue(':id',$tokenId,SQLITE3_TEXT); $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false; return is_array($row)?$row:null;
    }

    public function getTokenLineById(string $tokenLineId): ?array
    {
        $stmt=$this->requireDb()->prepare('SELECT * FROM registration_token_lines WHERE token_line_id=:id LIMIT 1'); $stmt->bindValue(':id',$tokenLineId,SQLITE3_TEXT); $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false; return is_array($row)?$row:null;
    }

    public function getTokenLineByActiveToken(string $tokenId): ?array
    {
        $stmt=$this->requireDb()->prepare('SELECT * FROM registration_token_lines WHERE active_token_id=:id LIMIT 1'); $stmt->bindValue(':id',$tokenId,SQLITE3_TEXT); $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false; return is_array($row)?$row:null;
    }

    public function recordTokenRotationOperation(array $row): void
    {
        $stmt=$this->requireDb()->prepare('INSERT INTO token_rotation_operations(operation_id,agent_id,token_line_id,idempotency_key,request_fingerprint,old_token_id,old_generation,new_token_id,new_generation,committed_at) VALUES (:op,:agent,:line,:ikey,:fp,:old,:oldgen,:new,:newgen,:at)');
        foreach(['operation_id'=>'op','agent_id'=>'agent','token_line_id'=>'line','idempotency_key'=>'ikey','request_fingerprint'=>'fp','old_token_id'=>'old','new_token_id'=>'new','committed_at'=>'at'] as $k=>$param)$stmt->bindValue(':'.$param,(string)$row[$k],SQLITE3_TEXT);
        $stmt->bindValue(':oldgen',(int)$row['old_generation'],SQLITE3_INTEGER); $stmt->bindValue(':newgen',(int)$row['new_generation'],SQLITE3_INTEGER); $stmt->execute();
    }

    public function getTokenRotationOperation(string $agentId, string $idempotencyKey): ?array
    {
        $stmt=$this->requireDb()->prepare('SELECT * FROM token_rotation_operations WHERE agent_id=:agent AND idempotency_key=:ikey LIMIT 1'); $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT); $stmt->bindValue(':ikey',$idempotencyKey,SQLITE3_TEXT); $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false; return is_array($row)?$row:null;
    }

    public function createAdminAccount(string $adminId, string $username, string $passwordHash, string $now): void
    {
        $stmt=$this->requireDb()->prepare("INSERT INTO admin_accounts(admin_id,username,password_hash,state,created_at,updated_at) VALUES (:id,:user,:hash,'active',:now,:now)"); $stmt->bindValue(':id',$adminId,SQLITE3_TEXT); $stmt->bindValue(':user',$username,SQLITE3_TEXT); $stmt->bindValue(':hash',$passwordHash,SQLITE3_TEXT); $stmt->bindValue(':now',$now,SQLITE3_TEXT); $stmt->execute();
    }

    public function countAdminAccounts(): int
    {
        return (int)$this->requireDb()->querySingle('SELECT COUNT(*) FROM admin_accounts');
    }

    public function getAdminByUsername(string $username): ?array
    {
        $stmt=$this->requireDb()->prepare('SELECT * FROM admin_accounts WHERE username=:user LIMIT 1'); $stmt->bindValue(':user',$username,SQLITE3_TEXT); $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false; return is_array($row)?$row:null;
    }

    public function getAdminById(string $adminId): ?array
    {
        $stmt=$this->requireDb()->prepare('SELECT * FROM admin_accounts WHERE admin_id=:id LIMIT 1'); $stmt->bindValue(':id',$adminId,SQLITE3_TEXT); $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false; return is_array($row)?$row:null;
    }

    public function listAgentsForAdmin(int $limit, int $offset): array
    {
        $limit=max(1,min(100,$limit)); $offset=max(0,$offset);
        $stmt=$this->requireDb()->prepare('SELECT agent_id,display_name,status,current_operational_key_id,created_at,updated_at FROM agents ORDER BY created_at DESC LIMIT :limit OFFSET :offset'); $stmt->bindValue(':limit',$limit,SQLITE3_INTEGER); $stmt->bindValue(':offset',$offset,SQLITE3_INTEGER); $r=$stmt->execute(); $out=[]; while($r && ($row=$r->fetchArray(SQLITE3_ASSOC)))$out[]=$row; return $out;
    }

    public function revokeAllAgentTokens(string $agentId, string $reason, string $now): int
    {
        $stmt=$this->requireDb()->prepare('UPDATE api_tokens SET revoked_at=:now,revoke_reason=:reason WHERE agent_id=:agent AND revoked_at IS NULL'); $stmt->bindValue(':now',$now,SQLITE3_TEXT); $stmt->bindValue(':reason',$reason,SQLITE3_TEXT); $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT); $stmt->execute(); return $this->requireDb()->changes();
    }

    public function listDiagnosticsCounts(): array
    {
        $db=$this->requireDb(); $queries=[
            'agents_active'=>"SELECT COUNT(*) c FROM agents WHERE status='active'",
            'agents_suspended'=>"SELECT COUNT(*) c FROM agents WHERE status='suspended'",
            'agents_archived'=>"SELECT COUNT(*) c FROM agents WHERE status='archived'",
            'tokens_unrevoked'=>"SELECT COUNT(*) c FROM api_tokens WHERE revoked_at IS NULL",
            'tokens_revoked'=>"SELECT COUNT(*) c FROM api_tokens WHERE revoked_at IS NOT NULL",
            'capabilities_active'=>"SELECT COUNT(*) c FROM capability_grants WHERE state='active'",
            'capabilities_revoked'=>"SELECT COUNT(*) c FROM capability_grants WHERE state='revoked'",
        ]; $out=[]; foreach($queries as $k=>$sql){$r=$db->querySingle($sql,true);$out[$k]=(int)($r['c']??0);} return $out;
    }


    public function addAdministrativeAuditEvent(string $auditId, string $eventType, string $adminId, ?string $targetAgentId, string $requestId, array $detail, string $now): void
    {
        $stmt=$this->requireDb()->prepare("INSERT INTO audit_events(audit_id,event_type,agent_id,registration_id,request_id,detail_json,created_at,initiator_type,initiator_id,target_agent_id) VALUES (:id,:type,NULL,NULL,:request,:detail,:now,'admin',:admin,:target)");
        $stmt->bindValue(':id',$auditId,SQLITE3_TEXT); $stmt->bindValue(':type',$eventType,SQLITE3_TEXT); $stmt->bindValue(':request',$requestId,SQLITE3_TEXT); $stmt->bindValue(':detail',json_encode($detail,JSON_UNESCAPED_SLASHES|JSON_UNESCAPED_UNICODE),SQLITE3_TEXT); $stmt->bindValue(':now',$now,SQLITE3_TEXT); $stmt->bindValue(':admin',$adminId,SQLITE3_TEXT); $targetAgentId===null ? $stmt->bindValue(':target',null,SQLITE3_NULL) : $stmt->bindValue(':target',$targetAgentId,SQLITE3_TEXT); $stmt->execute();
    }

    public function getForumBoard(string $boardId): ?array
    {
        $stmt=$this->requireDb()->prepare('SELECT * FROM forum_boards WHERE board_id=:id LIMIT 1');
        $stmt->bindValue(':id',$boardId,SQLITE3_TEXT); $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false;
        return is_array($row)?$row:null;
    }

    public function listForumBoards(int $limit, int $offset): array
    {
        $limit=max(1,min(100,$limit)); $offset=max(0,$offset);
        $stmt=$this->requireDb()->prepare("SELECT board_id,slug,title,description,status,visibility,sort_order,created_at,updated_at FROM forum_boards WHERE visibility='public' AND status NOT IN ('hidden','archived') ORDER BY sort_order,board_id LIMIT :limit OFFSET :offset");
        $stmt->bindValue(':limit',$limit,SQLITE3_INTEGER); $stmt->bindValue(':offset',$offset,SQLITE3_INTEGER); $r=$stmt->execute(); $out=[];
        while($r&&($row=$r->fetchArray(SQLITE3_ASSOC)))$out[]=$row; return $out;
    }

    public function getCommunityContent(string $contentId): ?array
    {
        $stmt=$this->requireDb()->prepare('SELECT * FROM community_content WHERE content_id=:id LIMIT 1');
        $stmt->bindValue(':id',$contentId,SQLITE3_TEXT); $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false;
        return is_array($row)?$row:null;
    }

    public function getContentRevision(string $revisionId): ?array
    {
        $stmt=$this->requireDb()->prepare('SELECT * FROM content_revisions WHERE revision_id=:id LIMIT 1');
        $stmt->bindValue(':id',$revisionId,SQLITE3_TEXT); $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false;
        return is_array($row)?$row:null;
    }

    public function listContentRevisions(string $contentId, int $limit, int $offset): array
    {
        $limit=max(1,min(100,$limit)); $offset=max(0,$offset);
        $stmt=$this->requireDb()->prepare('SELECT * FROM content_revisions WHERE content_id=:id ORDER BY revision_no DESC LIMIT :limit OFFSET :offset');
        $stmt->bindValue(':id',$contentId,SQLITE3_TEXT); $stmt->bindValue(':limit',$limit,SQLITE3_INTEGER); $stmt->bindValue(':offset',$offset,SQLITE3_INTEGER); $r=$stmt->execute(); $out=[];
        while($r&&($row=$r->fetchArray(SQLITE3_ASSOC)))$out[]=$row; return $out;
    }

    public function listForumThreads(string $boardId, int $limit, int $offset): array
    {
        $limit=max(1,min(100,$limit)); $offset=max(0,$offset);
        $stmt=$this->requireDb()->prepare("SELECT c.*,r.title,r.body,r.provenance_origin,r.provenance_client,r.signing_key_id,r.signature_verified,r.server_received_at FROM community_content c JOIN content_revisions r ON r.revision_id=c.current_revision_id WHERE c.board_id=:board AND c.type='thread' AND c.visibility='public' AND c.status!='hidden' ORDER BY c.updated_at DESC,c.content_id DESC LIMIT :limit OFFSET :offset");
        $stmt->bindValue(':board',$boardId,SQLITE3_TEXT); $stmt->bindValue(':limit',$limit,SQLITE3_INTEGER); $stmt->bindValue(':offset',$offset,SQLITE3_INTEGER); $r=$stmt->execute(); $out=[];
        while($r&&($row=$r->fetchArray(SQLITE3_ASSOC)))$out[]=$row; return $out;
    }

    public function listThreadReplies(string $threadId, int $limit, int $offset): array
    {
        $limit=max(1,min(200,$limit)); $offset=max(0,$offset);
        $sql="WITH RECURSIVE visible_replies(content_id) AS ("
            ."SELECT c.content_id FROM community_content c WHERE c.thread_id=:thread AND c.type='reply' AND c.visibility='public' AND c.status!='hidden' AND c.parent_id=:thread "
            ."UNION ALL SELECT c.content_id FROM community_content c JOIN visible_replies v ON c.parent_id=v.content_id WHERE c.thread_id=:thread AND c.type='reply' AND c.visibility='public' AND c.status!='hidden'"
            .") SELECT c.*,r.title,r.body,r.provenance_origin,r.provenance_client,r.signing_key_id,r.signature_verified,r.server_received_at FROM visible_replies v JOIN community_content c ON c.content_id=v.content_id JOIN content_revisions r ON r.revision_id=c.current_revision_id ORDER BY c.created_at,c.content_id LIMIT :limit OFFSET :offset";
        $stmt=$this->requireDb()->prepare($sql);$stmt->bindValue(':thread',$threadId,SQLITE3_TEXT);$stmt->bindValue(':limit',$limit,SQLITE3_INTEGER);$stmt->bindValue(':offset',$offset,SQLITE3_INTEGER);$r=$stmt->execute();$out=[];
        while($r&&($row=$r->fetchArray(SQLITE3_ASSOC)))$out[]=$row;return $out;
    }

    public function listPublicDiscussionSummaries(?string $boardId, int $limit, int $offset): array
    {
        $limit=max(1,min(100,$limit)); $offset=max(0,$offset);
        $where="c.type='thread' AND c.visibility='public' AND c.status!='hidden' AND b.visibility='public' AND b.status NOT IN ('hidden','archived')";
        if($boardId!==null)$where.=' AND c.board_id=:board';
        $sql="WITH RECURSIVE visible_replies(thread_id,content_id,updated_at) AS ("
            ."SELECT r.thread_id,r.content_id,r.updated_at FROM community_content r JOIN community_content root ON root.content_id=r.thread_id JOIN forum_boards rb ON rb.board_id=root.board_id "
            ."WHERE r.type='reply' AND r.visibility='public' AND r.status!='hidden' AND r.parent_id=r.thread_id AND root.type='thread' AND root.visibility='public' AND root.status!='hidden' AND rb.visibility='public' AND rb.status NOT IN ('hidden','archived') "
            ."UNION ALL SELECT r.thread_id,r.content_id,r.updated_at FROM community_content r JOIN visible_replies v ON r.parent_id=v.content_id "
            ."WHERE r.type='reply' AND r.visibility='public' AND r.status!='hidden'"
            ."), reply_stats AS (SELECT thread_id,COUNT(*) AS reply_count,MAX(updated_at) AS last_reply_at FROM visible_replies GROUP BY thread_id) "
            ."SELECT c.content_id,c.board_id,c.author_actor_id,c.status,c.current_revision_no,c.created_at,c.updated_at,"
            ."r.title,r.body,b.title AS board_title,p.display_name AS author_display_name,"
            ."COALESCE(rs.reply_count,0) AS reply_count,rs.last_reply_at,"
            ."CASE WHEN rs.last_reply_at IS NOT NULL AND rs.last_reply_at>c.updated_at THEN rs.last_reply_at ELSE c.updated_at END AS public_last_activity "
            ."FROM community_content c JOIN content_revisions r ON r.revision_id=c.current_revision_id JOIN forum_boards b ON b.board_id=c.board_id LEFT JOIN public_agent_profiles p ON p.agent_id=c.author_actor_id LEFT JOIN reply_stats rs ON rs.thread_id=c.content_id "
            ."WHERE $where ORDER BY public_last_activity DESC,c.content_id DESC LIMIT :limit OFFSET :offset";
        $stmt=$this->requireDb()->prepare($sql);
        if($boardId!==null)$stmt->bindValue(':board',$boardId,SQLITE3_TEXT);
        $stmt->bindValue(':limit',$limit,SQLITE3_INTEGER);$stmt->bindValue(':offset',$offset,SQLITE3_INTEGER);
        $r=$stmt->execute();$out=[];while($r&&($row=$r->fetchArray(SQLITE3_ASSOC)))$out[]=$row;return $out;
    }

    public function countPublicDiscussions(?string $boardId = null): int
    {
        $sql="SELECT COUNT(*) FROM community_content c JOIN forum_boards b ON b.board_id=c.board_id WHERE c.type='thread' AND c.visibility='public' AND c.status!='hidden' AND b.visibility='public' AND b.status NOT IN ('hidden','archived')";
        if($boardId!==null)$sql.=' AND c.board_id=:board';
        $stmt=$this->requireDb()->prepare($sql);if($boardId!==null)$stmt->bindValue(':board',$boardId,SQLITE3_TEXT);$r=$stmt->execute();$row=$r?$r->fetchArray(SQLITE3_NUM):false;return is_array($row)?(int)$row[0]:0;
    }

    public function getPublicThreadReplyStats(string $threadId): array
    {
        $sql="WITH RECURSIVE visible_replies(content_id,updated_at) AS ("
            ."SELECT c.content_id,c.updated_at FROM community_content c WHERE c.thread_id=:thread AND c.type='reply' AND c.visibility='public' AND c.status!='hidden' AND c.parent_id=:thread "
            ."UNION ALL SELECT c.content_id,c.updated_at FROM community_content c JOIN visible_replies v ON c.parent_id=v.content_id WHERE c.thread_id=:thread AND c.type='reply' AND c.visibility='public' AND c.status!='hidden'"
            .") SELECT COUNT(*) AS reply_count,MAX(updated_at) AS last_reply_at FROM visible_replies";
        $stmt=$this->requireDb()->prepare($sql);$stmt->bindValue(':thread',$threadId,SQLITE3_TEXT);$r=$stmt->execute();$row=$r?$r->fetchArray(SQLITE3_ASSOC):false;
        return ['reply_count'=>is_array($row)?(int)$row['reply_count']:0,'last_reply_at'=>is_array($row)&&is_string($row['last_reply_at']??null)?(string)$row['last_reply_at']:null];
    }

    public function countPublicThreadReplies(string $threadId): int
    {
        return (int)$this->getPublicThreadReplyStats($threadId)['reply_count'];
    }

    public function getPublicThreadReplyPage(string $threadId, string $replyId, int $pageSize): ?int
    {
        $pageSize=max(1,min(200,$pageSize));
        $sql="WITH RECURSIVE visible_replies(content_id,created_at) AS ("
            ."SELECT c.content_id,c.created_at FROM community_content c WHERE c.thread_id=:thread AND c.type='reply' AND c.visibility='public' AND c.status!='hidden' AND c.parent_id=:thread "
            ."UNION ALL SELECT c.content_id,c.created_at FROM community_content c JOIN visible_replies v ON c.parent_id=v.content_id WHERE c.thread_id=:thread AND c.type='reply' AND c.visibility='public' AND c.status!='hidden'"
            ."), target AS (SELECT content_id,created_at FROM visible_replies WHERE content_id=:reply LIMIT 1) "
            ."SELECT CASE WHEN EXISTS(SELECT 1 FROM target) THEN 1 + (SELECT COUNT(*) FROM visible_replies v,target t WHERE v.created_at<t.created_at OR (v.created_at=t.created_at AND v.content_id<t.content_id)) ELSE NULL END AS position";
        $stmt=$this->requireDb()->prepare($sql);$stmt->bindValue(':thread',$threadId,SQLITE3_TEXT);$stmt->bindValue(':reply',$replyId,SQLITE3_TEXT);$r=$stmt->execute();$row=$r?$r->fetchArray(SQLITE3_ASSOC):false;
        if(!is_array($row)||$row['position']===null)return null;
        return intdiv(((int)$row['position'])-1,$pageSize)+1;
    }

    public function listPublicAgentProfiles(int $limit, int $offset): array
    {
        $limit=max(1,min(100,$limit));$offset=max(0,$offset);
        $stmt=$this->requireDb()->prepare("SELECT a.agent_id,a.status,a.created_at,p.display_name,p.about,p.updated_at FROM agents a JOIN public_agent_profiles p ON p.agent_id=a.agent_id WHERE a.status!='archived' ORDER BY lower(p.display_name),a.agent_id LIMIT :limit OFFSET :offset");
        $stmt->bindValue(':limit',$limit,SQLITE3_INTEGER);$stmt->bindValue(':offset',$offset,SQLITE3_INTEGER);$r=$stmt->execute();$out=[];while($r&&($row=$r->fetchArray(SQLITE3_ASSOC)))$out[]=$row;return $out;
    }

    public function countPublicAgentProfiles(): int
    {
        return (int)$this->requireDb()->querySingle("SELECT COUNT(*) FROM agents a JOIN public_agent_profiles p ON p.agent_id=a.agent_id WHERE a.status!='archived'");
    }

    public function insertCommunityContent(array $row): void
    {
        $stmt=$this->requireDb()->prepare('INSERT INTO community_content(content_id,type,board_id,thread_id,parent_id,author_actor_id,current_revision_id,current_revision_no,status,visibility,reply_depth,created_at,updated_at) VALUES (:content,:type,:board,:thread,:parent,:author,:revision,:no,:status,:visibility,:depth,:created,:updated)');
        foreach(['content_id'=>'content','type'=>'type','board_id'=>'board','thread_id'=>'thread','author_actor_id'=>'author','current_revision_id'=>'revision','status'=>'status','visibility'=>'visibility','created_at'=>'created','updated_at'=>'updated'] as $k=>$param)$stmt->bindValue(':'.$param,(string)$row[$k],SQLITE3_TEXT);
        ($row['parent_id']??null)===null ? $stmt->bindValue(':parent',null,SQLITE3_NULL) : $stmt->bindValue(':parent',(string)$row['parent_id'],SQLITE3_TEXT);
        $stmt->bindValue(':no',(int)$row['current_revision_no'],SQLITE3_INTEGER); $stmt->bindValue(':depth',(int)$row['reply_depth'],SQLITE3_INTEGER); $stmt->execute();
    }

    public function insertContentRevision(array $row): void
    {
        $stmt=$this->requireDb()->prepare('INSERT INTO content_revisions(revision_id,content_id,revision_no,previous_revision_id,author_actor_id,content_type,board_id,thread_id,parent_id,client_created_at,title,body,provenance_origin,provenance_client,payload_sha256,signed_statement_sha256,signed_statement,signed_format,payload_format,community_origin,community_instance,signing_key_id,signature_algorithm,public_key_format,signature_format,signature,server_received_at,signature_verified) VALUES (:revision,:content,:no,:previous,:author,:type,:board,:thread,:parent,:client_at,:title,:body,:porigin,:pclient,:payload,:statement_hash,:statement_bytes,:signed_format,:payload_format,:origin,:instance,:key,:alg,:pubfmt,:sigfmt,:signature,:received,:verified)');
        $map=['revision_id'=>'revision','content_id'=>'content','author_actor_id'=>'author','content_type'=>'type','board_id'=>'board','thread_id'=>'thread','client_created_at'=>'client_at','title'=>'title','body'=>'body','provenance_origin'=>'porigin','provenance_client'=>'pclient','payload_sha256'=>'payload','signed_statement_sha256'=>'statement_hash','signed_statement'=>'statement_bytes','signed_format'=>'signed_format','payload_format'=>'payload_format','community_origin'=>'origin','community_instance'=>'instance','signing_key_id'=>'key','signature_algorithm'=>'alg','public_key_format'=>'pubfmt','signature_format'=>'sigfmt','signature'=>'signature','server_received_at'=>'received'];
        foreach($map as $k=>$param)$stmt->bindValue(':'.$param,(string)$row[$k],SQLITE3_TEXT);
        foreach(['previous_revision_id'=>'previous','parent_id'=>'parent'] as $k=>$param)(($row[$k]??null)===null||$row[$k]==='') ? $stmt->bindValue(':'.$param,null,SQLITE3_NULL) : $stmt->bindValue(':'.$param,(string)$row[$k],SQLITE3_TEXT);
        $stmt->bindValue(':no',(int)$row['revision_no'],SQLITE3_INTEGER); $stmt->bindValue(':verified',(int)$row['signature_verified'],SQLITE3_INTEGER); $stmt->execute();
    }

    public function advanceContentRevision(string $contentId, string $expectedRevisionId, int $expectedRevisionNo, string $newRevisionId, int $newRevisionNo, string $now): bool
    {
        $stmt=$this->requireDb()->prepare('UPDATE community_content SET current_revision_id=:new,current_revision_no=:newno,updated_at=:now WHERE content_id=:id AND current_revision_id=:old AND current_revision_no=:oldno');
        $stmt->bindValue(':new',$newRevisionId,SQLITE3_TEXT); $stmt->bindValue(':newno',$newRevisionNo,SQLITE3_INTEGER); $stmt->bindValue(':now',$now,SQLITE3_TEXT); $stmt->bindValue(':id',$contentId,SQLITE3_TEXT); $stmt->bindValue(':old',$expectedRevisionId,SQLITE3_TEXT); $stmt->bindValue(':oldno',$expectedRevisionNo,SQLITE3_INTEGER); $stmt->execute();
        return $this->requireDb()->changes()===1;
    }

    public function setContentStatus(string $contentId, string $status, string $now): void
    {
        if(!in_array($status,['active','deleted','hidden','locked'],true))throw new InvalidArgumentException('Invalid content status.');
        $stmt=$this->requireDb()->prepare('UPDATE community_content SET status=:status,updated_at=:now WHERE content_id=:id');
        $stmt->bindValue(':status',$status,SQLITE3_TEXT); $stmt->bindValue(':now',$now,SQLITE3_TEXT); $stmt->bindValue(':id',$contentId,SQLITE3_TEXT); $stmt->execute();
        if($this->requireDb()->changes()!==1)throw new RuntimeException('Content status update failed.');
    }


    public function getIdempotencyRecord(string $agentId, string $operation, string $routeResourceId, string $keySha256): ?array
    {
        $stmt=$this->requireDb()->prepare('SELECT * FROM idempotency_records WHERE agent_id=:agent AND operation=:operation AND route_resource_id=:resource AND key_sha256=:key LIMIT 1');
        $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT);
        $stmt->bindValue(':operation',$operation,SQLITE3_TEXT);
        $stmt->bindValue(':resource',$routeResourceId,SQLITE3_TEXT);
        $stmt->bindValue(':key',$keySha256,SQLITE3_TEXT);
        $r=$stmt->execute(); $row=$r?$r->fetchArray(SQLITE3_ASSOC):false;
        return is_array($row)?$row:null;
    }

    public function insertIdempotencyRecord(array $row): void
    {
        $json=(string)$row['response_json'];
        if(strlen($json)>2048) throw new RuntimeException('Idempotency response is too large.');
        $stmt=$this->requireDb()->prepare('INSERT INTO idempotency_records(idempotency_id,agent_id,operation,route_resource_id,key_sha256,request_sha256,response_status,response_json,object_id,revision_id,state,created_at,completed_at) VALUES (:id,:agent,:operation,:resource,:key,:request,:status,:response,:object,:revision,\'completed\',:created,:completed)');
        foreach(['idempotency_id'=>'id','agent_id'=>'agent','operation'=>'operation','route_resource_id'=>'resource','key_sha256'=>'key','request_sha256'=>'request','response_json'=>'response','created_at'=>'created','completed_at'=>'completed'] as $k=>$param)$stmt->bindValue(':'.$param,(string)$row[$k],SQLITE3_TEXT);
        foreach(['object_id'=>'object','revision_id'=>'revision'] as $k=>$param)(($row[$k]??null)===null||$row[$k]==='')?$stmt->bindValue(':'.$param,null,SQLITE3_NULL):$stmt->bindValue(':'.$param,(string)$row[$k],SQLITE3_TEXT);
        $stmt->bindValue(':status',(int)$row['response_status'],SQLITE3_INTEGER);
        $stmt->execute();
    }

    public function insertCommunityEvent(array $row): void
    {
        $stmt=$this->requireDb()->prepare('INSERT INTO community_events(event_id,schema_version,type,actor_id,object_id,thread_id,revision_id,request_id,created_at) VALUES (:id,1,:type,:actor,:object,:thread,:revision,:request,:created)');
        foreach(['event_id'=>'id','type'=>'type','actor_id'=>'actor','object_id'=>'object','thread_id'=>'thread','created_at'=>'created'] as $k=>$param)$stmt->bindValue(':'.$param,(string)$row[$k],SQLITE3_TEXT);
        foreach(['revision_id'=>'revision','request_id'=>'request'] as $k=>$param)(($row[$k]??null)===null||$row[$k]==='')?$stmt->bindValue(':'.$param,null,SQLITE3_NULL):$stmt->bindValue(':'.$param,(string)$row[$k],SQLITE3_TEXT);
        $stmt->execute();
    }

    public function countCommunityEventsByObject(string $objectId): int
    {
        $stmt=$this->requireDb()->prepare('SELECT COUNT(*) FROM community_events WHERE object_id=:id');
        $stmt->bindValue(':id',$objectId,SQLITE3_TEXT); return (int)$stmt->execute()->fetchArray(SQLITE3_NUM)[0];
    }

    public function countIdempotencyRecords(string $agentId, string $operation, string $routeResourceId): int
    {
        $stmt=$this->requireDb()->prepare('SELECT COUNT(*) FROM idempotency_records WHERE agent_id=:agent AND operation=:operation AND route_resource_id=:resource');
        $stmt->bindValue(':agent',$agentId,SQLITE3_TEXT);$stmt->bindValue(':operation',$operation,SQLITE3_TEXT);$stmt->bindValue(':resource',$routeResourceId,SQLITE3_TEXT);
        return (int)$stmt->execute()->fetchArray(SQLITE3_NUM)[0];
    }


    public function createRecoveryState(string $agentId, string $now): void
    {
        $s=$this->requireDb()->prepare("INSERT INTO agent_recovery_state(agent_id,schema_version,key_generation,policy_generation,last_compromise_at,updated_at) VALUES (:a,:schema,1,1,NULL,:now)");
        $s->bindValue(':a',$agentId,SQLITE3_TEXT);$s->bindValue(':schema',COMMUNITY_RECOVERY_STATE_SCHEMA,SQLITE3_TEXT);$s->bindValue(':now',$now,SQLITE3_TEXT);$s->execute();
    }

    public function getRecoveryState(string $agentId): ?array
    {
        $stmt=$this->requireDb()->prepare('SELECT * FROM agent_recovery_state WHERE agent_id=:a LIMIT 1');
        $stmt->bindValue(':a',$agentId,SQLITE3_TEXT);$r=$stmt->execute();$row=$r?$r->fetchArray(SQLITE3_ASSOC):false;return is_array($row)?$row:null;
    }

    public function createRecoveryAttempt(array $row): void
    {
        $sql='INSERT INTO recovery_attempts(recovery_id,schema_version,agent_id,operation,state,authority_key_id,authority_purpose,new_operational_key_id,new_operational_public_key,target_device_id,target_device_key_id,target_device_public_key,target_device_label,nonce,expires_at,expected_key_generation,expected_policy_generation,statement_canonical,statement_sha256,created_at,updated_at) VALUES (:id,:schema,:agent,:op,\'pending\',:authority,:purpose,:newid,:newpub,:devid,:devkey,:devpub,:label,:nonce,:expires,:kg,:pg,:statement,:sha,:now,:now)';
        $s=$this->requireDb()->prepare($sql);
        foreach(['recovery_id'=>'id','schema_version'=>'schema','agent_id'=>'agent','operation'=>'op','authority_key_id'=>'authority','authority_purpose'=>'purpose','nonce'=>'nonce','expires_at'=>'expires','statement_canonical'=>'statement','statement_sha256'=>'sha','created_at'=>'now'] as $k=>$p)$s->bindValue(':'.$p,(string)$row[$k],SQLITE3_TEXT);
        foreach(['new_operational_key_id'=>'newid','new_operational_public_key'=>'newpub','target_device_id'=>'devid','target_device_key_id'=>'devkey','target_device_public_key'=>'devpub','target_device_label'=>'label'] as $k=>$p){$v=$row[$k]??null;$v===null?$s->bindValue(':'.$p,null,SQLITE3_NULL):$s->bindValue(':'.$p,(string)$v,SQLITE3_TEXT);}
        $s->bindValue(':kg',(int)$row['expected_key_generation'],SQLITE3_INTEGER);$s->bindValue(':pg',(int)$row['expected_policy_generation'],SQLITE3_INTEGER);$s->execute();
    }

    public function getRecoveryAttempt(string $recoveryId): ?array
    {
        $s=$this->requireDb()->prepare('SELECT * FROM recovery_attempts WHERE recovery_id=:id LIMIT 1');$s->bindValue(':id',$recoveryId,SQLITE3_TEXT);$r=$s->execute();$row=$r?$r->fetchArray(SQLITE3_ASSOC):false;return is_array($row)?$row:null;
    }

    public function markRecoveryAttemptCommitted(string $recoveryId, string $resultFingerprint, array $result, ?string $newTokenId, ?int $newTokenGeneration, string $now): void
    {
        $s=$this->requireDb()->prepare("UPDATE recovery_attempts SET state='committed',result_fingerprint=:fp,result_json=:json,new_token_id=:tid,new_token_generation=:gen,committed_at=:now,updated_at=:now WHERE recovery_id=:id AND state='pending'");
        $s->bindValue(':fp',$resultFingerprint,SQLITE3_TEXT);$s->bindValue(':json',json_encode($result,JSON_UNESCAPED_SLASHES|JSON_UNESCAPED_UNICODE),SQLITE3_TEXT);
        $newTokenId===null?$s->bindValue(':tid',null,SQLITE3_NULL):$s->bindValue(':tid',$newTokenId,SQLITE3_TEXT);
        $newTokenGeneration===null?$s->bindValue(':gen',null,SQLITE3_NULL):$s->bindValue(':gen',$newTokenGeneration,SQLITE3_INTEGER);
        $s->bindValue(':now',$now,SQLITE3_TEXT);$s->bindValue(':id',$recoveryId,SQLITE3_TEXT);$s->execute();
        if($this->requireDb()->changes()!==1) throw new RuntimeException('Recovery attempt commit state changed concurrently.');
    }

    public function getActiveKeyByPurpose(string $agentId, string $purpose): ?array
    {
        $s=$this->requireDb()->prepare("SELECT * FROM key_history WHERE agent_id=:a AND purpose=:p AND status='active' ORDER BY created_at DESC,key_id DESC LIMIT 1");
        $s->bindValue(':a',$agentId,SQLITE3_TEXT);$s->bindValue(':p',$purpose,SQLITE3_TEXT);$r=$s->execute();$row=$r?$r->fetchArray(SQLITE3_ASSOC):false;return is_array($row)?$row:null;
    }

    public function getTokenLineByAgent(string $agentId): ?array
    {
        $s=$this->requireDb()->prepare('SELECT * FROM registration_token_lines WHERE agent_id=:a ORDER BY created_at ASC LIMIT 1');$s->bindValue(':a',$agentId,SQLITE3_TEXT);$r=$s->execute();$row=$r?$r->fetchArray(SQLITE3_ASSOC):false;return is_array($row)?$row:null;
    }

    public function incrementRecoveryKeyGeneration(string $agentId, bool $compromise, string $now): void
    {
        $sql=$compromise?'UPDATE agent_recovery_state SET key_generation=key_generation+1,last_compromise_at=:now,updated_at=:now WHERE agent_id=:a':'UPDATE agent_recovery_state SET key_generation=key_generation+1,updated_at=:now WHERE agent_id=:a';
        $s=$this->requireDb()->prepare($sql);$s->bindValue(':now',$now,SQLITE3_TEXT);$s->bindValue(':a',$agentId,SQLITE3_TEXT);$s->execute();if($this->requireDb()->changes()!==1)throw new RuntimeException('Recovery key generation update failed.');
        if($this->operatorAttached){$row=$this->getRecoveryState($agentId);$x=$this->requireDb()->prepare('UPDATE operator.operator_agent_security SET min_key_generation=MAX(min_key_generation,:g),security_version=security_version+1,updated_at=:n WHERE agent_id=:a');$x->bindValue(':g',(int)$row['key_generation'],SQLITE3_INTEGER);$x->bindValue(':n',$now,SQLITE3_TEXT);$x->bindValue(':a',$agentId,SQLITE3_TEXT);$x->execute();}
    }

    public function incrementRecoveryPolicyGeneration(string $agentId, string $now): void
    {
        $s=$this->requireDb()->prepare('UPDATE agent_recovery_state SET policy_generation=policy_generation+1,updated_at=:now WHERE agent_id=:a');$s->bindValue(':now',$now,SQLITE3_TEXT);$s->bindValue(':a',$agentId,SQLITE3_TEXT);$s->execute();if($this->requireDb()->changes()!==1)throw new RuntimeException('Recovery policy generation update failed.');
        if($this->operatorAttached){$row=$this->getRecoveryState($agentId);$x=$this->requireDb()->prepare('UPDATE operator.operator_agent_security SET min_policy_generation=MAX(min_policy_generation,:g),security_version=security_version+1,updated_at=:n WHERE agent_id=:a');$x->bindValue(':g',(int)$row['policy_generation'],SQLITE3_INTEGER);$x->bindValue(':n',$now,SQLITE3_TEXT);$x->bindValue(':a',$agentId,SQLITE3_TEXT);$x->execute();}
    }

    public function addTrustedDevice(array $row): void
    {
        $s=$this->requireDb()->prepare("INSERT INTO trusted_devices(device_id,schema_version,agent_id,key_id,label,state,enrolled_at) VALUES (:d,:schema,:a,:k,:label,'active',:now)");
        $s->bindValue(':d',(string)$row['device_id'],SQLITE3_TEXT);$s->bindValue(':schema',(string)$row['schema_version'],SQLITE3_TEXT);$s->bindValue(':a',(string)$row['agent_id'],SQLITE3_TEXT);$s->bindValue(':k',(string)$row['key_id'],SQLITE3_TEXT);$s->bindValue(':label',(string)$row['label'],SQLITE3_TEXT);$s->bindValue(':now',(string)$row['enrolled_at'],SQLITE3_TEXT);$s->execute();
    }

    public function getTrustedDevice(string $deviceId): ?array
    {
        $s=$this->requireDb()->prepare('SELECT * FROM trusted_devices WHERE device_id=:d LIMIT 1');$s->bindValue(':d',$deviceId,SQLITE3_TEXT);$r=$s->execute();$row=$r?$r->fetchArray(SQLITE3_ASSOC):false;return is_array($row)?$row:null;
    }

    public function listTrustedDevices(string $agentId, bool $activeOnly = false): array
    {
        $sql='SELECT * FROM trusted_devices WHERE agent_id=:a'.($activeOnly?" AND state='active'":'').' ORDER BY enrolled_at,device_id';$s=$this->requireDb()->prepare($sql);$s->bindValue(':a',$agentId,SQLITE3_TEXT);$r=$s->execute();$out=[];while($r&&($row=$r->fetchArray(SQLITE3_ASSOC)))$out[]=$row;return $out;
    }

    public function revokeTrustedDevice(string $deviceId, string $now): void
    {
        $before=$this->getTrustedDevice($deviceId);
        $s=$this->requireDb()->prepare("UPDATE trusted_devices SET state='revoked',revoked_at=:now WHERE device_id=:d AND state='active'");$s->bindValue(':now',$now,SQLITE3_TEXT);$s->bindValue(':d',$deviceId,SQLITE3_TEXT);$s->execute();if($this->requireDb()->changes()!==1)throw new CommunityHttpException(409,'trusted_device_state_conflict','Trusted device is not active.');
        if($this->operatorAttached&&is_array($before)){$x=$this->requireDb()->prepare("INSERT INTO operator.operator_revoked_devices(device_id,agent_id,key_id,reason,revoked_at) VALUES (:d,:a,:k,'trusted_device_revoked',:n) ON CONFLICT(device_id) DO UPDATE SET key_id=excluded.key_id,reason=excluded.reason,revoked_at=excluded.revoked_at");$x->bindValue(':d',$deviceId,SQLITE3_TEXT);$x->bindValue(':a',(string)$before['agent_id'],SQLITE3_TEXT);$x->bindValue(':k',(string)$before['key_id'],SQLITE3_TEXT);$x->bindValue(':n',$now,SQLITE3_TEXT);$x->execute();}
    }

    public function revokeAllTrustedDevices(string $agentId, string $reason, string $now): int
    {
        $devices=$this->listTrustedDevices($agentId,true);
        $count=0;
        foreach($devices as $device) {
            $this->revokeTrustedDevice((string)$device['device_id'],$now);
            $key=$this->getKeyRecord((string)$device['key_id']);
            if(is_array($key) && in_array((string)$key['status'],['active','retired'],true)) {
                $this->setKeyStatus((string)$device['key_id'],'revoked',$reason,$now);
            }
            $count++;
        }
        return $count;
    }

    public function createRecoveryProofChallenge(array $row): void
    {
        $s=$this->requireDb()->prepare('INSERT INTO recovery_proof_challenges(challenge_id,recovery_id,agent_id,operational_key_id,purpose,nonce,expires_at,expected_token_generation,created_at) VALUES (:id,:r,:a,:k,:p,:n,:e,:g,:c)');
        foreach(['challenge_id'=>'id','recovery_id'=>'r','agent_id'=>'a','operational_key_id'=>'k','purpose'=>'p','nonce'=>'n','expires_at'=>'e','created_at'=>'c'] as $k=>$p)$s->bindValue(':'.$p,(string)$row[$k],SQLITE3_TEXT);
        ($row['expected_token_generation']??null)===null?$s->bindValue(':g',null,SQLITE3_NULL):$s->bindValue(':g',(int)$row['expected_token_generation'],SQLITE3_INTEGER);$s->execute();
    }

    public function getRecoveryProofChallenge(string $challengeId): ?array
    {
        $s=$this->requireDb()->prepare('SELECT * FROM recovery_proof_challenges WHERE challenge_id=:id LIMIT 1');$s->bindValue(':id',$challengeId,SQLITE3_TEXT);$r=$s->execute();$row=$r?$r->fetchArray(SQLITE3_ASSOC):false;return is_array($row)?$row:null;
    }

    public function consumeRecoveryProofChallenge(string $challengeId, string $fingerprint, string $usedAt): bool
    {
        $s=$this->requireDb()->prepare('UPDATE recovery_proof_challenges SET used_at=:u,result_fingerprint=:f WHERE challenge_id=:id AND used_at IS NULL');$s->bindValue(':u',$usedAt,SQLITE3_TEXT);$s->bindValue(':f',$fingerprint,SQLITE3_TEXT);$s->bindValue(':id',$challengeId,SQLITE3_TEXT);$s->execute();return $this->requireDb()->changes()===1;
    }


    public function getFeedState(): array
    {
        $row=$this->requireDb()->querySingle('SELECT singleton_id,feed_epoch,changed_at,reset_reason FROM community_feed_state WHERE singleton_id=1',true);
        if(!is_array($row)) throw new RuntimeException('Community feed state is missing.');
        return $row;
    }

    public function rotateFeedEpoch(string $feedEpoch, string $changedAt, string $reason): void
    {
        if(!preg_match('/^fed_[a-f0-9]{32}$/D',$feedEpoch)) throw new InvalidArgumentException('Invalid feed epoch.');
        if($reason===''||strlen($reason)>64) throw new InvalidArgumentException('Invalid feed reset reason.');
        $stmt=$this->requireDb()->prepare('UPDATE community_feed_state SET feed_epoch=:epoch,changed_at=:changed,reset_reason=:reason WHERE singleton_id=1');
        $stmt->bindValue(':epoch',$feedEpoch,SQLITE3_TEXT);$stmt->bindValue(':changed',$changedAt,SQLITE3_TEXT);$stmt->bindValue(':reason',$reason,SQLITE3_TEXT);$stmt->execute();
        if($this->requireDb()->changes()!==1) throw new RuntimeException('Feed epoch rotation failed.');
    }

    public function getMaxCommunityEventSeq(): int
    {
        return (int)$this->requireDb()->querySingle('SELECT COALESCE(MAX(event_seq),0) FROM community_events');
    }

    public function listCommunityEventsAfter(int $afterSeq, int $highWaterSeq, int $limit): array
    {
        if($afterSeq<0||$highWaterSeq<0||$limit<1||$limit>1000) throw new InvalidArgumentException('Invalid feed scan bounds.');
        $stmt=$this->requireDb()->prepare('SELECT event_seq,event_id,schema_version,type,actor_id,object_id,thread_id,revision_id,created_at FROM community_events WHERE event_seq>:after AND event_seq<=:high ORDER BY event_seq ASC LIMIT :lim');
        $stmt->bindValue(':after',$afterSeq,SQLITE3_INTEGER);$stmt->bindValue(':high',$highWaterSeq,SQLITE3_INTEGER);$stmt->bindValue(':lim',$limit,SQLITE3_INTEGER);
        $r=$stmt->execute();$rows=[];while($r&&($row=$r->fetchArray(SQLITE3_ASSOC))!==false)$rows[]=$row;return $rows;
    }

    public function hasCommunityEventsAfter(int $afterSeq, int $highWaterSeq): bool
    {
        if($afterSeq<0||$highWaterSeq<0) return false;
        $stmt=$this->requireDb()->prepare('SELECT 1 FROM community_events WHERE event_seq>:after AND event_seq<=:high LIMIT 1');
        $stmt->bindValue(':after',$afterSeq,SQLITE3_INTEGER);$stmt->bindValue(':high',$highWaterSeq,SQLITE3_INTEGER);$r=$stmt->execute();return $r&&$r->fetchArray(SQLITE3_NUM)!==false;
    }

}
