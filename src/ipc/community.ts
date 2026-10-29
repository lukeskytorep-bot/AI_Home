import type {Request,Transport} from '../domain/types';
import {valid,type JsonSchema} from '../domain/schema';
import contract from '../../contracts/community.local.ipc.json';
export type CommunityCommand = keyof typeof contract.commands;
// Supplemental contracts stay separate from the frozen home_dispatch inventory.
export async function communityCall<T>(transport:Transport,command:CommunityCommand,profileId:string,revision:number,payload:Record<string,unknown>={},invite?:string):Promise<T>{
 if(transport.mode!=='desktop'||!transport.communityDispatch)throw Error('community.unavailable');
 if(command==='community.registration.start'?(!invite||new TextEncoder().encode(invite).length>200||/[\r\n\0]/.test(invite)):invite!==undefined)throw Error('community.invalid');
 const specification=contract.commands[command];
 if(!valid(payload,specification.payload as JsonSchema)||!Number.isSafeInteger(revision)||revision<1)throw Error('community.invalid');
 const request:Request={contractVersion:1,requestId:crypto.randomUUID(),command,profileId,expectedRevision:revision,payload};
 // No retry, logging, storage, or invitation in the request envelope.
 const response=await transport.communityDispatch<T>(request,invite);
 if(response.contractVersion!==1||response.requestId!==request.requestId)throw Error('community.invalid');
 if(!response.ok)throw Error(response.error.code==='REVISION_CONFLICT'?'community.conflict':response.error.code==='POLICY_DENIED'?'community.denied':'community.failed');
 if(!valid(response.result,specification.result as JsonSchema))throw Error('community.invalid');
 const value=response.result as Record<string,unknown>;
 if('preparationState' in value&&((value.prepared===false)!==(value.preparationState==='absent')||(value.preparationState==='absent'?value.registrationRevision!==0:!(Number(value.registrationRevision)>0))))throw Error('community.invalid');
 if('finishState' in value&&(value.finishState==='absent'?value.finishRevision!==0:!(Number(value.finishRevision)>0)))throw Error('community.invalid');
 return response.result;
}
