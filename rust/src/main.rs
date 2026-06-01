use std::process::ExitCode;

fn main() -> ExitCode {
    match immich_geodata_migration::cli::run(std::env::args().collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
