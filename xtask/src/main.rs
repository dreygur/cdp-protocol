//! Entry point for `just generate-methods`.

fn main() {
    let root = xtask::repository_root();
    let generated = xtask::generate(&root);
    println!(
        "generated {} constants across {} domains",
        generated.commands, generated.domains
    );
}
