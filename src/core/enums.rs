pub enum NodeType {
    Start,
    Decision,
    Subflow,
    Group,
}

impl NodeType {
    pub fn to_str(&self) -> &str {
        match self {
            NodeType::Start => "start",
            NodeType::Decision => "decision",
            NodeType::Subflow => "subflow",
            NodeType::Group => "group",
        }
    }
}
