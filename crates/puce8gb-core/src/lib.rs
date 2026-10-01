#![forbid(unsafe_code)]

pub fn crate_name() -> &'static str {
    "puce8gb-core"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_is_puce8gb_core() {
        assert_eq!(crate_name(), "puce8gb-core");
    }
}
