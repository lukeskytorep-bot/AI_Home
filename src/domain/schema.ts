import schema from '../../contracts/domain.schema.json';import foundation from '../../contracts/foundation.schema.json';import experimental from '../../contracts/experimental.schema.json';
export type JsonSchema = { $ref?:string; oneOf?:JsonSchema[]; type?:string; const?:unknown; enum?:unknown[]; properties?:Record<string,JsonSchema>; required?:string[]; additionalProperties?:boolean|JsonSchema; items?:JsonSchema; minItems?:number; uniqueItems?:boolean; minProperties?:number; minLength?:number; maxLength?:number; pattern?:string; format?:string; minimum?:number; maximum?:number; exclusiveMinimum?:number };
const definitions={...schema.$defs,...foundation.$defs,...experimental.$defs} as unknown as Record<string,JsonSchema>;
export function valid(value:unknown,s:JsonSchema):boolean {
 if(s.$ref)return valid(value,definitions[s.$ref.split('/').at(-1)!]);
 if(s.oneOf)return s.oneOf.filter(x=>valid(value,x)).length===1;
 if('const' in s && value!==s.const)return false;
 if(s.enum&&!s.enum.includes(value))return false;
 switch(s.type){
 case 'object':{
  if(!value||typeof value!=='object'||Array.isArray(value))return false;
  const o=value as Record<string,unknown>, props=s.properties??{};
  if(Object.keys(o).length<(s.minProperties??0)||(s.required??[]).some(k=>!Object.hasOwn(o,k)))return false;
  return Object.entries(o).every(([k,v])=>Object.hasOwn(props,k)?valid(v,props[k]):s.additionalProperties===false?false:typeof s.additionalProperties==='object'?valid(v,s.additionalProperties):true);
 }
 case 'array':return Array.isArray(value)&&value.length>=(s.minItems??0)&&(!s.uniqueItems||new Set(value.map(v=>JSON.stringify(v))).size===value.length)&&value.every(v=>valid(v,s.items!));
 case 'string':{
  if(typeof value!=='string'||Array.from(value).length<(s.minLength??0)||Array.from(value).length>(s.maxLength??Infinity)||(s.pattern&&!new RegExp(s.pattern).test(value)))return false;
  if(s.format==='date-time')return /^\d{4}-\d{2}-\d{2}T/.test(value)&&!Number.isNaN(Date.parse(value))&&new Date(value).toISOString().slice(0,10)===value.slice(0,10);
  if(s.format==='uri'){try{return Boolean(new URL(value).protocol);}catch{return false;}}
  return true;
 }
 case 'integer':return typeof value==='number'&&Number.isSafeInteger(value)&&value>=(s.minimum??-Infinity)&&value<=(s.maximum??Infinity);
 case 'number':return typeof value==='number'&&Number.isFinite(value)&&value>=(s.minimum??-Infinity)&&value<=(s.maximum??Infinity)&&value>(s.exclusiveMinimum??-Infinity);
 case 'boolean':return typeof value==='boolean';
 default:return true;
 }
}
export function validDto(name:string,value:unknown):boolean{return Object.hasOwn(definitions,name)&&valid(value,definitions[name]);}
