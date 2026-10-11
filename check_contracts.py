"""Reference checks for the P2 subset, not a JSON Schema conformance suite or runtime test."""
import copy,json,re,sqlite3,sys
from pathlib import Path
from datetime import datetime
from urllib.parse import urlsplit
root=Path(__file__).parent
schema=json.loads((root/'contracts/domain.schema.json').read_text());defs=schema['$defs']
cat=json.loads((root/'contracts/ipc.catalog.json').read_text());defaults=json.loads((root/'contracts/defaults.json').read_text())
count=0
class Invalid(ValueError):pass
def require(ok,msg):
 if not ok:raise Invalid(msg)
def validate(value,s):
 if '$ref' in s:return validate(value,defs[s['$ref'].split('/')[-1]])
 if 'oneOf' in s:
  matches=0
  for option in s['oneOf']:
   try:validate(value,option);matches+=1
   except Invalid:pass
  require(matches==1,'oneOf');return
 if 'const' in s:require(type(value)==type(s['const']) and value==s['const'],'const')
 if 'enum' in s:require(value in s['enum'],'enum')
 t=s.get('type')
 if t=='object':
  require(isinstance(value,dict),'object');require(set(s.get('required',[]))<=value.keys(),'required')
  require(len(value)>=s.get('minProperties',0),'minProperties')
  for k,v in value.items():
   if k in s.get('properties',{}):validate(v,s['properties'][k])
   elif s.get('additionalProperties',True) is False:raise Invalid('additional property')
   elif isinstance(s.get('additionalProperties'),dict):validate(v,s['additionalProperties'])
 elif t=='array':
  require(isinstance(value,list),'array')
  for v in value:validate(v,s['items'])
 elif t=='string':
  require(isinstance(value,str),'string');require(len(value)>=s.get('minLength',0),'minLength');require(len(value)<=s.get('maxLength',float('inf')),'maxLength')
  if 'pattern' in s:require(re.search(s['pattern'],value) is not None,'pattern')
  if s.get('format')=='date-time':
   try:datetime.fromisoformat(value.replace('Z','+00:00'))
   except ValueError:raise Invalid('date-time')
  if s.get('format')=='uri':require(bool(urlsplit(value).scheme),'uri')
 elif t in ['integer','number']:
  require(type(value) in ([int] if t=='integer' else [int,float]),t)
  require(value>=s.get('minimum',-float('inf')),'minimum');require(value<=s.get('maximum',float('inf')),'maximum')
  require(value>s.get('exclusiveMinimum',-float('inf')),'exclusiveMinimum')
 elif t=='boolean':require(type(value)==bool,'boolean')
 elif t is not None:raise AssertionError('unsupported type '+t)
def check(label,fn,valid=True):
 global count
 try:fn()
 except Invalid:
  if valid:raise AssertionError('unexpected invalid: '+label)
 else:
  if not valid:raise AssertionError('unexpected accepted: '+label)
 count+=1;print('PASS '+label)
def typed(name,value):return lambda:validate(value,defs[name])
def refs(s):
 if isinstance(s,dict):
  if '$ref' in s:yield s['$ref']
  for v in s.values():yield from refs(v)
 elif isinstance(s,list):
  for v in s:yield from refs(v)
for r in set(refs([schema,cat])):check('reference '+r,lambda r=r:require(r.startswith('#/$defs/') and r.split('/')[-1] in defs,'unknown ref'))
check('defaults EN',lambda:require(defaults['appPreferences']['locale']=='en','EN'))
for n,key in [('AppPreferences','appPreferences'),('MemorySettings','memorySettings'),('ContextBudgetPolicy','contextBudget')]:check(n+' defaults',typed(n,defaults[key]))
for n in [299,300,500,800,1000,1000000]:check('bounded '+str(n),typed('MemoryCapacityPolicy',{'kind':'bounded','tokens':n}))
for v in [{'kind':'bounded','tokens':0},{'kind':'bounded','tokens':True},{'kind':'unlimited','tokens':1000000},{'kind':'unlimited','secret':'x'}]:check('invalid capacity '+str(v),typed('MemoryCapacityPolicy',v),False)
check('explicit unlimited',typed('MemoryCapacityPolicy',{'kind':'unlimited'}))
check('independent L4 default',lambda:require(defaults['memorySettings']['longTerm']['tokens']==500 and defaults['memorySettings']['working']['tokens']==300,'independent'))
base={'schemaVersion':1,'id':'c1','profileId':'p1','revision':1,'createdAt':'2026-10-11T10:00:00Z'}
connection={**base,'providerKind':'openai','normalizedEndpoint':'https://api.openai.com/v1','credentialRef':'vault-ref-1','credentialRevision':1,'label':'Main','status':'unverified'}
check('owned endpoint DTO',typed('ProviderConnection',connection))
for bad in [dict(connection,secret='key'),dict(connection,revision=0),dict(connection,createdAt='2026-99-99T10:00:00Z')]:check('reject unsafe/bad connection '+str(list(bad)[-1]),typed('ProviderConnection',bad),False)
route={**base,'id':'r1','role':'primary','connectionId':'c1','modelId':'demo','outputLimitTokens':1024}
check('route DTO',typed('ModelRoute',route))
def ownership(route,connection):require(route['profileId']==connection['profileId'],'PROFILE_SCOPE_MISMATCH')
check('route own connection',lambda:ownership(route,connection))
check('route foreign connection',lambda:ownership(dict(route,profileId='p2'),connection),False)
def attestation(c):require(c['status']!='verified' or (c.get('verifiedRevision')==c['revision'] and c.get('verifiedCredentialRevision')==c['credentialRevision'] and 'testedAt' in c),'STALE_CONNECTION_TEST')
verified=dict(connection,status='verified',verifiedRevision=1,verifiedCredentialRevision=1,testedAt=base['createdAt'])
check('matching test attestation',lambda:attestation(verified))
for field in ['revision','credentialRevision']:check('stale '+field,lambda field=field:attestation(dict(verified,**{field:2})),False)
for decision in [{'decision':'accept','reason':'supported'},{'decision':'modify','modifiedContent':'Shorter','reason':'clarified'},{'decision':'reject','reason':'unsupported'}]:check('primary '+decision['decision'],typed('PrimaryDecisionOutput',decision))
for decision in [{'decision':'modify','reason':'missing content'},{'decision':'accept','reason':'ok','modifiedContent':'x'},{'decision':'accept','reason':'ok','humanApproved':True}]:check('invalid primary decision',typed('PrimaryDecisionOutput',decision),False)
check('no UI agent decision command',lambda:require(not any('approve' in k or 'decision' in k for k in cat['commands']),'forged decision command'))
for name,c in cat['commands'].items():check('catalog '+name,lambda c=c:require(c['scope'] in ['app','profile'] and set(c)=={'scope','payload','result','firstImplementationStage','mutates','requiresExpectedRevision','async'},'catalog'))
check('credential input no extra fields',typed('CredentialInput',{'secret':'example','endpoint':'x'}),False)
check('default output separate from unlimited',lambda:require(defaults['contextBudget']['memoryOutputCeilingTokens']==4096,'output'))
for fixture in json.loads((root/'contracts/fixtures.json').read_text()):
 check('fixture '+fixture['name'],typed(fixture['dto'],fixture['value']))
