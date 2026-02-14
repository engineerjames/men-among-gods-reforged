use rand::Rng;

pub fn randomly_generate_name() -> String {
    let syl1 = [
        "thi", "ar", "an", "un", "iss", "ish", "urs", "ur", "ent", "esh", "ash", "jey", "jay",
        "dur", "lon", "lan", "len", "lun", "so", "lur", "gar", "cry", "au", "dau", "dei", "zir",
        "zil", "sol", "luc", "ni", "bus", "mid", "err", "doo", "do", "al", "ea", "jac", "ta", "bi",
        "vae", "rif", "tol", "nim", "ru", "li", "fro", "sam", "beut", "bil", "ga", "nee", "ara",
        "rho", "dan", "va", "lan", "cec", "cic", "cac", "cuc", "ix", "vea", "cya", "hie", "bo",
        "ni", "do", "sar", "phe", "ho", "cos", "sin", "tan", "mul", "har", "gur", "tar", "a", "e",
        "i", "o", "u", "je", "ho", "if", "jai", "coy", "ya", "pa", "pul", "pil", "rez", "rel",
        "rar", "dom", "rom", "tom", "ar", "ur", "ir", "er", "yr", "li", "la", "lu", "lo",
    ];
    let syl2 = [
        "tar", "tur", "kar", "kur", "kan", "tan", "gar", "gur", "run",
    ];
    let syl3 = ["a", "e", "i", "o", "u"];

    let mut rng = rand::thread_rng();

    let mut name = String::new();

    let n = rng.gen_range(0..syl1.len());
    name.push_str(syl1[n]);
    if let Some(first_char) = name.chars().next() {
        name.replace_range(0..1, &first_char.to_uppercase().to_string());
    }

    let n = rng.gen_range(0..syl2.len());
    name.push_str(syl2[n]);

    if rng.gen_range(0..2) == 0 {
        return name;
    }

    let n = rng.gen_range(0..syl3.len());
    name.push_str(syl3[n]);

    name
}
