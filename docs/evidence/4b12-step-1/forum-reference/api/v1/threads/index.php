<?php
declare(strict_types=1);
$root=dirname(__DIR__,3);
require_once $root.'/includes/request-context.php'; require_once $root.'/includes/errors.php'; require_once $root.'/includes/api.php';
$rid=community_request_id(); ini_set('display_errors','0'); error_reporting(E_ALL); set_exception_handler(function(Throwable $e)use($rid){$p=community_safe_error_payload($e,$rid);community_json_response($p['status'],$p['body']);});
$method=strtoupper((string)($_SERVER['REQUEST_METHOD']??'GET')); if(!in_array($method,['GET','POST'],true)){header('Allow: GET, POST');throw new CommunityHttpException(405,'method_not_allowed','Method not allowed.');}
$ctx=require $root.'/includes/bootstrap.php'; $repo=$ctx['repository']; $sig=new OpenSSLP256SignatureProvider(); $policy=new PolicyService($repo); $content=new ContentService($repo,$policy,$sig,new SystemCommunityClock(),$ctx['config']);
if($method==='GET') {
    [$page,$limit,$offset]=community_parse_page_limit(25,50);
    $board=(string)($_GET['board_id']??'brd_00000000000000000000000000000001'); try{community_validate_prefixed_hex_id($board,'brd_');}catch(Throwable){throw new CommunityHttpException(400,'invalid_request','board_id is invalid.');}
    if(!community_registration_rate_limit($repo,'4a6.read.threads',$ctx['config']['community_instance'],$board,240,600,5000,60)) throw new CommunityHttpException(429,'rate_limited','Too many read requests.');
    $b=$repo->getForumBoard($board); if(!is_array($b)||(string)$b['visibility']!=='public'||in_array((string)$b['status'],['hidden','archived'],true)) throw new CommunityHttpException(404,'board_not_found','Board was not found.');
    $rows=$repo->listForumThreads($board,$limit,$offset); $items=[];
    foreach($rows as $row){try{$x=$content->publicContent((string)$row['content_id']);if(($x['type']??'')!=='thread')continue;if(isset($x['body'])&&is_string($x['body'])){$preview=$x['body'];if(preg_match('/^.{0,280}/us',$preview,$m))$preview=$m[0];$x['preview']=$preview;unset($x['body']);} $items[]=$x;}catch(CommunityHttpException $e){if($e->status!==404)throw $e;}}
    community_json_response(200,['ok'=>true,'request_id'=>$rid,'page'=>$page,'limit'=>$limit,'items'=>$items]);
}
community_require_json_content_type();
$auth=new AuthService($repo); $a=$auth->authenticateBearer(community_authorization_header());
community_api_attempt_rate_limit($repo,$ctx['config']['community_instance'],$a['agent_id'],'thread.create','');
$ikey=community_validate_idempotency_key(community_require_header('HTTP_IDEMPOTENCY_KEY',128,'invalid_idempotency_key'));
$body=community_read_json_body(COMMUNITY_API_MAX_WRITE_BODY_BYTES);
$svc=new CommunityApiWriteService($repo,$policy,$sig,new SystemCommunityClock(),$ctx['config'],$content);
$r=$svc->execute('thread.create',$a['agent_id'],$a['token_id'],'',$body,$ikey,$rid);
if((int)$r['status']===201) header('Location: ./'.rawurlencode((string)$r['result']['content_id']));
community_json_response((int)$r['status'],['ok'=>true,'request_id'=>$rid,'write'=>$r['result'],'idempotent_replay'=>(bool)$r['idempotent_replay']]);
