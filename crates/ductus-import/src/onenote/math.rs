//! OneNote equations as LaTeX. OneNote keeps an equation as linear text where U+FDD0 opens
//! a math object, U+FDEE separates its arguments and U+FDEF closes it. Each opening
//! character comes with the object it opens.

use onenote_parser::contents::{MathInlineObject, MathObjectType};

pub(crate) const OPEN: char = '\u{fdd0}';
const NEXT: char = '\u{fdee}';
const CLOSE: char = '\u{fdef}';

pub(crate) fn to_latex(text: &[(char, MathInlineObject)]) -> String {
    let mut parser = Parser { text, pos: 0 };
    let mut out = String::new();
    // Stray separators (an equation split over runs) are skipped rather than ending it.
    while parser.pos < text.len() {
        out.push_str(&parser.sequence());
        parser.pos += 1;
    }
    out.trim().to_owned()
}

struct Parser<'a> {
    text: &'a [(char, MathInlineObject)],
    pos: usize,
}

impl Parser<'_> {
    /// Up to the next separator or close, which is left in place.
    fn sequence(&mut self) -> String {
        let mut out = String::new();
        while let Some(&(c, object)) = self.text.get(self.pos) {
            match c {
                NEXT | CLOSE => break,
                OPEN => {
                    self.pos += 1;
                    let mut args = Vec::new();
                    loop {
                        args.push(self.sequence());
                        match self.text.get(self.pos).map(|&(c, _)| c) {
                            Some(NEXT) => self.pos += 1,
                            Some(CLOSE) => {
                                self.pos += 1;
                                break;
                            }
                            _ => break,
                        }
                    }
                    out.push_str(&object_latex(&object, &args));
                    // Typst's LaTeX reader glues "\\frac{1}{a}e" into the name "ae".
                    out.push(' ');
                }
                c => {
                    out.push_str(&symbol(c));
                    self.pos += 1;
                }
            }
        }
        out
    }
}

fn arg(args: &[String], i: usize) -> &str {
    args.get(i).map_or("", String::as_str)
}

fn object_latex(object: &MathInlineObject, args: &[String]) -> String {
    let a = |i| arg(args, i);
    match object.object_type() {
        MathObjectType::Fraction => format!("\\frac{{{}}}{{{}}}", a(0), a(1)),
        MathObjectType::SlashedFraction => format!("{{{}}}/{{{}}}", a(0), a(1)),
        MathObjectType::Stack => format!("\\genfrac{{}}{{}}{{0pt}}{{}}{{{}}}{{{}}}", a(0), a(1)),
        MathObjectType::Superscript => format!("{{{}}}^{{{}}}", a(0), a(1)),
        MathObjectType::Subscript => format!("{{{}}}_{{{}}}", a(0), a(1)),
        MathObjectType::SubSup => format!("{{{}}}_{{{}}}^{{{}}}", a(0), a(1), a(2)),
        MathObjectType::LeftSubSup => format!("{{}}_{{{}}}^{{{}}}{{{}}}", a(0), a(1), a(2)),
        MathObjectType::Radical => match args.len() {
            0 | 1 => format!("\\sqrt{{{}}}", a(0)),
            _ if a(0).is_empty() => format!("\\sqrt{{{}}}", a(1)),
            _ => format!("\\sqrt[{}]{{{}}}", a(0), a(1)),
        },
        MathObjectType::Nary => {
            let op = object.char().map_or_else(|| "\\int".to_owned(), operator);
            let mut out = op;
            if !a(0).is_empty() {
                out.push_str(&format!("_{{{}}}", a(0)));
            }
            if !a(1).is_empty() {
                out.push_str(&format!("^{{{}}}", a(1)));
            }
            format!("{out} {}", a(2))
        }
        MathObjectType::Brackets | MathObjectType::BracketsWithSeps => {
            let open = object.char().map_or("(".to_owned(), |c| delimiter(c, true));
            let close = object
                .char1()
                .map_or(")".to_owned(), |c| delimiter(c, false));
            let sep = match object.char2() {
                Some(c) if object.object_type() == MathObjectType::BracketsWithSeps => {
                    format!(" {} ", symbol(c))
                }
                _ => ", ".to_owned(),
            };
            format!("\\left{open} {} \\right{close}", args.join(&sep))
        }
        MathObjectType::FunctionApply => {
            let name = a(0);
            let named = if name.chars().all(|c| c.is_ascii_alphabetic()) && !name.is_empty() {
                if KNOWN_FUNCTIONS.contains(&name) {
                    format!("\\{name}")
                } else {
                    format!("\\operatorname{{{name}}}")
                }
            } else {
                name.to_owned()
            };
            format!("{named} {}", a(1))
        }
        MathObjectType::Matrix => {
            let columns = object
                .column()
                .map(usize::from)
                .filter(|c| *c > 0)
                .unwrap_or_else(|| (args.len() as f64).sqrt().round().max(1.0) as usize);
            let rows: Vec<String> = args.chunks(columns).map(|r| r.join(" & ")).collect();
            format!("\\begin{{matrix}} {} \\end{{matrix}}", rows.join(" \\\\ "))
        }
        MathObjectType::EquationArray => {
            format!(
                "\\begin{{aligned}} {} \\end{{aligned}}",
                args.join(" \\\\ ")
            )
        }
        MathObjectType::LowerLimit if KNOWN_FUNCTIONS.contains(&a(0).trim()) => {
            format!("\\{}_{{{}}}", a(0).trim(), a(1))
        }
        MathObjectType::LowerLimit => format!("\\underset{{{}}}{{{}}}", a(1), a(0)),
        MathObjectType::UpperLimit => format!("\\overset{{{}}}{{{}}}", a(1), a(0)),
        MathObjectType::Overbar => format!("\\overline{{{}}}", a(0)),
        MathObjectType::Underbar => format!("\\underline{{{}}}", a(0)),
        MathObjectType::BoxedFormula => format!("\\boxed{{{}}}", a(0)),
        MathObjectType::Accent => {
            let accent = match object.char() {
                Some('\u{302}') => "\\hat",
                Some('\u{303}') => "\\tilde",
                Some('\u{304}' | '\u{305}') => "\\bar",
                Some('\u{307}') => "\\dot",
                Some('\u{308}') => "\\ddot",
                Some('\u{20d7}') => "\\vec",
                _ => "",
            };
            format!("{accent}{{{}}}", a(0))
        }
        _ => args.join(" "),
    }
}

