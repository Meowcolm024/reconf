use std::borrow::Cow;

use reedline::{
    DefaultPrompt, DefaultPromptSegment, Prompt, PromptEditMode, PromptHistorySearch, PromptViMode,
};

pub struct ReconfPrompt(DefaultPrompt);

impl ReconfPrompt {
    pub fn new(counter: usize) -> Self {
        Self(DefaultPrompt {
            left_prompt: DefaultPromptSegment::Basic("reconf ".into()),
            right_prompt: DefaultPromptSegment::Basic(format!(" -<{counter}>- ")),
        })
    }
}

impl Prompt for ReconfPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        self.0.render_prompt_left()
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        self.0.render_prompt_right()
    }

    fn render_prompt_indicator(&self, prompt_mode: PromptEditMode) -> Cow<'_, str> {
        match prompt_mode {
            PromptEditMode::Default | PromptEditMode::Emacs => "> ".into(),
            PromptEditMode::Vi(vi_mode) => match vi_mode {
                PromptViMode::Normal => "> ".into(),
                PromptViMode::Insert => ": ".into(),
            },
            PromptEditMode::Custom(name) => format!("({name})").into(),
        }
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        let prompt_length = match &self.0.left_prompt {
            DefaultPromptSegment::Basic(prompt) => prompt.len(),
            _ => 4,
        };
        Cow::Owned(" ".repeat(prompt_length) + "| ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        self.0
            .render_prompt_history_search_indicator(history_search)
    }
}
