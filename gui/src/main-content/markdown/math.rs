// =============================================================================
// Markdown LaTeX Math Engine (`src/markdown/math.rs`)
// =============================================================================
// Formatter for LaTeX inline math (`$...$`) and display math (`$$...$$`) expressions.
// Translates LaTeX environments (`\begin{aligned}`), fractions (`\frac{a}{b}`),
// text blocks (`\text{...}`), spacing (`\quad`), delimiters (`\left`, `\right`),
// Greek symbols, operators, and sub/superscripts to clean, legible mathematical notation.

use regex::Regex;
use std::sync::OnceLock;

static RE_FRAC: OnceLock<Regex> = OnceLock::new();
static RE_TEXT: OnceLock<Regex> = OnceLock::new();
static RE_SUB_BRAC: OnceLock<Regex> = OnceLock::new();
static RE_SUP_BRAC: OnceLock<Regex> = OnceLock::new();

fn get_re_frac() -> &'static Regex {
    RE_FRAC.get_or_init(|| Regex::new(r"\\frac\s*\{([^}]+)\}\s*\{([^}]+)\}").unwrap())
}

fn get_re_text() -> &'static Regex {
    RE_TEXT.get_or_init(|| {
        Regex::new(r"\\(?:text|mathrm|mathbf|mathsf|mathtt)\s*\{([^}]+)\}").unwrap()
    })
}

fn get_re_sub_brac() -> &'static Regex {
    RE_SUB_BRAC.get_or_init(|| Regex::new(r"_\{([^}]+)\}").unwrap())
}

fn get_re_sup_brac() -> &'static Regex {
    RE_SUP_BRAC.get_or_init(|| Regex::new(r"\^\{([^}]+)\}").unwrap())
}

