use rand::Rng;
use std::collections::HashSet;
use std::iter::repeat;

#[derive(Debug)]
pub struct WordStonksGame {
    vocabulary: HashSet<String>,
    word_to_guess: String,
    current_word_interval: WordInterval,
    game_over: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WordInterval {
    pub lower_bound: String,
    pub upper_bound: String,
}

#[derive(Debug)]
pub enum GuessResult {
    Correct,
    Incorrect(WordInterval),
    InvalidWord,
    OutOfRange,
    GameOver(String),
}

impl WordStonksGame {
    pub fn new(vocabulary_txt: &str) -> WordStonksGame {
        let mut vocabulary = HashSet::new();
        let mut vocabulary_list = vec![];
        let mut initial_word_interval = WordInterval {
            lower_bound: "zzzzzz".to_owned(),
            upper_bound: "aaaaaa".to_owned(),
        };
        for word in vocabulary_txt.split('\n') {
            let word = word.to_lowercase();
            if word.is_empty() {
                continue;
            }
            if word < initial_word_interval.lower_bound {
                initial_word_interval.lower_bound = word.to_owned();
            }
            if word > initial_word_interval.upper_bound {
                initial_word_interval.upper_bound = word.to_owned();
            }
            vocabulary_list.push(word.to_owned());
            vocabulary.insert(word.to_owned());
        }
        let mut rng = rand::thread_rng();
        let word_to_guess = vocabulary_list[rng.gen_range(0..vocabulary_list.len())].clone();
        WordStonksGame {
            vocabulary,
            word_to_guess,
            current_word_interval: initial_word_interval,
            game_over: false,
        }
    }

    pub fn guess(&mut self, word: &str) -> GuessResult {
        if self.game_over {
            return GuessResult::GameOver(self.word_to_guess.clone());
        }
        if word == self.word_to_guess {
            self.game_over = true;
            return GuessResult::Correct;
        }
        if !self.vocabulary.contains(word) {
            return GuessResult::InvalidWord;
        }
        let word = String::from(word);
        if word > self.current_word_interval.lower_bound
            && word < self.current_word_interval.upper_bound
        {
            if word > self.word_to_guess {
                self.current_word_interval.upper_bound = word;
            } else {
                self.current_word_interval.lower_bound = word;
            }
            return GuessResult::Incorrect(self.current_word_interval.clone());
        }
        GuessResult::OutOfRange
    }
    pub fn current_word_interval(&self) -> &WordInterval {
        &self.current_word_interval
    }

    pub fn hamming_distance(&self, guess: String) -> u32 {
        // Not the cleanest solution, but words won't be that large, so this clone should be okay.
        let word1 = self.word_to_guess.clone();
        // w1 is always the longer word.
        let (w1, mut w2) = if word1.len() > guess.len() {
            (word1, guess)
        } else {
            (guess, word1)
        };
        // Generate the correct amount of spaces to 'pad' the shorter string.
        let append_spaces = repeat(" ").take(w1.len() - w2.len()).collect::<String>();
        // Push spaces to the shorter string.
        w2.push_str(&append_spaces);
        // Calculating the Hamming distance
        w1.chars().zip(w2.chars()).filter(|(x, y)| x != y).count() as u32
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    impl WordStonksGame {
        // Force the word to guess for testing purposes.
        fn new_for_testing(vocabulary_txt: &str, word_to_guess: &str) -> WordStonksGame {
            let game = WordStonksGame::new(vocabulary_txt);
            WordStonksGame {
                word_to_guess: word_to_guess.to_owned(),
                ..game
            }
        }
    }

    #[test]
    fn test_hamming_distance() {
        let forced_word = "Pong";
        let game =
            WordStonksGame::new_for_testing(include_str!("../assets/words.txt"), forced_word);
        // Best test (and also different length of words).
        let w1 = String::from("Stonk");
        assert_eq!(game.hamming_distance(w1), 5);
        // When both words are the same.
        let w1 = String::from("Pong");
        assert_eq!(game.hamming_distance(w1), 0);
        // When both words are the same but different cases.
        let w1 = String::from("pong");
        assert_eq!(game.hamming_distance(w1), 1);
        // When one of the words is the empty string.
        let w1 = String::from("");
        assert_eq!(game.hamming_distance(w1), 4);
    }

    #[test]
    fn test_game() {
        let forced_word = "pond";
        let mut game =
            WordStonksGame::new_for_testing(include_str!("../assets/words.txt"), forced_word);
        let initial_word_interval = WordInterval {
            lower_bound: "aardvark".to_owned(),
            upper_bound: "zyzzyva".to_owned(),
        };
        assert_eq!(&initial_word_interval, game.current_word_interval());

        let invalid_word = "xyz";
        assert!(!game.vocabulary.contains(invalid_word));
        assert_matches!(game.guess(invalid_word), GuessResult::InvalidWord);

        let valid_word_lower = "fork";
        assert!(game.vocabulary.contains(valid_word_lower));
        assert_matches!(game.guess(valid_word_lower), GuessResult::Incorrect(word_interval) => {
            assert_eq!(word_interval.lower_bound, valid_word_lower);
            assert_eq!(word_interval.upper_bound, initial_word_interval.upper_bound);
        });

        {
            let current_word_interval = game.current_word_interval();
            assert_eq!(current_word_interval.lower_bound, valid_word_lower);
            assert_eq!(
                current_word_interval.upper_bound,
                initial_word_interval.upper_bound
            );
        }

        let valid_word_upper = "respond";
        assert!(game.vocabulary.contains(valid_word_upper));
        assert_matches!(game.guess(valid_word_upper), GuessResult::Incorrect(word_interval) => {
            assert_eq!(word_interval.lower_bound, valid_word_lower);
            assert_eq!(word_interval.upper_bound, valid_word_upper);
        });

        {
            let current_word_interval = game.current_word_interval();
            assert_eq!(current_word_interval.lower_bound, valid_word_lower);
            assert_eq!(current_word_interval.upper_bound, valid_word_upper);
        }

        assert_matches!(game.guess(forced_word), GuessResult::Correct);

        assert_matches!(game.guess(forced_word), GuessResult::GameOver(s) => { assert_eq!(s, forced_word)});
        assert_matches!(game.guess(invalid_word), GuessResult::GameOver(s) => { assert_eq!(s, forced_word)});
        assert_matches!(game.guess(valid_word_lower), GuessResult::GameOver(s) => { assert_eq!(s, forced_word)});
        assert_matches!(game.guess(valid_word_upper), GuessResult::GameOver(s) => { assert_eq!(s, forced_word)});
    }
}
