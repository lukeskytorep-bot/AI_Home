#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lifecycle { Booting, Ready, Failed, Stopping }
pub trait StateProvider: Send + Sync { fn name(&self) -> &'static str; }
pub struct NullState;
impl StateProvider for NullState { fn name(&self) -> &'static str { "null" } }
pub struct Kernel { lifecycle: Lifecycle, state: NullState }
impl Kernel {
    pub fn new() -> Self { Self { lifecycle: Lifecycle::Booting, state: NullState } }
    pub fn ready(&mut self) { assert_eq!(self.lifecycle,Lifecycle::Booting); self.lifecycle=Lifecycle::Ready; }
    pub fn fail(&mut self) { self.lifecycle=Lifecycle::Failed; }
    pub fn stop(&mut self) { self.lifecycle=Lifecycle::Stopping; }
    pub fn status(&self, sequence: i64) -> serde_json::Value {
        serde_json::json!({"schemaVersion":1,"lifecycle": match self.lifecycle { Lifecycle::Ready=>"ready",Lifecycle::Booting=>"booting",Lifecycle::Failed=>"failed",Lifecycle::Stopping=>"stopping" },"mode":"standard","stateProvider":self.state.name(),"stage":"4B-0","services":["preferences","packs","events"],"lastEventSequence":sequence})
    }
}
impl Default for Kernel {fn default()->Self{Self::new()}}
