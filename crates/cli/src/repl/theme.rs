use std::str::FromStr;

use serde_json::Value;
use syntect::highlighting::ParseThemeError::*;
use syntect::highlighting::{
    Color, FontStyle, ParseThemeError, ScopeSelectors, StyleModifier, Theme, ThemeItem,
    ThemeSettings, UnderlineOption,
};

pub trait ParseSettings: Sized {
    type Error;

    fn parse_settings(settings: Value) -> Result<Self, Self::Error>;
}

impl ParseSettings for Theme {
    type Error = ParseThemeError;

    fn parse_settings(settings: Value) -> Result<Theme, Self::Error> {
        let mut obj = match settings {
            Value::Object(obj) => obj,
            _ => return Err(IncorrectSyntax),
        };
        let name = match obj.remove("name") {
            Some(Value::String(name)) => Some(name),
            None => None,
            _ => return Err(IncorrectSyntax),
        };
        let author = match obj.remove("author") {
            Some(Value::String(author)) => Some(author),
            None => None,
            _ => return Err(IncorrectSyntax),
        };
        let items = match obj.remove("settings") {
            Some(Value::Array(items)) => items,
            _ => return Err(IncorrectSyntax),
        };
        let mut settings = match obj.remove("globals") {
            Some(globals) => ThemeSettings::parse_settings(globals)?,
            None => ThemeSettings::default(),
        };
        if let Some(Value::Object(obj)) = obj.remove("gutterSettings") {
            for (key, value) in obj {
                let color = Color::parse_settings(value).ok();
                match &key[..] {
                    "background" => settings.gutter = settings.gutter.or(color),
                    "foreground" => {
                        settings.gutter_foreground = settings.gutter_foreground.or(color);
                    }
                    _ => (),
                }
            }
        }
        let mut scopes = Vec::new();
        for json in items {
            if let Ok(item) = ThemeItem::parse_settings(json) {
                scopes.push(item);
            }
        }
        Ok(Theme {
            name,
            author,
            settings,
            scopes,
        })
    }
}

impl ParseSettings for Color {
    type Error = ParseThemeError;

    fn parse_settings(settings: Value) -> Result<Color, Self::Error> {
        match settings {
            Value::String(value) => Color::from_str(&value),
            _ => Err(IncorrectColor),
        }
    }
}

impl ParseSettings for StyleModifier {
    type Error = ParseThemeError;

    fn parse_settings(settings: Value) -> Result<StyleModifier, Self::Error> {
        let mut obj = match settings {
            Value::Object(obj) => obj,
            _ => return Err(ColorShemeScopeIsNotObject),
        };
        let font_style = match obj.remove("fontStyle").or_else(|| obj.remove("font_style")) {
            Some(Value::String(value)) => Some(FontStyle::from_str(&value)?),
            None => None,
            Some(c) => return Err(IncorrectFontStyle(c.to_string())),
        };
        let foreground = match obj.remove("foreground") {
            Some(Value::String(value)) => Some(Color::from_str(&value)?),
            None => None,
            _ => return Err(IncorrectColor),
        };
        let background = match obj.remove("background") {
            Some(Value::String(value)) => Some(Color::from_str(&value)?),
            None => None,
            _ => return Err(IncorrectColor),
        };

        Ok(StyleModifier {
            foreground,
            background,
            font_style,
        })
    }
}

impl ParseSettings for ThemeItem {
    type Error = ParseThemeError;

    fn parse_settings(settings: Value) -> Result<ThemeItem, Self::Error> {
        let mut obj = match settings {
            Value::Object(obj) => obj,
            _ => return Err(ColorShemeScopeIsNotObject),
        };
        let scope = match obj.remove("scope") {
            Some(Value::String(value)) => ScopeSelectors::from_str(&value)?,
            _ => return Err(ScopeSelectorIsNotString(format!("{:?}", obj))),
        };
        let style = if let Some(Value::Object(mut style)) = obj.remove("settings") {
            if let Some(font_style) = obj.remove("fontStyle").or_else(|| obj.remove("font_style")) {
                style.insert("fontStyle".to_string(), font_style);
            }
            StyleModifier::parse_settings(Value::Object(style))?
        } else {
            StyleModifier::parse_settings(Value::Object(obj))?
        };
        Ok(ThemeItem { scope, style })
    }
}

impl ParseSettings for ThemeSettings {
    type Error = ParseThemeError;

    fn parse_settings(json: Value) -> Result<ThemeSettings, Self::Error> {
        let mut settings = ThemeSettings::default();

        let obj = match json {
            Value::Object(obj) => obj,
            _ => return Err(ColorShemeSettingsIsNotObject),
        };

        for (key, value) in obj {
            match &key[..] {
                "foreground" => settings.foreground = Color::parse_settings(value).ok(),
                "background" => settings.background = Color::parse_settings(value).ok(),
                "caret" => settings.caret = Color::parse_settings(value).ok(),
                "lineHighlight" | "line_highlight" => {
                    settings.line_highlight = Color::parse_settings(value).ok();
                }
                "selection" => settings.selection = Color::parse_settings(value).ok(),
                "accent" => settings.accent = Color::parse_settings(value).ok(),
                "guide" => settings.guide = Color::parse_settings(value).ok(),
                "activeGuide" => settings.active_guide = Color::parse_settings(value).ok(),
                "stackGuide" => settings.stack_guide = Color::parse_settings(value).ok(),
                "shadow" => settings.shadow = Color::parse_settings(value).ok(),
                _ => (),
            }
        }
        Ok(settings)
    }
}

impl ParseSettings for UnderlineOption {
    type Error = ParseThemeError;

    fn parse_settings(settings: Value) -> Result<UnderlineOption, Self::Error> {
        match settings {
            Value::String(value) => UnderlineOption::from_str(&value),
            _ => Err(IncorrectUnderlineOption),
        }
    }
}
