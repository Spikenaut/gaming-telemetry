#[path = "../verify/mod.rs"]
mod verify;

use verify::cli::Args;

fn main() {
    let args = <Args as clap::Parser>::parse();
    match verify::run(args.clone()) {
        Ok(sidecar) => {
            if let Err(err) = verify::write_output(&args, &sidecar) {
                eprintln!("{err}");
                std::process::exit(err.exit_code());
            }
            std::process::exit(verify::success_exit_code(&sidecar));
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(err.exit_code());
        }
    }
}
