fn save_code_format(message: &str) {
    let path = "chat_code.rs";
    let _ = File::create(path);
    fs::write(path, message).expect("Unable to write");
    let mut tidy = Command::new("rustfmt");
    tidy.arg(path);
    tidy.status().expect("not working");
}
