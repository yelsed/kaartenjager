//! Euro formatting in Dutch convention: thousands with a dot, decimals with a comma.

pub fn euros(amount: f64) -> String {
    format!("€{}", group_thousands(&format!("{:.0}", amount.round())))
}

pub fn euros_precise(amount: f64) -> String {
    let whole = amount.trunc().abs();
    let cents = ((amount.abs() - whole) * 100.0).round() as u64;
    let sign = if amount < 0.0 { "-" } else { "" };
    format!(
        "{sign}€{},{:02}",
        group_thousands(&format!("{whole:.0}")),
        cents
    )
}

fn group_thousands(digits: &str) -> String {
    let negative = digits.starts_with('-');
    let bare = digits.trim_start_matches('-');
    let mut grouped = String::new();
    for (position, character) in bare.chars().enumerate() {
        if position > 0 && (bare.len() - position) % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(character);
    }
    if negative {
        format!("-{grouped}")
    } else {
        grouped
    }
}