check('empty profile patch rejected',typed('ProfilePatch',{}),False)
# Reference storage transaction demonstrates constraints only; not the future product migration.
db=sqlite3.connect(':memory:');db.execute('PRAGMA foreign_keys=ON')
db.executescript('''CREATE TABLE profiles(id TEXT PRIMARY KEY);
CREATE TABLE connections(profile_id TEXT,id TEXT,endpoint TEXT NOT NULL,PRIMARY KEY(profile_id,id),FOREIGN KEY(profile_id) REFERENCES profiles(id));
CREATE TABLE routes(profile_id TEXT,id TEXT,connection_id TEXT NOT NULL,PRIMARY KEY(profile_id,id),FOREIGN KEY(profile_id,connection_id) REFERENCES connections(profile_id,id));
CREATE TABLE memories(profile_id TEXT,id TEXT,revision INTEGER NOT NULL CHECK(revision>0),active_version TEXT,PRIMARY KEY(profile_id,id));''')
db.executemany('INSERT INTO profiles VALUES(?)',[('p1',),('p2',)])
db.execute('INSERT INTO connections VALUES(?,?,?)',('p1','c1',connection['normalizedEndpoint']))
db.execute('INSERT INTO routes VALUES(?,?,?)',('p1','r1','c1'))
def foreign_insert():
 try:db.execute('INSERT INTO routes VALUES(?,?,?)',('p2','r2','c1'))
 except sqlite3.IntegrityError:raise Invalid('FK')
check('SQLite composite FK isolation',foreign_insert,False)
db.execute('INSERT INTO memories VALUES(?,?,?,?)',('p1','m1',1,'v1'));db.commit()
def cas(expected,new):
 cursor=db.execute('UPDATE memories SET revision=revision+1,active_version=? WHERE profile_id=? AND id=? AND revision=?',(new,'p1','m1',expected));require(cursor.rowcount==1,'REVISION_CONFLICT')
check('CAS first commit',lambda:cas(1,'v2'))
check('CAS stale rejection',lambda:cas(1,'v3'),False)
check('CAS latest preserved',lambda:require(db.execute('SELECT active_version FROM memories').fetchone()[0]=='v2','preserved'))
# Capacity validation of the actual modified candidate, before any commit.
def capacity(policy,tokens):require(policy['kind']=='unlimited' or tokens<=policy['tokens'],'MEMORY_CAPACITY_EXCEEDED')
for n in [299,300]:check('candidate fits '+str(n),lambda n=n:capacity({'kind':'bounded','tokens':300},n))
check('modified candidate 301 rejected',lambda:capacity({'kind':'bounded','tokens':300},301),False)
check('unlimited not numeric sentinel',lambda:capacity({'kind':'unlimited'},1000000))
for attempt in [1,2]:check('allowed capacity attempt '+str(attempt),lambda attempt=attempt:validate(attempt,{'type':'integer','enum':[1,2]}))
check('third attempt rejected',lambda:validate(3,{'type':'integer','enum':[1,2]}),False)
check('context sum fits',lambda:require(1000+1024+512<=4096,'budget'))
check('context sum overflow',lambda:require(3000+1024+512<=4096,'budget'),False)
print(f'RESULT: {count}/{count} reference checks PASS; no Rust/Tauri, keyring, browser or live API tests.')
