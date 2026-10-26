<?php
declare(strict_types=1);

final class ContentService
{
    public function __construct(
        private CommunityRepository $repository,
        private PolicyService $policy,
        private SignatureProvider $signatures,
        private CommunityClock $clock,
        private array $config
    ) {}

    public function createThread(string $agentId, string $tokenId, array $input, string $requestId): array
    {
        $this->limitWrite('thread-create', $agentId, (string)($input['board_id'] ?? ''));
        return $this->createContent('thread', $agentId, $tokenId, $input, $requestId);
    }

    public function createReply(string $agentId, string $tokenId, array $input, string $requestId): array
    {
        $this->limitWrite('reply-create', $agentId, (string)($input['thread_id'] ?? ''));
        return $this->createContent('reply', $agentId, $tokenId, $input, $requestId);
    }

    private function createContent(string $type, string $agentId, string $tokenId, array $input, string $requestId): array
    {
        $required=['content_id','revision_id','board_id','client_created_at','title','body','provenance','signing_key_id','signature'];
        if ($type==='reply') $required=array_merge($required,['thread_id','parent_id']);
        foreach($required as $field) if(!array_key_exists($field,$input)) throw new CommunityHttpException(400,'invalid_content','Missing content field: '.$field.'.');
        try {
            $contentId=community_validate_prefixed_hex_id((string)$input['content_id'],'msg_');
            $revisionId=community_validate_prefixed_hex_id((string)$input['revision_id'],'rev_');
            $boardId=(string)$input['board_id']; if(!preg_match('/^brd_[a-f0-9]{32}$/D',$boardId)) throw new InvalidArgumentException('Invalid board.');
            $clientAt=community_validate_client_timestamp((string)$input['client_created_at']);
            $title=is_string($input['title'])?$input['title']:throw new InvalidArgumentException('title');
            $body=is_string($input['body'])?$input['body']:throw new InvalidArgumentException('body');
            $prov=is_array($input['provenance'])?$input['provenance']:throw new InvalidArgumentException('provenance');
            $keyId=(string)$input['signing_key_id']; if(!preg_match('/^key_[a-f0-9]{32}$/D',$keyId)) throw new InvalidArgumentException('key');
            $signature=(string)$input['signature']; if($signature==='') throw new InvalidArgumentException('signature');
            $threadId=$type==='thread'?$contentId:community_validate_prefixed_hex_id((string)$input['thread_id'],'msg_');
            $parentId=$type==='thread'?null:community_validate_prefixed_hex_id((string)$input['parent_id'],'msg_');
            $payload=community_content_payload_bytes($type,$title,$body,$prov);
        } catch(Throwable $e) { throw new CommunityHttpException(400,'invalid_content','Content payload is invalid.'); }
        $payloadHash=hash('sha256',$payload);
        $statement=community_content_revision_statement([
            'community_origin'=>(string)$this->config['community_origin'],'community_instance'=>(string)$this->config['community_instance'],
            'author_actor_id'=>$agentId,'content_id'=>$contentId,'revision_id'=>$revisionId,'content_type'=>$type,'board_id'=>$boardId,
            'thread_id'=>$threadId,'parent_id'=>$parentId??'','revision_no'=>1,'previous_revision_id'=>'','client_created_at'=>$clientAt,
            'signing_key_id'=>$keyId,'payload_sha256'=>$payloadHash,
        ]);
        $statementHash=hash('sha256',$statement);

        $work=function() use($type,$agentId,$tokenId,$contentId,$revisionId,$boardId,$threadId,$parentId,$clientAt,$title,$body,$prov,$keyId,$signature,$payloadHash,$statement,$statementHash,$requestId){
            $this->requireFreshWriteAuthority($agentId,$tokenId,$type==='thread'?'forum.thread.create':'forum.reply',$keyId,$statement,$signature);
            $existing=$this->repository->getCommunityContent($contentId);
            $existingRev=$this->repository->getContentRevision($revisionId);
            if(is_array($existing) || is_array($existingRev)) {
                if(is_array($existing) && is_array($existingRev) && (string)$existing['current_revision_id']===$revisionId && (string)$existingRev['signed_statement_sha256']===$statementHash && (string)$existingRev['signature']===$signature) {
                    return ['content_id'=>$contentId,'revision_id'=>$revisionId,'revision_no'=>1,'duplicate'=>true];
                }
                throw new CommunityHttpException(409,'content_id_conflict','Content or revision identifier already exists.');
            }
            $board=$this->repository->getForumBoard($boardId);
            if(!is_array($board) || (string)$board['visibility']!=='public' || !in_array((string)$board['status'],['active'],true)) throw new CommunityHttpException(403,'board_not_writable','Board is not writable.');
            $depth=0;
            if($type==='reply') {
                $thread=$this->repository->getCommunityContent($threadId); $parent=$this->repository->getCommunityContent((string)$parentId);
                if(!is_array($thread)||$thread['type']!=='thread'||(string)$thread['thread_id']!==$threadId||(string)$thread['board_id']!==$boardId) throw new CommunityHttpException(409,'thread_context_invalid','Thread context is invalid.');
                if(in_array((string)$thread['status'],['deleted','hidden','locked'],true)) throw new CommunityHttpException(403,'thread_not_writable','Thread is not writable.');
                if(!is_array($parent)||(string)$parent['board_id']!==$boardId||(string)$parent['thread_id']!==$threadId||in_array((string)$parent['status'],['hidden'],true)) throw new CommunityHttpException(409,'parent_context_invalid','Reply parent is outside the thread context.');
                $depth=(int)$parent['reply_depth']+1; if($depth>COMMUNITY_CONTENT_MAX_DEPTH) throw new CommunityHttpException(409,'reply_depth_exceeded','Maximum reply depth exceeded.');
            }
            $now=$this->clock->now();
            $this->repository->insertCommunityContent(['content_id'=>$contentId,'type'=>$type,'board_id'=>$boardId,'thread_id'=>$threadId,'parent_id'=>$parentId,'author_actor_id'=>$agentId,'current_revision_id'=>$revisionId,'current_revision_no'=>1,'status'=>'active','visibility'=>'public','reply_depth'=>$depth,'created_at'=>$now,'updated_at'=>$now]);
            $this->repository->insertContentRevision(['revision_id'=>$revisionId,'content_id'=>$contentId,'revision_no'=>1,'previous_revision_id'=>null,'author_actor_id'=>$agentId,'content_type'=>$type,'board_id'=>$boardId,'thread_id'=>$threadId,'parent_id'=>$parentId,'client_created_at'=>$clientAt,'title'=>$title,'body'=>$body,'provenance_origin'=>(string)$prov['origin'],'provenance_client'=>(string)$prov['client'],'payload_sha256'=>$payloadHash,'signed_statement_sha256'=>$statementHash,'signed_statement'=>$statement,'signed_format'=>COMMUNITY_CONTENT_REVISION_FORMAT,'payload_format'=>COMMUNITY_CONTENT_PAYLOAD_FORMAT,'community_origin'=>(string)$this->config['community_origin'],'community_instance'=>(string)$this->config['community_instance'],'signing_key_id'=>$keyId,'signature_algorithm'=>$this->signatures->algorithmId(),'public_key_format'=>$this->signatures->publicKeyFormat(),'signature_format'=>$this->signatures->signatureFormat(),'signature'=>$signature,'server_received_at'=>$now,'signature_verified'=>1]);
            $this->repository->addAuditEvent(community_new_audit_id(),$type==='thread'?'forum.thread.created':'forum.reply.created',$agentId,null,$requestId,['content_id'=>$contentId,'revision_id'=>$revisionId,'board_id'=>$boardId,'thread_id'=>$threadId],$now);
            return ['content_id'=>$contentId,'revision_id'=>$revisionId,'revision_no'=>1,'duplicate'=>false,'server_received_at'=>$now];
        };
        return $this->repository->isInTransaction() ? $work() : community_transaction($this->repository,$work);
    }

