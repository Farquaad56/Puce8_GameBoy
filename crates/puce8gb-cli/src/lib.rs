pub fn run(args: &[String]) -> (i32, String) {
    if args.iter().any(|arg| arg == "--version") {
        return (0, env!("CARGO_PKG_VERSION").to_string());
    }
    (1, "usage: puce8gb-cli [--version]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_version_returns_code_zero() {
        let (code, _out) = run(&["--version".to_string()]);
        assert_eq!(code, 0);
    }
}
