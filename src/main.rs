use clap::Parser;
use ropey::Rope;
use std::{fs::File, io};
use tracing::info;

mod editor;

#[derive(clap::Parser, Debug)]
struct Args {
    filename: String,
}

fn main() -> std::io::Result<()> {
    let stdout = io::stdout();
    let args = Args::parse();
    /*
        let subscriber = tracing_subscriber::fmt()
            .with_writer(File::options().write(true).open("latest.log").unwrap())
            .with_ansi(false)
            .finish();

        tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    */
    let file = File::open(&args.filename).unwrap();
    let rope = Rope::from_reader(&file).unwrap();

    let mut editor = editor::Editor::new(stdout, rope, args.filename);
    editor.init().unwrap();

    loop {
        if !editor.step()? {
            break;
        }
    }

    editor.close().unwrap();

    Ok(())
}