/// Translates LaTeX macros, environments, fractions, text blocks, spacing, Greek symbols, and sub/superscripts into Unicode notation.
pub fn latex_to_unicode(expr: &str) -> String {
    let mut s = expr.to_string();

    // 1. Handle Environments (\begin{aligned} ... \end{aligned})
    s = s.replace("\\begin{aligned}", "");
    s = s.replace("\\end{aligned}", "");
    s = s.replace("\\begin{array}", "");
    s = s.replace("\\end{array}", "");
    s = s.replace("\\begin{equation}", "");
    s = s.replace("\\end{equation}", "");

    // 2. Handle text blocks (\text{...}, \mathrm{...}, \mathbf{...})
    s = get_re_text().replace_all(&s, "$1").to_string();

    // 3. Spacing commands (\qquad, \quad, \,, \;, \!)
    s = s.replace("\\qquad", "      ");
    s = s.replace("\\quad", "   ");
    s = s.replace("\\,", " ");
    s = s.replace("\\;", " ");
    s = s.replace("\\!", "");

    // 4. Alignment markers and line breaks in LaTeX equations
    s = s.replace("&=", " = ");
    s = s.replace("&", " ");
    s = s.replace("\\\\", "\n");

    // 5. Delimiters & Modifiers (\left(, \right), \left[, \right], \cdot, \text{...})
    s = s.replace("\\left(", "(");
    s = s.replace("\\right)", ")");
    s = s.replace("\\left[", "[");
    s = s.replace("\\right]", "]");
    s = s.replace("\\left\\{", "{");
    s = s.replace("\\right\\}", "}");
    s = s.replace("\\cdot", " · ");
    s = s.replace("\\times", " × ");
    s = s.replace("\\div", " ÷ ");

    // 6. Fractions (\frac{numerator}{denominator})
    for _ in 0..3 {
        s = get_re_frac().replace_all(&s, "($1 / $2)").to_string();
    }

    // 7. Greek Lowercase
    s = s.replace("\\alpha", "α");
    s = s.replace("\\beta", "β");
    s = s.replace("\\gamma", "γ");
    s = s.replace("\\delta", "δ");
    s = s.replace("\\epsilon", "ε");
    s = s.replace("\\zeta", "ζ");
    s = s.replace("\\eta", "η");
    s = s.replace("\\theta", "θ");
    s = s.replace("\\iota", "ι");
    s = s.replace("\\kappa", "κ");
    s = s.replace("\\lambda", "λ");
    s = s.replace("\\mu", "μ");
    s = s.replace("\\nu", "ν");
    s = s.replace("\\xi", "ξ");
    s = s.replace("\\pi", "π");
    s = s.replace("\\rho", "ρ");
    s = s.replace("\\sigma", "σ");
    s = s.replace("\\tau", "τ");
    s = s.replace("\\upsilon", "υ");
    s = s.replace("\\phi", "φ");
    s = s.replace("\\chi", "χ");
    s = s.replace("\\psi", "ψ");
    s = s.replace("\\omega", "ω");

    // 8. Greek Uppercase
    s = s.replace("\\Gamma", "Γ");
    s = s.replace("\\Delta", "Δ");
    s = s.replace("\\Theta", "Θ");
    s = s.replace("\\Lambda", "Λ");
    s = s.replace("\\Xi", "Ξ");
    s = s.replace("\\Pi", "Π");
    s = s.replace("\\Sigma", "Σ");
    s = s.replace("\\Phi", "Φ");
    s = s.replace("\\Psi", "Ψ");
    s = s.replace("\\Omega", "Ω");

    // 9. Operators & Symbols (Replaced AFTER delimiters to avoid \left -> ≤ft bug)
    s = s.replace("\\infty", "∞");
    s = s.replace("\\sqrt", "√");
    s = s.replace("\\le", "≤");
    s = s.replace("\\ge", "≥");
    s = s.replace("\\ne", "≠");
    s = s.replace("\\pm", "±");
    s = s.replace("\\mp", "∓");
    s = s.replace("\\approx", "≈");
    s = s.replace("\\equiv", "≡");
    s = s.replace("\\partial", "∂");
    s = s.replace("\\nabla", "∇");
    s = s.replace("\\sum", "∑");
    s = s.replace("\\int", "∫");
    s = s.replace("\\prod", "∏");
    s = s.replace("\\rightarrow", "→");
    s = s.replace("\\to", "→");
    s = s.replace("\\leftarrow", "←");
    s = s.replace("\\Rightarrow", "⇒");
    s = s.replace("\\Leftarrow", "⇐");
    s = s.replace("\\in", "∈");
    s = s.replace("\\notin", "∉");
    s = s.replace("\\cap", "∩");
    s = s.replace("\\cup", "∪");

    // 10. Branced Subscripts & Superscripts (_{t} -> ₜ)
    s = get_re_sub_brac().replace_all(&s, "_$1").to_string();
    s = get_re_sup_brac().replace_all(&s, "^$1").to_string();

    // 11. Single Superscripts
    s = s.replace("^0", "⁰");
    s = s.replace("^1", "¹");
    s = s.replace("^2", "²");
    s = s.replace("^3", "³");
    s = s.replace("^4", "⁴");
    s = s.replace("^5", "⁵");
    s = s.replace("^6", "⁶");
    s = s.replace("^7", "⁷");
    s = s.replace("^8", "⁸");
    s = s.replace("^9", "⁹");
    s = s.replace("^+", "⁺");
    s = s.replace("^-", "⁻");
    s = s.replace("^n", "ⁿ");
    s = s.replace("^x", "ˣ");
    s = s.replace("^t", "ᵗ");

    // 12. Single Subscripts
    s = s.replace("_0", "₀");
    s = s.replace("_1", "₁");
    s = s.replace("_2", "₂");
    s = s.replace("_3", "₃");
    s = s.replace("_4", "₄");
    s = s.replace("_5", "₅");
    s = s.replace("_6", "₆");
    s = s.replace("_7", "₇");
    s = s.replace("_8", "₈");
    s = s.replace("_9", "₉");
    s = s.replace("_i", "ᵢ");
    s = s.replace("_j", "ⱼ");
    s = s.replace("_n", "ₙ");
    s = s.replace("_m", "ₘ");
    s = s.replace("_t", "ₜ");

    // Clean up remaining single curly braces
    s = s.replace('{', "");
    s = s.replace('}', "");

    s.trim().to_string()
}

/// Formats an inline math expression (`$...\$`) for Slint `StyledText` using CommonMark italic syntax.
pub fn format_inline_math(expr: &str) -> String {
    let unicode = latex_to_unicode(expr);
    format!(" *{}* ", unicode)
}

/// Formats a display math expression (`$$...\$$`) for Slint rendering.
pub fn format_display_math(expr: &str) -> String {
    latex_to_unicode(expr)
}