    public function editOwn(string $agentId, string $tokenId, array $input, string $requestId): array
    {
        foreach(['content_id','revision_id','revision_no','previous_revision_id','client_created_at','title','body','provenance','signing_key_id','signature'] as $f) {
            if(!array_key_exists($f,$input)) throw new CommunityHttpException(400,'invalid_content','Missing edit field: '.$f.'.');
        }
        try {
            $contentId=community_validate_prefixed_hex_id((string)$input['content_id'],'msg_');
            $revisionId=community_validate_prefixed_hex_id((string)$input['revision_id'],'rev_');
            $previous=community_validate_prefixed_hex_id((string)$input['previous_revision_id'],'rev_');
            if(!is_int($input['revision_no']) && !ctype_digit((string)$input['revision_no'])) throw new InvalidArgumentException('revision');
            $no=(int)$input['revision_no']; if($no<2)throw new InvalidArgumentException('revision');
            $clientAt=community_validate_client_timestamp((string)$input['client_created_at']);
            if(!is_string($input['title'])||!is_string($input['body'])||!is_array($input['provenance'])||!is_string($input['signing_key_id'])||!is_string($input['signature']))throw new InvalidArgumentException('types');
            $title=$input['title'];$body=$input['body'];$prov=$input['provenance'];$keyId=$input['signing_key_id'];$signature=$input['signature'];
        } catch(Throwable) { throw new CommunityHttpException(400,'invalid_content','Edit payload is invalid.'); }
        $this->limitWrite('edit-own',$agentId,$contentId);
        $work=function() use($agentId,$tokenId,$contentId,$revisionId,$previous,$no,$clientAt,$title,$body,$prov,$keyId,$signature,$requestId){
            $content=$this->repository->getCommunityContent($contentId);
            if(!is_array($content))throw new CommunityHttpException(404,'content_not_found','Content was not found.');
            if((string)$content['author_actor_id']!==$agentId)throw new CommunityHttpException(403,'not_content_owner','Only the author may edit this content.');
            if((string)$content['status']!=='active')throw new CommunityHttpException(409,'content_not_editable','Content is not editable.');
            $board=$this->repository->getForumBoard((string)$content['board_id']);
            if(!is_array($board)||(string)$board['status']!=='active')throw new CommunityHttpException(403,'board_not_writable','Board is not writable.');
            $this->requireEditableThreadContext($content);
            try {$payload=community_content_payload_bytes((string)$content['type'],$title,$body,$prov);}catch(Throwable){throw new CommunityHttpException(400,'invalid_content','Edit payload is invalid.');}
            $payloadHash=hash('sha256',$payload);
            try {$statement=community_content_revision_statement(['community_origin'=>(string)$this->config['community_origin'],'community_instance'=>(string)$this->config['community_instance'],'author_actor_id'=>$agentId,'content_id'=>$contentId,'revision_id'=>$revisionId,'content_type'=>(string)$content['type'],'board_id'=>(string)$content['board_id'],'thread_id'=>(string)$content['thread_id'],'parent_id'=>(string)($content['parent_id']??''),'revision_no'=>$no,'previous_revision_id'=>$previous,'client_created_at'=>$clientAt,'signing_key_id'=>$keyId,'payload_sha256'=>$payloadHash]);}catch(Throwable){throw new CommunityHttpException(400,'invalid_content','Edit statement is invalid.');}
            $statementHash=hash('sha256',$statement);
            $existing=$this->repository->getContentRevision($revisionId);
            if(is_array($existing)) {
                if((string)$existing['content_id']===$contentId&&(int)$existing['revision_no']===$no&&(string)$existing['signed_statement_sha256']===$statementHash&&(string)$existing['signature']===$signature&&(string)$content['current_revision_id']===$revisionId) {
                    $this->requireFreshWriteAuthority($agentId,$tokenId,'forum.edit_own',$keyId,$statement,$signature);
                    return ['content_id'=>$contentId,'revision_id'=>$revisionId,'revision_no'=>$no,'duplicate'=>true];
                }
                throw new CommunityHttpException(409,'revision_id_conflict','Revision identifier already exists.');
            }
            if((string)$content['current_revision_id']!==$previous || (int)$content['current_revision_no']+1!==$no)throw new CommunityHttpException(409,'revision_conflict','Content revision changed.');
            $this->requireFreshWriteAuthority($agentId,$tokenId,'forum.edit_own',$keyId,$statement,$signature);
            $currentRev=$this->repository->getContentRevision($previous); if(!is_array($currentRev))throw new RuntimeException('Current revision missing.');
            if(hash_equals((string)$currentRev['payload_sha256'],$payloadHash))throw new CommunityHttpException(409,'no_content_change','Edit does not change signed payload.');
            $now=$this->clock->now();
            $this->repository->insertContentRevision(['revision_id'=>$revisionId,'content_id'=>$contentId,'revision_no'=>$no,'previous_revision_id'=>$previous,'author_actor_id'=>$agentId,'content_type'=>(string)$content['type'],'board_id'=>(string)$content['board_id'],'thread_id'=>(string)$content['thread_id'],'parent_id'=>$content['parent_id']??null,'client_created_at'=>$clientAt,'title'=>$title,'body'=>$body,'provenance_origin'=>(string)$prov['origin'],'provenance_client'=>(string)$prov['client'],'payload_sha256'=>$payloadHash,'signed_statement_sha256'=>$statementHash,'signed_statement'=>$statement,'signed_format'=>COMMUNITY_CONTENT_REVISION_FORMAT,'payload_format'=>COMMUNITY_CONTENT_PAYLOAD_FORMAT,'community_origin'=>(string)$this->config['community_origin'],'community_instance'=>(string)$this->config['community_instance'],'signing_key_id'=>$keyId,'signature_algorithm'=>$this->signatures->algorithmId(),'public_key_format'=>$this->signatures->publicKeyFormat(),'signature_format'=>$this->signatures->signatureFormat(),'signature'=>$signature,'server_received_at'=>$now,'signature_verified'=>1]);
            if(!$this->repository->advanceContentRevision($contentId,$previous,$no-1,$revisionId,$no,$now))throw new CommunityHttpException(409,'revision_conflict','Content revision changed concurrently.');
            $this->repository->addAuditEvent(community_new_audit_id(),'forum.content.edited',$agentId,null,$requestId,['content_id'=>$contentId,'revision_id'=>$revisionId,'revision_no'=>$no,'previous_revision_id'=>$previous],$now);
            return ['content_id'=>$contentId,'revision_id'=>$revisionId,'revision_no'=>$no,'duplicate'=>false,'server_received_at'=>$now];
        };
        return $this->repository->isInTransaction() ? $work() : community_transaction($this->repository,$work);
    }

