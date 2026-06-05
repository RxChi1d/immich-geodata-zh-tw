use std::process::ExitCode;

fn main() -> ExitCode {
    match immich_geodata::cli::run(std::env::args().collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
