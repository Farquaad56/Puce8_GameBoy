fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (code, out) = puce8gb_cli::run(&args);
    if !out.is_empty() {
        println!("{out}");
    }
    std::process::exit(code);
}