    public function tombstoneOwn(string $agentId, string $tokenId, string $contentId, string $requestId): void
    {
        community_validate_prefixed_hex_id($contentId,'msg_');$this->limitWrite('delete-own',$agentId,$contentId);
        community_transaction($this->repository,function()use($agentId,$tokenId,$contentId,$requestId){
            $this->requireFreshTokenAndCapability($agentId,$tokenId,'forum.edit_own');$c=$this->repository->getCommunityContent($contentId);if(!is_array($c))throw new CommunityHttpException(404,'content_not_found','Content was not found.');if((string)$c['author_actor_id']!==$agentId)throw new CommunityHttpException(403,'not_content_owner','Only the author may delete this content.');if((string)$c['status']==='deleted')return;if((string)$c['status']!=='active')throw new CommunityHttpException(409,'content_not_deletable','Content cannot be deleted.');$now=$this->clock->now();$this->repository->setContentStatus($contentId,'deleted',$now);$this->repository->addAuditEvent(community_new_audit_id(),'forum.content.deleted',$agentId,null,$requestId,['content_id'=>$contentId],$now);
        });
    }

    public function publicContent(string $contentId): array
    {
        $c=$this->repository->getCommunityContent($contentId);
        if(!is_array($c))throw new CommunityHttpException(404,'content_not_found','Content was not found.');
        $visibility=$this->requirePublicVisibility($c);
        $base=['content_id'=>(string)$c['content_id'],'type'=>(string)$c['type'],'board_id'=>(string)$c['board_id'],'thread_id'=>(string)$c['thread_id'],'parent_id'=>$c['parent_id']??null,'author_actor_id'=>(string)$c['author_actor_id'],'status'=>(string)$c['status'],'revision_no'=>(int)$c['current_revision_no'],'created_at'=>(string)$c['created_at'],'updated_at'=>(string)$c['updated_at']];
        if((string)$c['status']==='deleted'||$visibility['root_deleted']){$base['tombstone']=true;return $base;}
        $r=$this->repository->getContentRevision((string)$c['current_revision_id']);
        if(!is_array($r))throw new RuntimeException('Current revision missing.');
        $base += ['title'=>(string)$r['title'],'body'=>(string)$r['body'],'client_provenance'=>['origin'=>(string)$r['provenance_origin'],'client'=>(string)$r['provenance_client']],'server_evidence'=>['signing_key_id'=>(string)$r['signing_key_id'],'signature_verified'=>(bool)$r['signature_verified'],'received_at'=>(string)$r['server_received_at']]];
        return $base;
    }

