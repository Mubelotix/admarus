fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    
    // Download words
    let url = "https://norvig.com/ngrams/count_1w.txt";
    let rep = reqwest::blocking::get(url).unwrap();

    // Keep most common 5000 words
    let mut words = Vec::new();
    for line in rep.text().unwrap().lines() {
        let Some(word) = line.split_once('\t').map(|(w, _)| w) else {continue};
        words.push(word.to_string());
        if words.len() >= 5000 {
            break
        }
    }
    words.sort();

    // Create rfust code
    let mut code = String::new();
    code.push_str("pub const WORDS_EN: &[&str] = &[");
    for word in words.iter() {
        code.push('"');
        code.push_str(word);
        code.push_str("\", ");
    }
    code.push_str("];");

    // Write code to file
    std::fs::write("src/word_lists.rs", code).unwrap();
}