const KNOWN_FUNCTIONS: &[&str] = &[
    "sin", "cos", "tan", "cot", "sec", "csc", "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh",
    "log", "ln", "lg", "exp", "lim", "max", "min", "sup", "inf", "det", "deg", "dim", "gcd", "arg",
];

fn operator(c: char) -> String {
    match c {
        '∑' => "\\sum".into(),
        '∏' => "\\prod".into(),
        '∐' => "\\coprod".into(),
        '∫' => "\\int".into(),
        '∬' => "\\iint".into(),
        '∭' => "\\iiint".into(),
        '∮' => "\\oint".into(),
        '⋃' => "\\bigcup".into(),
        '⋂' => "\\bigcap".into(),
        c => symbol(c),
    }
}

fn delimiter(c: char, open: bool) -> String {
    match c {
        '(' | ')' | '[' | ']' | '|' | '/' => c.to_string(),
        '{' => "\\{".into(),
        '}' => "\\}".into(),
        '‖' => "\\|".into(),
        '⟨' => "\\langle".into(),
        '⟩' => "\\rangle".into(),
        '⌊' => "\\lfloor".into(),
        '⌋' => "\\rfloor".into(),
        '⌈' => "\\lceil".into(),
        '⌉' => "\\rceil".into(),
        _ if open => "(".into(),
        _ => ")".into(),
    }
}

const GREEK: [&str; 25] = [
    "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa",
    "lambda", "mu", "nu", "xi", "o", "pi", "rho", "varsigma", "sigma", "tau", "upsilon", "phi",
    "chi", "psi", "omega",
];

fn greek(index: u32, capital: bool) -> String {
    let Some(name) = GREEK.get(index as usize) else {
        return String::new();
    };
    if !capital {
        return if *name == "o" {
            "o".into()
        } else {
            format!("\\{name} ")
        };
    }
    // Capitals without a command of their own look like Latin letters.
    match *name {
        "alpha" => "A".into(),
        "beta" => "B".into(),
        "epsilon" => "E".into(),
        "zeta" => "Z".into(),
        "eta" => "H".into(),
        "iota" => "I".into(),
        "kappa" => "K".into(),
        "mu" => "M".into(),
        "nu" => "N".into(),
        "o" => "O".into(),
        "rho" => "P".into(),
        "tau" => "T".into(),
        "chi" => "X".into(),
        "varsigma" => String::new(),
        _ => {
            let mut chars = name.chars();
            let first = chars
                .next()
                .map(|c| c.to_ascii_uppercase())
                .unwrap_or_default();
            format!("\\{first}{} ", chars.as_str())
        }
    }
}

