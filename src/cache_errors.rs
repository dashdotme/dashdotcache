#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("Dependencies are disabled in the cache configuration.")]
    DependenciesDisabled,

    #[error("Parent key '{0}' does not exist.")]
    ParentNotFound(String),

    #[error("Setting parent '{1}' for key '{0}' would create a dependency cycle.")]
    DependencyCycle(String, String),

    #[error("Memory limit exceeded.")]
    MemoryLimitExceeded,

    #[error("Key count limit exceeded.")]
    KeyLimitExceeded,
}
