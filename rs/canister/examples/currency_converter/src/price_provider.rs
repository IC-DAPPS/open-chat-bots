pub mod xrc;

use thousands::Separable;

pub fn format_float(number: f64) -> String {
    if number >= 1.0 {
        let formatted = format!("{:.2}", number);
        // Remove trailing zeros
        let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
        trimmed.separate_with_commas().to_string()
    } else if number >= 0.01 {
        let formatted = format!("{:.4}", number);
        // Remove trailing zeros
        let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
        trimmed.separate_with_commas().to_string()
    } else if number >= 0.00001 {
        let formatted = format!("{:.6}", number);
        // Remove trailing zeros
        let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
        trimmed.separate_with_commas().to_string()
    } else {
        number.separate_with_commas().to_string() // Return the number as is (no formatting)
    }
}
