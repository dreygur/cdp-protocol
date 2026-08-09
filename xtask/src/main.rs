//! Entry point for `just generate-methods`.

fn main() {
    let root = xtask::repository_root();
    let generated = xtask::generate(&root);
    println!(
        "generated {} commands, {} events and {} types across {} domains",
        generated.commands, generated.events, generated.types, generated.domains
    );
}
