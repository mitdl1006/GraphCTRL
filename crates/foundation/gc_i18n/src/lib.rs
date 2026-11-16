//! User-interface text in Korean and English.
//!
//! Every message is a variant of [`Msg`], declared once together with both translations, so a missing
//! translation or a misspelt key cannot compile. Widgets and tests refer to messages by variant, never by
//! their text.

#![forbid(unsafe_code)]

use std::fmt::Display;

/// A user-interface language.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lang {
    /// Korean (the default).
    #[default]
    Ko,
    /// English.
    En,
}

impl Lang {
    /// Every supported language.
    pub const ALL: [Self; 2] = [Self::Ko, Self::En];

    /// The language's own name, for the language menu.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::Ko => "한국어",
            Self::En => "English",
        }
    }

    /// The code stored in the settings file.
    pub fn code(self) -> &'static str {
        match self {
            Self::Ko => "ko",
            Self::En => "en",
        }
    }

    /// The language with the given settings code.
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|lang| lang.code() == code)
    }
}

macro_rules! messages {
    ($( $(#[$doc:meta])* $key:ident => ($ko:literal, $en:literal), )*) => {
        /// A translatable message. `{0}`, `{1}`... in the text are filled by [`Msg::format`].
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Msg {
            $( $(#[$doc])* $key, )*
        }

        impl Msg {
            /// Every message, for tests that check the whole catalogue.
            pub const ALL: &'static [Self] = &[ $( Self::$key, )* ];

            /// The text in the given language.
            pub fn text(self, lang: Lang) -> &'static str {
                match (self, lang) {
                    $( (Self::$key, Lang::Ko) => $ko, (Self::$key, Lang::En) => $en, )*
                }
            }

            /// The variant name: a stable identifier that does not depend on the language.
            pub fn key(self) -> &'static str {
                match self {
                    $( Self::$key => stringify!($key), )*
                }
            }
        }
    };
}

messages! {
    /// Application name in the title bar.
    AppTitle => ("GraphCTRL", "GraphCTRL"),
    /// Menu: File.
    MenuFile => ("파일", "File"),
    /// Menu: Edit.
    MenuEdit => ("편집", "Edit"),
    /// Menu: View.
    MenuView => ("뷰", "View"),
    /// Menu: Tools.
    MenuTools => ("도구", "Tools"),
    /// Menu: Help.
    MenuHelp => ("도움말", "Help"),
    /// Command: New document.
    CmdNew => ("새로 만들기", "New"),
    /// Command: Open a document.
    CmdOpen => ("열기…", "Open…"),
    /// Command: Save the document.
    CmdSave => ("저장", "Save"),
    /// Command: Save under another name.
    CmdSaveAs => ("다른 이름으로 저장…", "Save As…"),
    /// Command: Quit the application.
    CmdQuit => ("끝내기", "Quit"),
    /// Command: Undo.
    CmdUndo => ("실행 취소", "Undo"),
    /// Command: Redo.
    CmdRedo => ("다시 실행", "Redo"),
    /// Command: Delete the selection.
    CmdDelete => ("삭제", "Delete"),
    /// Status bar while idle.
    StatusReady => ("준비", "Ready"),
    /// Status bar: how many dimensions a sketch still needs. `{0}` is the count.
    StatusDimensionsNeeded => ("{0}개 치수 필요", "{0} dimensions needed"),
    /// Status bar: the sketch is fully constrained.
    StatusFullyConstrained => ("완전 구속", "Fully constrained"),
    /// Button: confirm.
    Ok => ("확인", "OK"),
    /// Button: cancel.
    Cancel => ("취소", "Cancel"),
}

impl Msg {
    /// The text with `{0}`, `{1}`... replaced by the arguments. Placeholders without an argument stay as
    /// they are, so a mistake is visible instead of silent.
    pub fn format(self, lang: Lang, args: &[&dyn Display]) -> String {
        let mut out = self.text(lang).to_owned();
        for (index, arg) in args.iter().enumerate() {
            out = out.replace(&format!("{{{index}}}"), &arg.to_string());
        }
        out
    }
}

/// Shorthand for [`Msg::text`] and [`Msg::format`]: `tr!(lang, StatusReady)`, `tr!(lang, Key, a, b)`.
#[macro_export]
macro_rules! tr {
    ($lang:expr, $key:ident) => {
        $crate::Msg::$key.text($lang)
    };
    ($lang:expr, $key:ident, $($arg:expr),+ $(,)?) => {
        $crate::Msg::$key.format($lang, &[$(&$arg),+])
    };
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// The placeholders (`{0}`, `{1}`...) that occur in a text.
    fn placeholders(text: &str) -> BTreeSet<usize> {
        (0..10).filter(|index| text.contains(&format!("{{{index}}}"))).collect()
    }

    #[test]
    fn every_message_has_text_in_every_language() {
        for message in Msg::ALL {
            for lang in Lang::ALL {
                assert!(!message.text(lang).trim().is_empty(), "{} is empty in {lang:?}", message.key());
            }
        }
    }

    #[test]
    fn translations_use_the_same_placeholders() {
        for message in Msg::ALL {
            assert_eq!(
                placeholders(message.text(Lang::Ko)),
                placeholders(message.text(Lang::En)),
                "{}",
                message.key()
            );
        }
    }

    #[test]
    fn keys_are_unique() {
        let keys: BTreeSet<&str> = Msg::ALL.iter().map(|m| m.key()).collect();
        assert_eq!(keys.len(), Msg::ALL.len());
    }

    #[test]
    fn formatting_fills_placeholders() {
        assert_eq!(tr!(Lang::Ko, StatusDimensionsNeeded, 3), "3개 치수 필요");
        assert_eq!(tr!(Lang::En, StatusDimensionsNeeded, 3), "3 dimensions needed");
        assert_eq!(tr!(Lang::Ko, StatusReady), "준비");
        assert_eq!(Msg::StatusDimensionsNeeded.format(Lang::En, &[]), "{0} dimensions needed");
    }

    #[test]
    fn language_codes_round_trip() {
        for lang in Lang::ALL {
            assert_eq!(Lang::from_code(lang.code()), Some(lang));
        }
        assert_eq!(Lang::from_code("fr"), None);
        assert_eq!(Lang::default(), Lang::Ko);
    }
}
