use agent_core::AgentCore;

#[derive(Clone)]
pub struct ApiState {
    pub core: AgentCore,
}

impl ApiState {
    pub fn new(core: AgentCore) -> Self {
        Self { core }
    }
}
