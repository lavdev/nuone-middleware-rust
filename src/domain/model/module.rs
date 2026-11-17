#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    pub name: String,
    pub id: String,
    pub module_type: String,
}

impl Module {
    pub fn new(
        name: impl Into<String>,
        id: impl Into<String>,
        module_type: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            id: id.into(),
            module_type: module_type.into(),
        }
    }
}
