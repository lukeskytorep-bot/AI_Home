<?php
declare(strict_types=1);
$root=dirname(__DIR__,4);require_once $root.'/includes/request-context.php';require_once $root.'/includes/errors.php';require_once $root.'/includes/api.php';$rid=community_request_id();ini_set('display_errors','0');set_exception_handler(function(Throwable $e)use($rid){$p=community_safe_error_payload($e,$rid);community_json_response($p['status'],$p['body']);});community_require_method('GET');
$registrationId=(string)($_GET['registration_id']??'');
if(strlen($registrationId)>128)throw new CommunityHttpException(400,'invalid_registration_id','Registration id is invalid.');
$challenge=community_require_header('HTTP_X_COMMUNITY_CHALLENGE_ID',128,'invalid_challenge_header');
$signature=community_require_header('HTTP_X_COMMUNITY_SIGNATURE',1024,'invalid_signature_header');
$ctx=require $root.'/includes/bootstrap.php';require_once $root.'/includes/identity-service.php';require_once $root.'/includes/key-policy-service.php';require_once $root.'/includes/registration-service.php';$repo=$ctx['repository'];
if(!community_registration_rate_limit($repo,'registration.status',$ctx['config']['community_instance'],$registrationId,60,30,300,60))throw new CommunityHttpException(429,'rate_limited','Too many registration status requests.');
$sig=new OpenSSLP256SignatureProvider();$svc=new RegistrationService($repo,$sig,new IdentityService($repo,$sig),$ctx['config']);community_json_response(200,['ok'=>true,'request_id'=>$ctx['request_id'],'registration'=>$svc->status($registrationId,$challenge,$signature)]);
