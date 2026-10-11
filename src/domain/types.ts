export interface Preferences {schemaVersion:1;revision:number;locale:string;themeId:string;themeVariant:'light'|'dark'|'system'}
export interface Request {contractVersion:1;requestId:string;command:string;payload:Record<string,unknown>;expectedRevision?:number;profileId?:string}
export interface SafeError {code:string;messageKey:string;retryable:boolean;field?:string}
export type Response<T=unknown>={contractVersion:1;requestId:string;ok:true;result:T}|{contractVersion:1;requestId:string;ok:false;error:SafeError};
export interface KernelStatus {schemaVersion:1;lifecycle:'ready';mode:'standard';stateProvider:'null';stage:'4B-0';services:string[];lastEventSequence:number}
export interface AppEvent {contractVersion:1;eventId:string;sequence:number;eventType:'app.preferences.changed';occurredAt:string;payload:Preferences}
export interface EventPage {items:AppEvent[];lastSequence:number;hasMore:boolean}
export interface Transport {mode:'desktop'|'browser';dispatch<T>(r:Request):Promise<Response<T>>;subscribe(handler:(event:AppEvent)=>void):Promise<()=>void>}