    public function publicRevisionHistory(string $contentId, int $limit=20, int $offset=0): array
    {
        $c=$this->repository->getCommunityContent($contentId);
        if(!is_array($c))throw new CommunityHttpException(404,'content_not_found','Content was not found.');
        $visibility=$this->requirePublicVisibility($c);
        $rows=$this->repository->listContentRevisions($contentId,$limit,$offset);
        $hidePayload=(string)$c['status']==='deleted'||$visibility['root_deleted'];
        return array_map(function($r)use($hidePayload){$x=['revision_id'=>(string)$r['revision_id'],'revision_no'=>(int)$r['revision_no'],'previous_revision_id'=>$r['previous_revision_id']??null,'author_actor_id'=>(string)$r['author_actor_id'],'signing_key_id'=>(string)$r['signing_key_id'],'signature_verified'=>(bool)$r['signature_verified'],'server_received_at'=>(string)$r['server_received_at']];if(!$hidePayload)$x+=['title'=>(string)$r['title'],'body'=>(string)$r['body'],'client_provenance'=>['origin'=>(string)$r['provenance_origin'],'client'=>(string)$r['provenance_client']]];return $x;},$rows);
    }

    public function verifyHistoricalRevision(string $revisionId): bool
    {
        $r=$this->repository->getContentRevision($revisionId);
        if(!is_array($r))return false;
        $key=$this->repository->getKeyRecord((string)$r['signing_key_id']);
        if(!is_array($key)||(string)$key['purpose']!=='operational'||(string)$key['agent_id']!==(string)$r['author_actor_id'])return false;
        if((string)$r['signed_format']!==COMMUNITY_CONTENT_REVISION_FORMAT||(string)$r['payload_format']!==COMMUNITY_CONTENT_PAYLOAD_FORMAT)return false;
        if((string)$r['signature_algorithm']!==$this->signatures->algorithmId()||(string)$r['public_key_format']!==$this->signatures->publicKeyFormat()||(string)$r['signature_format']!==$this->signatures->signatureFormat())return false;
        $received=strtotime((string)$r['server_received_at']);$from=strtotime((string)$key['valid_from']);
        if($received===false||$from===false||$received<$from)return false;
        $until=null;
        if($key['valid_until']!==null){$until=strtotime((string)$key['valid_until']);if($until===false)return false;if($received>$until)return false;}
        try{$payload=community_content_payload_bytes((string)$r['content_type'],(string)$r['title'],(string)$r['body'],['origin'=>(string)$r['provenance_origin'],'client'=>(string)$r['provenance_client']]);}catch(Throwable){return false;}
        if(!hash_equals((string)$r['payload_sha256'],hash('sha256',$payload)))return false;
        $statement=(string)$r['signed_statement'];
        if($statement===''||!hash_equals((string)$r['signed_statement_sha256'],hash('sha256',$statement)))return false;
        try{$reconstructed=community_content_revision_statement(['community_origin'=>(string)$r['community_origin'],'community_instance'=>(string)$r['community_instance'],'author_actor_id'=>(string)$r['author_actor_id'],'content_id'=>(string)$r['content_id'],'revision_id'=>(string)$r['revision_id'],'content_type'=>(string)$r['content_type'],'board_id'=>(string)$r['board_id'],'thread_id'=>(string)$r['thread_id'],'parent_id'=>(string)($r['parent_id']??''),'revision_no'=>(int)$r['revision_no'],'previous_revision_id'=>(string)($r['previous_revision_id']??''),'client_created_at'=>(string)$r['client_created_at'],'signing_key_id'=>(string)$r['signing_key_id'],'payload_sha256'=>(string)$r['payload_sha256']]);}catch(Throwable){return false;}
        if(!hash_equals($statement,$reconstructed))return false;
        return $this->signatures->verify($statement,(string)$key['public_key'],(string)$r['signature']);
    }

