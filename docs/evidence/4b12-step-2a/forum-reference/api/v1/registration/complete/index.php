<?php
declare(strict_types=1);
$root=dirname(__DIR__,4); require_once $root.'/includes/request-context.php'; require_once $root.'/includes/errors.php'; require_once $root.'/includes/api.php';
$rid=community_request_id(); ini_set('display_errors','0'); set_exception_handler(function(Throwable $e)use($rid){$p=community_safe_error_payload($e,$rid);community_json_response($p['status'],$p['body']);}); community_require_method('POST');
$input=community_read_json_body(4096); $ctx=require $root.'/includes/bootstrap.php'; require_once $root.'/includes/identity-service.php'; require_once $root.'/includes/key-policy-service.php'; require_once $root.'/includes/registration-service.php';
$repo=$ctx['repository']; $context=is_string($input['registration_id']??null)?$input['registration_id']:'unknown';
if(!community_registration_rate_limit($repo,'registration.complete',$ctx['config']['community_instance'],$context,40,20,200,60)) throw new CommunityHttpException(429,'rate_limited','Too many registration complete requests.');
$sig=new OpenSSLP256SignatureProvider(); $svc=new RegistrationService($repo,$sig,new IdentityService($repo,$sig),$ctx['config']);
$result=$svc->complete($input,$ctx['request_id']); community_json_response(200,['ok'=>true,'request_id'=>$ctx['request_id'],'registration'=>$result]);