/// One character of OneNote's math text as LaTeX.
fn symbol(c: char) -> String {
    let code = c as u32;
    match code {
        // Math italic letters, each its own variable (a space keeps "dx" from becoming one
        // name in Typst); italic h has its own code point.
        0x1D44E..=0x1D467 => {
            return format!("{} ", char::from_u32(code - 0x1D44E + 'a' as u32).unwrap());
        }
        0x1D434..=0x1D44D => {
            return format!("{} ", char::from_u32(code - 0x1D434 + 'A' as u32).unwrap());
        }
        0x210E => return "h ".into(),
        0x1D6FC..=0x1D714 => return greek(code - 0x1D6FC, false),
        0x1D6E2..=0x1D6FA => return greek(code - 0x1D6E2, true),
        0x3B1..=0x3C9 => return greek(code - 0x3B1, false),
        0x391..=0x3A9 => return greek(code - 0x391, true),
        _ => {}
    }
    let s = match c {
        '−' => "-",
        '∞' => "\\infty ",
        '→' => "\\to ",
        '←' => "\\leftarrow ",
        '↔' => "\\leftrightarrow ",
        '⇒' => "\\Rightarrow ",
        '⇐' => "\\Leftarrow ",
        '⇔' => "\\Leftrightarrow ",
        '∈' => "\\in ",
        '∉' => "\\notin ",
        '⊂' => "\\subset ",
        '⊆' => "\\subseteq ",
        '⊃' => "\\supset ",
        '⊇' => "\\supseteq ",
        '∩' => "\\cap ",
        '∪' => "\\cup ",
        '∅' => "\\emptyset ",
        '∀' => "\\forall ",
        '∃' => "\\exists ",
        '¬' => "\\neg ",
        '∧' => "\\wedge ",
        '∨' => "\\vee ",
        'ℝ' => "\\mathbb{R}",
        'ℕ' => "\\mathbb{N}",
        'ℤ' => "\\mathbb{Z}",
        'ℚ' => "\\mathbb{Q}",
        'ℂ' => "\\mathbb{C}",
        'ⅆ' => "d",
        'ⅇ' => "e",
        'ⅈ' => "i",
        '≤' => "\\le ",
        '≥' => "\\ge ",
        '≠' => "\\neq ",
        '≈' => "\\approx ",
        '≡' => "\\equiv ",
        '∼' => "\\sim ",
        '∝' => "\\propto ",
        '±' => "\\pm ",
        '∓' => "\\mp ",
        '×' => "\\times ",
        '÷' => "\\div ",
        '·' | '⋅' => "\\cdot ",
        '∘' => "\\circ ",
        '∂' => "\\partial ",
        '∇' => "\\nabla ",
        '∑' => "\\sum ",
        '∏' => "\\prod ",
        '∫' => "\\int ",
        '∮' => "\\oint ",
        '√' => "\\sqrt",
        '…' => "\\ldots ",
        '⋯' => "\\cdots ",
        '°' => "^\\circ ",
        '′' => "'",
        '{' => "\\{",
        '}' => "\\}",
        '%' => "\\%",
        '#' => "\\#",
        '&' => "\\&",
        '$' => "\\$",
        '_' => "\\_",
        '\\' => "\\backslash ",
        // Invisible function application, times and separator.
        '\u{2061}' | '\u{2062}' | '\u{2063}' | '\u{200b}' => "",
        c => return c.to_string(),
    };
    s.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn latex(text: &str) -> String {
        let text: Vec<_> = text
            .chars()
            .map(|c| (c, MathInlineObject::default()))
            .collect();
        to_latex(&text)
    }

    #[test]
    fn letters_and_symbols_become_latex() {
        assert_eq!(latex("𝑎+𝜃≤∞, Ω"), "a +\\theta \\le \\infty , \\Omega");
    }

    #[test]
    fn unknown_objects_keep_their_arguments() {
        assert_eq!(latex("\u{fdd0}𝑥\u{fdee}2\u{fdef}"), "x  2");
    }
}
