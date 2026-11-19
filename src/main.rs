//! The Spelling plugin of Ardoise: the words the Hunspell dictionaries
//! of the machine do not know, in prose and in the comments of programs,
//! with suggestions and a personal dictionary.

mod comments;
mod spell;

use std::collections::HashMap;
use std::path::PathBuf;

use simpleedit_plugin::protocol::InitializeParams;
use simpleedit_plugin::spell::{CheckParams, CheckResult, Language, LearnParams, SuggestParams};
use simpleedit_plugin::{run, Host, Plugin};

/// The words the user added, one per line, in the plugin's data folder.
const PERSONAL: &str = "dictionary.txt";

#[derive(Default)]
struct Spelling {
    data_dir: PathBuf,
    /// The dictionaries found (looked for once).
    languages: Option<Vec<spell::Language>>,
    /// The dictionaries read, by language.
    spellers: HashMap<String, spell::Speller>,
    /// Added for ever / until the editor closes.
    personal: Vec<String>,
    ignored: Vec<String>,
}

impl Spelling {
    fn languages(&mut self) -> &[spell::Language] {
        let dirs = spell::search_dirs(&self.data_dir);
        self.languages
            .get_or_insert_with(|| spell::languages_in(&dirs))
    }

    /// The dictionary of `code`, read the first time it is needed, with the
    /// words of the user.
    fn speller(&mut self, code: &str) -> Result<&spell::Speller, String> {
        if !self.spellers.contains_key(code) {
            let language = self
                .languages()
                .iter()
                .find(|l| l.code == code)
                .cloned()
                .ok_or_else(|| format!("no dictionary for {code}"))?;
            let mut speller = spell::Speller::load(&language)?;
            for word in self.personal.iter() {
                speller.add_personal(word);
            }
            for word in self.ignored.iter() {
                speller.ignore(word);
            }
            self.spellers.insert(code.to_string(), speller);
        }
        Ok(&self.spellers[code])
    }

    fn check(&mut self, params: &CheckParams) -> Result<CheckResult, String> {
        let picked = spell::pick_language(&params.language, &params.interface, self.languages())
            .map(|l| l.code.clone());
        let Some(code) = picked else {
            return Ok(CheckResult::default());
        };
        let speller = self.speller(&code)?;
        Ok(CheckResult {
            misspelled: spell::misspelled(speller, &params.text, &params.syntax),
            language: Some(code),
        })
    }

    fn learn(&mut self, params: &LearnParams) -> Result<(), String> {
        let word = params.word.trim().to_string();
        if word.is_empty() {
            return Ok(());
        }
        for speller in self.spellers.values_mut() {
            if params.keep {
                speller.add_personal(&word);
            } else {
                speller.ignore(&word);
            }
        }
        if !params.keep {
            self.ignored.push(word);
            return Ok(());
        }
        self.personal.push(word);
        let mut text = self.personal.join("\n");
        text.push('\n');
        std::fs::write(self.data_dir.join(PERSONAL), text).map_err(|e| e.to_string())
    }
}

impl Plugin for Spelling {
    fn initialize(&mut self, _: &Host, params: &InitializeParams) {
        self.data_dir = params.data_dir.clone();
        self.personal = std::fs::read_to_string(self.data_dir.join(PERSONAL))
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect();
    }

    fn spell_languages(&mut self, _: &Host) -> Result<Vec<Language>, String> {
        Ok(self
            .languages()
            .iter()
            .map(|l| Language {
                code: l.code.clone(),
            })
            .collect())
    }

    fn spell_check(&mut self, _: &Host, params: &CheckParams) -> Result<CheckResult, String> {
        self.check(params)
    }

    fn spell_suggest(&mut self, _: &Host, params: &SuggestParams) -> Result<Vec<String>, String> {
        Ok(self.speller(&params.language)?.suggest(&params.word))
    }

    fn spell_learn(&mut self, _: &Host, params: &LearnParams) -> Result<(), String> {
        self.learn(params)
    }
}

fn main() -> std::io::Result<()> {
    run(Spelling::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plugin with one dictionary, `en`, in its data folder.
    fn with_dictionary() -> (Spelling, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let dicts = dir.path().join("dictionaries");
        std::fs::create_dir_all(&dicts).unwrap();
        std::fs::write(
            dicts.join("en.aff"),
            "SET UTF-8\nTRY esianrtolcdugmphbyfvkwzxjq\n",
        )
        .unwrap();
        std::fs::write(dicts.join("en.dic"), "3\nhello\nworld\nthe\n").unwrap();
        let plugin = Spelling {
            data_dir: dir.path().to_path_buf(),
            ..Spelling::default()
        };
        (plugin, dir)
    }

    fn check(plugin: &mut Spelling, text: &str) -> CheckResult {
        plugin
            .check(&CheckParams {
                text: text.into(),
                syntax: String::new(),
                language: String::new(),
                interface: "en".into(),
            })
            .unwrap()
    }

    #[test]
    fn words_are_checked_in_the_language_found_and_learnt_for_ever_or_for_now() {
        let (mut plugin, dir) = with_dictionary();
        let found = check(&mut plugin, "hello wrold zork");
        assert_eq!(found.language.as_deref(), Some("en"));
        assert_eq!(found.misspelled, vec![(6, 11), (12, 16)]);
        assert!(plugin
            .speller("en")
            .unwrap()
            .suggest("wrold")
            .contains(&"world".to_string()));
        plugin
            .learn(&LearnParams {
                word: "zork".into(),
                keep: true,
            })
            .unwrap();
        plugin
            .learn(&LearnParams {
                word: "wrold".into(),
                keep: false,
            })
            .unwrap();
        assert!(check(&mut plugin, "hello wrold zork").misspelled.is_empty());
        // Only the word kept for ever is still known by the next run.
        assert_eq!(
            std::fs::read_to_string(dir.path().join(PERSONAL)).unwrap(),
            "zork\n"
        );
        let mut again = Spelling::default();
        again.initialize_from(dir.path());
        assert_eq!(check(&mut again, "wrold zork").misspelled, vec![(0, 5)]);
    }

    #[test]
    fn without_a_dictionary_nothing_is_said() {
        let dir = tempfile::tempdir().unwrap();
        let mut plugin = Spelling {
            data_dir: dir.path().to_path_buf(),
            ..Spelling::default()
        };
        // The machine's own dictionaries may exist: a language none has.
        let found = plugin
            .check(&CheckParams {
                text: "wrold".into(),
                syntax: String::new(),
                language: "zz_ZZ".into(),
                interface: "zz".into(),
            })
            .unwrap();
        assert_eq!(found, CheckResult::default());
    }

    impl Spelling {
        fn initialize_from(&mut self, dir: &std::path::Path) {
            self.data_dir = dir.to_path_buf();
            self.personal = std::fs::read_to_string(dir.join(PERSONAL))
                .unwrap_or_default()
                .lines()
                .map(str::to_string)
                .collect();
        }
    }
}
