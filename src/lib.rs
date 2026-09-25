/// Returns the gateway crate version.
pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_version_matches_cargo_package_version() {
        assert_eq!(app_version(), env!("CARGO_PKG_VERSION"));
    }
}
