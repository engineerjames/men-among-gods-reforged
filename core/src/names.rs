use rand::Rng;

const SYL1: [&str; 107] = [
    "thi", "ar", "an", "un", "iss", "ish", "urs", "ur", "ent", "esh", "ash", "jey", "jay", "dur",
    "lon", "lan", "len", "lun", "so", "lur", "gar", "cry", "au", "dau", "dei", "zir", "zil", "sol",
    "luc", "ni", "bus", "mid", "err", "doo", "do", "al", "ea", "jac", "ta", "bi", "vae", "rif",
    "tol", "nim", "ru", "li", "fro", "sam", "beut", "bil", "ga", "nee", "ara", "rho", "dan", "va",
    "lan", "cec", "cic", "cac", "cuc", "ix", "vea", "cya", "hie", "bo", "ni", "do", "sar", "phe",
    "ho", "cos", "sin", "tan", "mul", "har", "gur", "tar", "a", "e", "i", "o", "u", "je", "ho",
    "if", "jai", "coy", "ya", "pa", "pul", "pil", "rez", "rel", "rar", "dom", "rom", "tom", "ar",
    "ur", "ir", "er", "yr", "li", "la", "lu", "lo",
];
const SYL2: [&str; 9] = [
    "tar", "tur", "kar", "kur", "kan", "tan", "gar", "gur", "run",
];
const SYL3: [&str; 5] = ["a", "e", "i", "o", "u"];

pub fn randomly_generate_name() -> String {
    let mut rng = rand::thread_rng();

    let mut name = String::new();

    let n = rng.gen_range(0..SYL1.len());
    name.push_str(SYL1[n]);
    if let Some(first_char) = name.chars().next() {
        name.replace_range(0..1, &first_char.to_uppercase().to_string());
    }

    let n = rng.gen_range(0..SYL2.len());
    name.push_str(SYL2[n]);

    if rng.gen_range(0..2) == 0 {
        return name;
    }

    let n = rng.gen_range(0..SYL3.len());
    name.push_str(SYL3[n]);

    name
}

#[cfg(test)]
mod tests {
    use crate::names::*;

    use super::randomly_generate_name;
    use std::collections::HashSet;

    fn is_ascii_title_case(name: &str) -> bool {
        let mut chars = name.chars();
        let Some(first) = chars.next() else {
            return false;
        };

        first.is_ascii_uppercase() && chars.all(|c| c.is_ascii_lowercase())
    }

    fn parse_generated_name(name: &str) -> Option<bool> {
        let lower = name.to_lowercase();

        for first in SYL1 {
            if !lower.starts_with(first) {
                continue;
            }

            let rem_after_first = &lower[first.len()..];
            for second in SYL2 {
                if !rem_after_first.starts_with(second) {
                    continue;
                }

                let rem_after_second = &rem_after_first[second.len()..];
                if rem_after_second.is_empty() {
                    return Some(false);
                }
                if SYL3.contains(&rem_after_second) {
                    return Some(true);
                }
            }
        }

        None
    }

    #[test]
    fn generated_names_match_expected_template_and_casing() {
        for _ in 0..2_000 {
            let name = randomly_generate_name();
            assert!(!name.is_empty(), "generated name should never be empty");
            assert!(
                is_ascii_title_case(&name),
                "generated name should be title-cased ascii: {name}"
            );
            assert!(
                parse_generated_name(&name).is_some(),
                "generated name does not match syllable template: {name}"
            );
        }
    }

    #[test]
    fn generated_names_respect_length_bounds() {
        let min_syl1 = SYL1
            .iter()
            .map(|s| s.len())
            .min()
            .expect("SYL1 must not be empty");
        let max_syl1 = SYL1
            .iter()
            .map(|s| s.len())
            .max()
            .expect("SYL1 must not be empty");
        let min_syl2 = SYL2
            .iter()
            .map(|s| s.len())
            .min()
            .expect("SYL2 must not be empty");
        let max_syl2 = SYL2
            .iter()
            .map(|s| s.len())
            .max()
            .expect("SYL2 must not be empty");
        let min_syl3 = SYL3
            .iter()
            .map(|s| s.len())
            .min()
            .expect("SYL3 must not be empty");
        let max_syl3 = SYL3
            .iter()
            .map(|s| s.len())
            .max()
            .expect("SYL3 must not be empty");

        let min_len = min_syl1 + min_syl2;
        let max_len = max_syl1 + max_syl2 + max_syl3;

        for _ in 0..2_000 {
            let name = randomly_generate_name();
            assert!(
                (min_len..=max_len).contains(&name.len()),
                "generated name has unexpected length {} (expected {}..={}): {}",
                name.len(),
                min_len,
                max_len,
                name
            );
        }

        assert_eq!(
            min_syl3, max_syl3,
            "third syllables are expected to be single-char vowels"
        );
    }

    #[test]
    fn generator_produces_both_two_and_three_syllable_names() {
        let mut saw_two_syllable = false;
        let mut saw_three_syllable = false;

        for _ in 0..512 {
            let name = randomly_generate_name();
            let has_third = parse_generated_name(&name)
                .expect("every generated name should match known syllable structure");
            if has_third {
                saw_three_syllable = true;
            } else {
                saw_two_syllable = true;
            }

            if saw_two_syllable && saw_three_syllable {
                break;
            }
        }

        assert!(
            saw_two_syllable,
            "expected to see at least one 2-syllable name"
        );
        assert!(
            saw_three_syllable,
            "expected to see at least one 3-syllable name"
        );
    }

    #[test]
    fn generator_has_basic_output_variation() {
        let mut unique = HashSet::new();

        for _ in 0..256 {
            unique.insert(randomly_generate_name());
        }

        assert!(
            unique.len() >= 16,
            "expected at least modest variation; got only {} unique names",
            unique.len()
        );
    }
}