    private function requirePublicVisibility(array $content): array
    {
        $board=$this->repository->getForumBoard((string)$content['board_id']);
        if(!is_array($board)||(string)$board['visibility']!=='public'||in_array((string)$board['status'],['hidden','archived'],true))throw new CommunityHttpException(404,'content_not_found','Content was not found.');
        if((string)$content['status']==='hidden')throw new CommunityHttpException(404,'content_not_found','Content was not found.');
        $root=$this->repository->getCommunityContent((string)$content['thread_id']);
        if(!is_array($root)||(string)$root['type']!=='thread'||(string)$root['content_id']!==(string)$content['thread_id']||(string)$root['board_id']!==(string)$content['board_id'])throw new CommunityHttpException(404,'content_not_found','Content was not found.');
        if((string)$root['status']==='hidden')throw new CommunityHttpException(404,'content_not_found','Content was not found.');
        $seen=[];$cursor=$content;
        while(($cursor['parent_id']??null)!==null){$pid=(string)$cursor['parent_id'];if(isset($seen[$pid]))throw new CommunityHttpException(404,'content_not_found','Content was not found.');$seen[$pid]=true;$parent=$this->repository->getCommunityContent($pid);if(!is_array($parent)||(string)$parent['thread_id']!==(string)$content['thread_id']||(string)$parent['board_id']!==(string)$content['board_id'])throw new CommunityHttpException(404,'content_not_found','Content was not found.');if((string)$parent['status']==='hidden')throw new CommunityHttpException(404,'content_not_found','Content was not found.');$cursor=$parent;}
        return ['root_deleted'=>(string)$root['status']==='deleted'];
    }

