fn format_code(message: &str) {
    println!("{}", message);
    let path = "scrap.rs";
    let _ = File::create(path);
    fs::write(path, message).expect("Unable to write");
    let mut tidy = Command::new("rustfmt");
    tidy.arg("scrap.rs");
    tidy.status().expect("not working");
}
