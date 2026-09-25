//! Catalog domain concepts: models and providers as provider-independent values.

/// A model identifier as understood by the gateway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// Stable identifier.
    pub id: String,
}

impl Model {
    /// Creates a model with the given identifier.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

/// An LLM service the gateway can route to, without vendor coupling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider {
    /// Stable identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
}

impl Provider {
    /// Creates a provider with the given id and name.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_construction() {
        let model = Model::new("gpt-4o");
        assert_eq!(model.id, "gpt-4o");
    }

    #[test]
    fn provider_construction() {
        let provider = Provider::new("example", "Example Provider");
        assert_eq!(provider.id, "example");
        assert_eq!(provider.name, "Example Provider");
    }
}
