//! Replaceable, deterministic lexical ranking; no network, storage, or model calls.
use std::collections::BTreeSet;
pub struct SearchDocument {pub id:String,pub text:String,pub importance:u64}
pub trait MemoryRetriever {fn rank(&self,query:&str,documents:&[SearchDocument])->Vec<(usize,usize)>;}
pub struct LexicalRetriever;
fn terms(text:&str)->BTreeSet<String>{text.split(|c:char|!c.is_alphanumeric()).filter(|s|!s.is_empty()).map(str::to_lowercase).collect()}
impl MemoryRetriever for LexicalRetriever {
 fn rank(&self,query:&str,documents:&[SearchDocument])->Vec<(usize,usize)>{
  let q=terms(query);let mut matches:Vec<_>=documents.iter().enumerate().filter_map(|(i,d)|{let score=q.intersection(&terms(&d.text)).count();(score>0).then_some((i,score))}).collect();
  matches.sort_by(|(a,sa),(b,sb)|sb.cmp(sa).then_with(||documents[*b].importance.cmp(&documents[*a].importance)).then_with(||documents[*a].id.cmp(&documents[*b].id)));matches
 }
}
#[cfg(test)]mod tests {use super::*;
 #[test]fn unicode_exact_terms_and_stable_ties(){let d=vec![SearchDocument{id:"b".into(),text:"ŻÓŁW kot".into(),importance:0},SearchDocument{id:"a".into(),text:"żółw żółw kot".into(),importance:0},SearchDocument{id:"c".into(),text:"żółwiki".into(),importance:99}];assert_eq!(LexicalRetriever.rank("żółw KOT",&d),vec![(1,2),(0,2)]);assert!(LexicalRetriever.rank("",&d).is_empty());}
 #[test]fn importance_breaks_only_matching_score_ties(){let d=vec![SearchDocument{id:"a".into(),text:"cat".into(),importance:0},SearchDocument{id:"b".into(),text:"cat".into(),importance:100},SearchDocument{id:"c".into(),text:"dog".into(),importance:100}];assert_eq!(LexicalRetriever.rank("cat",&d),vec![(1,1),(0,1)]);}
}
