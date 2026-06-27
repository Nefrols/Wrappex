fn main() {
    if let Err(error) = wrappex::app::run_from_env() {
        eprintln!("wrappex: {error:#}");
        std::process::exit(1);
    }
}