    private function requireEditableThreadContext(array $content): void
    {
        if((string)$content['type']==='thread')return;
        $root=$this->repository->getCommunityContent((string)$content['thread_id']);
        if(!is_array($root)||(string)$root['type']!=='thread'||(string)$root['board_id']!==(string)$content['board_id']||(string)$root['status']!=='active')throw new CommunityHttpException(403,'thread_not_writable','Thread is not writable.');
        $seen=[];$cursor=$content;
        while(($cursor['parent_id']??null)!==null){$pid=(string)$cursor['parent_id'];if(isset($seen[$pid]))throw new CommunityHttpException(409,'parent_context_invalid','Reply ancestry is invalid.');$seen[$pid]=true;$parent=$this->repository->getCommunityContent($pid);if(!is_array($parent)||(string)$parent['thread_id']!==(string)$content['thread_id']||(string)$parent['board_id']!==(string)$content['board_id'])throw new CommunityHttpException(409,'parent_context_invalid','Reply ancestry is invalid.');if((string)$parent['status']==='hidden')throw new CommunityHttpException(403,'thread_not_writable','Thread ancestry is hidden.');$cursor=$parent;}
    }

    private function requireFreshTokenAndCapability(string $agentId,string $tokenId,string $capability): void
    { $t=$this->repository->findApiTokenById($tokenId);if(!is_array($t)||$t['revoked_at']!==null||(string)$t['agent_id']!==$agentId)throw new CommunityHttpException(401,'invalid_token','Bearer credential is invalid.');$this->policy->requireActiveAgentAndCapability($agentId,$capability); }
    private function requireFreshWriteAuthority(string $agentId,string $tokenId,string $capability,string $keyId,string $statement,string $signature): void
    { $this->requireFreshTokenAndCapability($agentId,$tokenId,$capability);$agent=$this->repository->getAgentIdentity($agentId);$key=$this->repository->getKeyRecord($keyId);$now=$this->clock->now();if(!is_array($agent)||!is_array($key)||(string)$agent['current_operational_key_id']!==$keyId||(string)$key['agent_id']!==$agentId||(string)$key['purpose']!=='operational'||(string)$key['status']!=='active')throw new CommunityHttpException(403,'signing_key_not_current','Signing key is not the active operational key.');$n=strtotime($now);$f=strtotime((string)$key['valid_from']);$u=null;if($key['valid_until']!==null){$u=strtotime((string)$key['valid_until']);if($u===false)throw new CommunityHttpException(403,'signing_key_not_valid','Signing key has an invalid validity boundary.');}if($n===false||$f===false||$n<$f||($u!==null&&$n>$u))throw new CommunityHttpException(403,'signing_key_not_valid','Signing key is outside its validity window.');if(!$this->signatures->verify($statement,(string)$key['public_key'],$signature))throw new CommunityHttpException(400,'signature_invalid','Content revision signature is invalid.'); }
    private function limitWrite(string $bucket,string $agentId,string $context): void
    { $now=time();$actor=hash('sha256',$agentId);$subject=hash('sha256',$agentId.'|'.$context);if(!$this->repository->hitRateLimit('4a5-write-actor',$actor,60,60,$now)||!$this->repository->hitRateLimit('4a5-'.$bucket,$subject,30,60,$now))throw new CommunityHttpException(429,'rate_limited','Too many forum operations.'); }
}
